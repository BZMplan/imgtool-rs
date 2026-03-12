use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use image::{ColorType, GenericImageView, ImageReader};
use walkdir::{DirEntry, WalkDir};

use super::exif::{extract_exif_bundle, is_no_exif_error};
use super::format::{
    FileKind, format_info_from_path, has_supported_extension, is_supported_raster_format,
    raster_format_mime_and_ext,
};
use super::model::{BatchSummary, MetadataRecord, ProcessOutput, RecordStatus};

#[derive(Debug, Clone, Copy)]
pub struct ProcessOptions {
    pub include_gps: bool,
    pub include_hidden: bool,
    pub fail_fast: bool,
}

pub fn process_path(path: &Path, opts: ProcessOptions) -> Result<ProcessOutput, String> {
    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }

    let targets = if path.is_file() {
        if has_supported_extension(path) {
            vec![path.to_path_buf()]
        } else {
            return Ok(ProcessOutput {
                records: Vec::new(),
                summary: BatchSummary {
                    total: 1,
                    succeeded: 0,
                    failed: 1,
                    aborted: false,
                },
            });
        }
    } else if path.is_dir() {
        collect_files(path, opts.include_hidden)?
    } else {
        return Err(format!("unsupported path type: {}", path.display()));
    };

    let mut records = Vec::new();
    let mut succeeded = 0;
    let mut failed = 0;
    let mut aborted = false;

    for target in targets {
        match read_metadata(&target, opts.include_gps) {
            Ok(record) => {
                succeeded += 1;
                records.push(record);
            }
            Err(_) => {
                failed += 1;
                if opts.fail_fast {
                    aborted = true;
                    break;
                }
            }
        }
    }

    Ok(ProcessOutput {
        summary: BatchSummary {
            total: succeeded + failed,
            succeeded,
            failed,
            aborted,
        },
        records,
    })
}

fn collect_files(root: &Path, include_hidden: bool) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();

    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| include_hidden || !is_hidden_entry(entry));

    for item in walker {
        let entry = item.map_err(|err| format!("walkdir error: {err}"))?;
        if entry.file_type().is_file() && has_supported_extension(entry.path()) {
            files.push(entry.into_path());
        }
    }

    files.sort();
    Ok(files)
}

fn is_hidden_entry(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| name.starts_with('.'))
}

fn read_metadata(path: &Path, include_gps: bool) -> Result<MetadataRecord, String> {
    let format_info = format_info_from_path(path)
        .ok_or_else(|| format!("unsupported file extension: {}", path.display()))?;

    let mut record = MetadataRecord {
        path: path.display().to_string(),
        status: RecordStatus::Ok,
        error: None,
        warnings: Vec::new(),
        file_size_bytes: None,
        modified_at: None,
        mime: Some(format_info.mime.to_string()),
        extension: path
            .extension()
            .and_then(OsStr::to_str)
            .map(|ext| ext.to_ascii_lowercase())
            .or_else(|| Some(format_info.canonical_ext.to_string())),
        width: None,
        height: None,
        pixel_count: None,
        channels: None,
        bit_depth: None,
        has_alpha: None,
        orientation: None,
        color_space: None,
        capture: None,
    };

    fill_fs_metadata(path, &mut record)?;

    match format_info.kind {
        FileKind::Raster => read_raster_metadata(path, include_gps, &mut record)?,
        FileKind::Raw => read_raw_metadata(path, include_gps, &mut record)?,
    }

    Ok(record)
}

fn fill_fs_metadata(path: &Path, record: &mut MetadataRecord) -> Result<(), String> {
    let fs_meta = std::fs::metadata(path)
        .map_err(|err| format!("failed to read file metadata for {}: {err}", path.display()))?;

    record.file_size_bytes = Some(fs_meta.len());

    if let Ok(modified) = fs_meta.modified() {
        if let Ok(since_epoch) = modified.duration_since(UNIX_EPOCH) {
            record.modified_at = Some(since_epoch.as_secs().to_string());
        }
    }

    Ok(())
}

fn read_raster_metadata(
    path: &Path,
    include_gps: bool,
    record: &mut MetadataRecord,
) -> Result<(), String> {
    let reader = ImageReader::open(path)
        .map_err(|err| format!("failed to open image {}: {err}", path.display()))?;
    let reader = reader
        .with_guessed_format()
        .map_err(|err| format!("failed to detect image format {}: {err}", path.display()))?;

    let format = reader
        .format()
        .ok_or_else(|| format!("unsupported or unknown image format: {}", path.display()))?;

    if !is_supported_raster_format(format) {
        return Err(format!(
            "unsupported raster format {:?}: {}",
            format,
            path.display()
        ));
    }

    if let Some((mime, canonical_ext)) = raster_format_mime_and_ext(format) {
        record.mime = Some(mime.to_string());
        if record.extension.is_none() {
            record.extension = Some(canonical_ext.to_string());
        }
    }

    let decoded = reader
        .decode()
        .map_err(|err| format!("failed to decode image {}: {err}", path.display()))?;

    let (width, height) = decoded.dimensions();
    let color = decoded.color();
    record.width = Some(width);
    record.height = Some(height);
    record.pixel_count = Some(width as u64 * height as u64);
    record.channels = Some(color.channel_count());
    record.bit_depth = Some(bit_depth(color));
    record.has_alpha = Some(color.has_alpha());
    record.color_space = Some(infer_color_space(color));

    match extract_exif_bundle(path, include_gps) {
        Ok(Some(exif)) => apply_exif(record, exif),
        Ok(None) => {}
        Err(err) => {
            if !is_no_exif_error(&err) {
                record.warnings.push(format!("failed to read EXIF: {err}"));
            }
        }
    }

    Ok(())
}

fn read_raw_metadata(
    path: &Path,
    include_gps: bool,
    record: &mut MetadataRecord,
) -> Result<(), String> {
    let exif = extract_exif_bundle(path, include_gps)
        .map_err(|err| format!("failed to read RAW metadata {}: {err}", path.display()))?
        .ok_or_else(|| format!("RAW file has no EXIF metadata: {}", path.display()))?;

    apply_exif(record, exif);

    if record.width.is_none() || record.height.is_none() {
        return Err(format!(
            "RAW metadata missing dimensions: {}",
            path.display()
        ));
    }

    if record.color_space.is_none() {
        record.color_space = Some("Unknown (RAW)".to_string());
    }

    Ok(())
}

fn apply_exif(record: &mut MetadataRecord, exif: super::exif::ExifBundle) {
    if let Some(orientation) = exif.orientation {
        record.orientation = Some(orientation);
    }

    if let Some(color_space) = exif.color_space {
        record.color_space = Some(color_space);
    }

    if let Some(capture) = exif.capture {
        record.capture = Some(capture);
    }

    if let Some((width, height)) = exif.dimensions {
        record.width = Some(width);
        record.height = Some(height);
        record.pixel_count = Some(width as u64 * height as u64);
    }
}

fn bit_depth(color: ColorType) -> u8 {
    let channels = color.channel_count();
    if channels == 0 {
        return 0;
    }
    (color.bits_per_pixel() / channels as u16) as u8
}

fn infer_color_space(color: ColorType) -> String {
    match color {
        ColorType::L8 | ColorType::L16 | ColorType::La8 | ColorType::La16 => {
            "Grayscale".to_string()
        }
        ColorType::Rgb8 | ColorType::Rgb16 | ColorType::Rgb32F => "RGB".to_string(),
        ColorType::Rgba8 | ColorType::Rgba16 | ColorType::Rgba32F => "RGB+Alpha".to_string(),
        _ => "sRGB".to_string(),
    }
}
