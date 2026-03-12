use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use walkdir::{DirEntry, WalkDir};

use crate::compress::{CompressOptions, compress_image};
use crate::resize::{ResizeFilter, ResizeOptions, resize_image};

const SUPPORTED_RASTER_EXTENSIONS: &[&str] =
    &["jpg", "jpeg", "png", "webp", "tiff", "tif", "bmp", "gif"];

#[derive(Debug, Clone)]
pub struct BatchOptions {
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub resize_scale: Option<f64>,
    pub resize_filter: ResizeFilter,
    pub do_compress: bool,
    pub quality: u8,
    pub max_size_kb: Option<u64>,
    pub include_hidden: bool,
    pub fail_fast: bool,
}

#[derive(Debug)]
pub struct BatchResult {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub aborted: bool,
    pub errors: Vec<String>,
}

pub fn run_batch(options: &BatchOptions) -> Result<BatchResult, String> {
    if !options.input_dir.exists() {
        return Err(format!(
            "input directory does not exist: {}",
            options.input_dir.display()
        ));
    }
    if !options.input_dir.is_dir() {
        return Err(format!(
            "input path is not a directory: {}",
            options.input_dir.display()
        ));
    }

    if options.resize_scale.is_none() && !options.do_compress {
        return Err(
            "batch requires at least one operation: --resize-scale and/or compression options"
                .to_string(),
        );
    }

    let targets = collect_targets(&options.input_dir, options.include_hidden)?;

    let mut succeeded = 0_usize;
    let mut failed = 0_usize;
    let mut aborted = false;
    let mut errors = Vec::new();

    for (index, input_path) in targets.iter().enumerate() {
        let rel = match input_path.strip_prefix(&options.input_dir) {
            Ok(rel) => rel,
            Err(err) => {
                failed += 1;
                errors.push(format!(
                    "failed to build relative path for {}: {err}",
                    input_path.display()
                ));
                if options.fail_fast {
                    aborted = true;
                    break;
                }
                continue;
            }
        };

        let output_path = options.output_dir.join(rel);

        match process_one(options, index, input_path, &output_path) {
            Ok(()) => succeeded += 1,
            Err(err) => {
                failed += 1;
                errors.push(format!("{}: {err}", input_path.display()));
                if options.fail_fast {
                    aborted = true;
                    break;
                }
            }
        }
    }

    Ok(BatchResult {
        total: succeeded + failed,
        succeeded,
        failed,
        aborted,
        errors,
    })
}

fn process_one(
    options: &BatchOptions,
    index: usize,
    input_path: &Path,
    output_path: &Path,
) -> Result<(), String> {
    match (options.resize_scale, options.do_compress) {
        (Some(scale), true) => {
            let temp = build_temp_path(index, input_path.extension().and_then(OsStr::to_str));

            let resize_result = resize_image(&ResizeOptions {
                input: input_path.to_path_buf(),
                output: temp.clone(),
                scale,
                filter: options.resize_filter,
            });

            if let Err(err) = resize_result {
                cleanup_temp_file(&temp);
                return Err(err);
            }

            let compress_result = compress_image(&CompressOptions {
                input: temp.clone(),
                output: output_path.to_path_buf(),
                quality: options.quality,
                max_size_kb: options.max_size_kb,
            });

            cleanup_temp_file(&temp);
            compress_result.map(|_| ())
        }
        (Some(scale), false) => resize_image(&ResizeOptions {
            input: input_path.to_path_buf(),
            output: output_path.to_path_buf(),
            scale,
            filter: options.resize_filter,
        })
        .map(|_| ()),
        (None, true) => compress_image(&CompressOptions {
            input: input_path.to_path_buf(),
            output: output_path.to_path_buf(),
            quality: options.quality,
            max_size_kb: options.max_size_kb,
        })
        .map(|_| ()),
        (None, false) => Err("no operation configured".to_string()),
    }
}

fn build_temp_path(index: usize, extension: Option<&str>) -> PathBuf {
    let ext = extension.unwrap_or("tmp");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    std::env::temp_dir().join(format!(
        "imgtool-rs-batch-{}-{}-{}.{}",
        std::process::id(),
        nanos,
        index,
        ext
    ))
}

fn cleanup_temp_file(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn collect_targets(root: &Path, include_hidden: bool) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();

    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| include_hidden || !is_hidden_entry(entry));

    for item in walker {
        let entry = item.map_err(|err| format!("walkdir error: {err}"))?;
        if entry.file_type().is_file() && is_supported_raster(entry.path()) {
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

fn is_supported_raster(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase())
        .is_some_and(|ext| SUPPORTED_RASTER_EXTENSIONS.contains(&ext.as_str()))
}
