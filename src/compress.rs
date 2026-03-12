use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{GenericImageView, ImageReader};

const SUPPORTED_INPUT_EXTENSIONS: &[&str] =
    &["jpg", "jpeg", "png", "webp", "tiff", "tif", "bmp", "gif"];

#[derive(Debug)]
pub struct CompressOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub quality: u8,
}

#[derive(Debug)]
pub struct CompressResult {
    pub input: PathBuf,
    pub output: PathBuf,
    pub width: u32,
    pub height: u32,
    pub input_size_bytes: u64,
    pub output_size_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
enum OutputFormat {
    Jpeg,
    Png,
}

pub fn compress_image(options: &CompressOptions) -> Result<CompressResult, String> {
    validate_input_path(&options.input)?;
    validate_input_extension(&options.input)?;

    if !(1..=100).contains(&options.quality) {
        return Err("quality must be in 1..=100".to_string());
    }

    let output_format = infer_output_format(&options.output)?;

    let parent = options
        .output
        .parent()
        .ok_or_else(|| format!("invalid output path: {}", options.output.display()))?;
    std::fs::create_dir_all(parent).map_err(|err| {
        format!(
            "failed to create output directory {}: {err}",
            parent.display()
        )
    })?;

    let image = ImageReader::open(&options.input)
        .map_err(|err| {
            format!(
                "failed to open input image {}: {err}",
                options.input.display()
            )
        })?
        .with_guessed_format()
        .map_err(|err| {
            format!(
                "failed to detect input format {}: {err}",
                options.input.display()
            )
        })?
        .decode()
        .map_err(|err| {
            format!(
                "failed to decode input image {}: {err}",
                options.input.display()
            )
        })?;

    let (width, height) = image.dimensions();
    let input_size_bytes = std::fs::metadata(&options.input)
        .map_err(|err| {
            format!(
                "failed to read input file metadata {}: {err}",
                options.input.display()
            )
        })?
        .len();

    let file = File::create(&options.output).map_err(|err| {
        format!(
            "failed to create output file {}: {err}",
            options.output.display()
        )
    })?;
    let writer = BufWriter::new(file);

    match output_format {
        OutputFormat::Jpeg => {
            let encoder = JpegEncoder::new_with_quality(writer, options.quality);
            image.write_with_encoder(encoder).map_err(|err| {
                format!("failed to encode JPEG {}: {err}", options.output.display())
            })?;
        }
        OutputFormat::Png => {
            let encoder =
                PngEncoder::new_with_quality(writer, CompressionType::Best, FilterType::Adaptive);
            image.write_with_encoder(encoder).map_err(|err| {
                format!("failed to encode PNG {}: {err}", options.output.display())
            })?;
        }
    }

    let output_size_bytes = std::fs::metadata(&options.output)
        .map_err(|err| {
            format!(
                "failed to read output file metadata {}: {err}",
                options.output.display()
            )
        })?
        .len();

    Ok(CompressResult {
        input: options.input.clone(),
        output: options.output.clone(),
        width,
        height,
        input_size_bytes,
        output_size_bytes,
    })
}

fn validate_input_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("input path does not exist: {}", path.display()));
    }

    if !path.is_file() {
        return Err(format!("input path is not a file: {}", path.display()));
    }

    Ok(())
}

fn validate_input_extension(path: &Path) -> Result<(), String> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| format!("missing file extension: {}", path.display()))?;

    if SUPPORTED_INPUT_EXTENSIONS.contains(&ext.as_str()) {
        Ok(())
    } else {
        Err(format!(
            "unsupported input extension `{ext}` for compress: {}",
            path.display()
        ))
    }
}

fn infer_output_format(path: &Path) -> Result<OutputFormat, String> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| format!("missing output extension: {}", path.display()))?;

    match ext.as_str() {
        "jpg" | "jpeg" => Ok(OutputFormat::Jpeg),
        "png" => Ok(OutputFormat::Png),
        _ => Err(format!(
            "compression currently supports only jpg/jpeg/png output: {}",
            path.display()
        )),
    }
}
