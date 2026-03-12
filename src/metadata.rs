use std::ffi::OsStr;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use exif::{In, Rational, Reader as ExifReader, Tag, Value};
use image::{ColorType, GenericImageView, ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone, Copy)]
pub struct ProcessOptions {
    pub include_gps: bool,
    pub include_hidden: bool,
    pub fail_fast: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessOutput {
    pub records: Vec<MetadataRecord>,
    pub summary: BatchSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchSummary {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub aborted: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RecordStatus {
    Ok,
    Error,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetadataRecord {
    pub path: String,
    pub status: RecordStatus,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub file_size_bytes: Option<u64>,
    pub modified_at: Option<String>,
    pub mime: Option<String>,
    pub extension: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub pixel_count: Option<u64>,
    pub channels: Option<u8>,
    pub bit_depth: Option<u8>,
    pub has_alpha: Option<bool>,
    pub orientation: Option<String>,
    pub color_space: Option<String>,
    pub capture: Option<CaptureInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CaptureInfo {
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub datetime_original: Option<String>,
    pub exposure_time: Option<String>,
    pub f_number: Option<String>,
    pub iso: Option<String>,
    pub focal_length: Option<String>,
    pub gps: Option<GpsInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GpsInfo {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude_m: Option<f64>,
}

struct ExifBundle {
    orientation: Option<String>,
    color_space: Option<String>,
    capture: Option<CaptureInfo>,
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
        let record = read_metadata(&target, opts.include_gps);
        if record.status == RecordStatus::Ok {
            succeeded += 1;
            records.push(record);
        } else {
            failed += 1;
            if opts.fail_fast {
                aborted = true;
                break;
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

fn read_metadata(path: &Path, include_gps: bool) -> MetadataRecord {
    let mut record = MetadataRecord {
        path: path.display().to_string(),
        status: RecordStatus::Ok,
        error: None,
        warnings: Vec::new(),
        file_size_bytes: None,
        modified_at: None,
        mime: None,
        extension: path
            .extension()
            .and_then(OsStr::to_str)
            .map(|ext| ext.to_ascii_lowercase()),
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

    let fs_meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(err) => {
            set_error(&mut record, format!("failed to read file metadata: {err}"));
            return record;
        }
    };

    record.file_size_bytes = Some(fs_meta.len());
    if let Ok(modified) = fs_meta.modified() {
        if let Ok(since_epoch) = modified.duration_since(UNIX_EPOCH) {
            record.modified_at = Some(since_epoch.as_secs().to_string());
        }
    }

    let reader = match ImageReader::open(path) {
        Ok(reader) => reader,
        Err(err) => {
            set_error(&mut record, format!("failed to open image: {err}"));
            return record;
        }
    };

    let reader = match reader.with_guessed_format() {
        Ok(reader) => reader,
        Err(err) => {
            set_error(&mut record, format!("failed to detect image format: {err}"));
            return record;
        }
    };

    let format = match reader.format() {
        Some(format) if is_supported_format(format) => format,
        Some(format) => {
            set_error(
                &mut record,
                format!("unsupported image format: {:?}", format),
            );
            return record;
        }
        None => {
            set_error(
                &mut record,
                "unsupported or unknown image format".to_string(),
            );
            return record;
        }
    };

    let (mime, fallback_ext) = format_mime_and_ext(format);
    record.mime = Some(mime.to_string());
    if record.extension.is_none() {
        record.extension = Some(fallback_ext.to_string());
    }

    let decoded = match reader.decode() {
        Ok(image) => image,
        Err(err) => {
            set_error(&mut record, format!("failed to decode image: {err}"));
            return record;
        }
    };

    let (width, height) = decoded.dimensions();
    let color = decoded.color();
    record.width = Some(width);
    record.height = Some(height);
    record.pixel_count = Some(width as u64 * height as u64);
    record.channels = Some(color.channel_count());
    record.has_alpha = Some(color.has_alpha());
    record.bit_depth = Some(bit_depth(color));
    record.color_space = Some(infer_color_space(color));

    match extract_exif(path, include_gps) {
        Ok(Some(exif)) => {
            if exif.orientation.is_some() {
                record.orientation = exif.orientation;
            }
            if exif.color_space.is_some() {
                record.color_space = exif.color_space;
            }
            if exif.capture.is_some() {
                record.capture = exif.capture;
            }
        }
        Ok(None) => {}
        Err(err) => {
            if !is_no_exif_error(&err) {
                record.warnings.push(format!("failed to read EXIF: {err}"));
            }
        }
    }

    record
}

fn extract_exif(path: &Path, include_gps: bool) -> Result<Option<ExifBundle>, String> {
    let file = File::open(path).map_err(|err| err.to_string())?;
    let mut reader = BufReader::new(file);
    let exif = ExifReader::new()
        .read_from_container(&mut reader)
        .map_err(|err| err.to_string())?;

    let camera_make = field_text(&exif, Tag::Make);
    let camera_model = field_text(&exif, Tag::Model);
    let lens_model = field_text(&exif, Tag::LensModel);
    let datetime_original = field_text(&exif, Tag::DateTimeOriginal);
    let exposure_time = field_text(&exif, Tag::ExposureTime);
    let f_number = field_text(&exif, Tag::FNumber);
    let iso = field_text(&exif, Tag::PhotographicSensitivity)
        .or_else(|| field_text(&exif, Tag::ISOSpeed));
    let focal_length = field_text(&exif, Tag::FocalLength);
    let gps = if include_gps { parse_gps(&exif) } else { None };

    let capture_has_data = camera_make.is_some()
        || camera_model.is_some()
        || lens_model.is_some()
        || datetime_original.is_some()
        || exposure_time.is_some()
        || f_number.is_some()
        || iso.is_some()
        || focal_length.is_some()
        || gps.is_some();

    let capture = if capture_has_data {
        Some(CaptureInfo {
            camera_make,
            camera_model,
            lens_model,
            datetime_original,
            exposure_time,
            f_number,
            iso,
            focal_length,
            gps,
        })
    } else {
        None
    };

    let orientation = field_text(&exif, Tag::Orientation);
    let color_space = field_text(&exif, Tag::ColorSpace);

    if orientation.is_none() && color_space.is_none() && capture.is_none() {
        return Ok(None);
    }

    Ok(Some(ExifBundle {
        orientation,
        color_space,
        capture,
    }))
}

fn field_text(exif: &exif::Exif, tag: Tag) -> Option<String> {
    exif.get_field(tag, In::PRIMARY)
        .map(|field| field.display_value().with_unit(exif).to_string())
        .and_then(normalize_text)
}

fn parse_gps(exif: &exif::Exif) -> Option<GpsInfo> {
    let latitude = parse_gps_coordinate(exif, Tag::GPSLatitude, Tag::GPSLatitudeRef);
    let longitude = parse_gps_coordinate(exif, Tag::GPSLongitude, Tag::GPSLongitudeRef);
    let altitude = parse_gps_altitude(exif);

    if latitude.is_none() && longitude.is_none() && altitude.is_none() {
        return None;
    }

    Some(GpsInfo {
        latitude,
        longitude,
        altitude_m: altitude,
    })
}

fn parse_gps_coordinate(exif: &exif::Exif, coord_tag: Tag, ref_tag: Tag) -> Option<f64> {
    let coord_field = exif.get_field(coord_tag, In::PRIMARY)?;

    let values = match &coord_field.value {
        Value::Rational(values) if values.len() >= 3 => values,
        _ => return None,
    };

    let d = rational_to_f64(values[0])?;
    let m = rational_to_f64(values[1])?;
    let s = rational_to_f64(values[2])?;

    let mut decimal = d + m / 60.0 + s / 3600.0;
    if let Some(ref_value) = field_text(exif, ref_tag) {
        let first = ref_value.chars().next().unwrap_or_default();
        if matches!(first, 'S' | 'W' | 's' | 'w') {
            decimal *= -1.0;
        }
    }

    Some(decimal)
}

fn parse_gps_altitude(exif: &exif::Exif) -> Option<f64> {
    let altitude_field = exif.get_field(Tag::GPSAltitude, In::PRIMARY)?;
    let base = match &altitude_field.value {
        Value::Rational(values) if !values.is_empty() => rational_to_f64(values[0])?,
        _ => return None,
    };

    let below_sea_level = exif
        .get_field(Tag::GPSAltitudeRef, In::PRIMARY)
        .and_then(|field| match field.value {
            Value::Byte(ref bytes) if !bytes.is_empty() => Some(bytes[0] == 1),
            _ => None,
        })
        .unwrap_or(false);

    Some(if below_sea_level { -base } else { base })
}

fn rational_to_f64(rational: Rational) -> Option<f64> {
    if rational.denom == 0 {
        return None;
    }
    Some(rational.num as f64 / rational.denom as f64)
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
        other => format!("{other:?}"),
    }
}

fn is_supported_format(format: ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::Jpeg
            | ImageFormat::Png
            | ImageFormat::WebP
            | ImageFormat::Tiff
            | ImageFormat::Bmp
            | ImageFormat::Gif
    )
}

fn format_mime_and_ext(format: ImageFormat) -> (&'static str, &'static str) {
    match format {
        ImageFormat::Jpeg => ("image/jpeg", "jpg"),
        ImageFormat::Png => ("image/png", "png"),
        ImageFormat::WebP => ("image/webp", "webp"),
        ImageFormat::Tiff => ("image/tiff", "tiff"),
        ImageFormat::Bmp => ("image/bmp", "bmp"),
        ImageFormat::Gif => ("image/gif", "gif"),
        _ => ("application/octet-stream", "bin"),
    }
}

fn has_supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase())
        .is_some_and(|ext| {
            matches!(
                ext.as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "tiff" | "tif" | "bmp" | "gif"
            )
        })
}

fn is_no_exif_error(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("no exif") || message.contains("not found")
}

fn normalize_text(raw: String) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(trimmed);

    if unquoted.is_empty() {
        None
    } else {
        Some(unquoted.to_string())
    }
}

fn set_error(record: &mut MetadataRecord, message: String) {
    record.status = RecordStatus::Error;
    record.error = Some(message);
}
