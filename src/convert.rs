use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::codecs::webp::WebPEncoder;
use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader};

const SUPPORTED_INPUT_EXTENSIONS: &[&str] =
    &["jpg", "jpeg", "png", "webp", "tiff", "tif", "bmp", "gif"];

#[derive(Debug)]
pub struct ConvertOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub quality: u8,
}

#[derive(Debug)]
pub struct ConvertResult {
    pub input: PathBuf,
    pub output: PathBuf,
    pub width: u32,
    pub height: u32,
    pub input_size_bytes: u64,
    pub output_size_bytes: u64,
    pub output_format: &'static str,
    pub applied_quality: Option<u8>,
}

#[derive(Debug, Clone, Copy)]
enum OutputFormat {
    Jpeg,
    Png,
    Webp,
    Bmp,
    Gif,
    Tiff,
}

pub fn convert_image(options: &ConvertOptions) -> Result<ConvertResult, String> {
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
    fs::create_dir_all(parent).map_err(|err| {
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
    let input_size_bytes = fs::metadata(&options.input)
        .map_err(|err| {
            format!(
                "failed to read input file metadata {}: {err}",
                options.input.display()
            )
        })?
        .len();

    let (encoded, output_format_name, applied_quality) = match output_format {
        OutputFormat::Jpeg => (
            encode_jpeg(&image, options.quality)?,
            "jpeg",
            Some(options.quality),
        ),
        OutputFormat::Png => (encode_png(&image)?, "png", None),
        OutputFormat::Webp => (encode_webp(&image)?, "webp", None),
        OutputFormat::Bmp => (encode_with_format(&image, ImageFormat::Bmp)?, "bmp", None),
        OutputFormat::Gif => (encode_with_format(&image, ImageFormat::Gif)?, "gif", None),
        OutputFormat::Tiff => (encode_with_format(&image, ImageFormat::Tiff)?, "tiff", None),
    };

    fs::write(&options.output, &encoded).map_err(|err| {
        format!(
            "failed to write output file {}: {err}",
            options.output.display()
        )
    })?;

    Ok(ConvertResult {
        input: options.input.clone(),
        output: options.output.clone(),
        width,
        height,
        input_size_bytes,
        output_size_bytes: encoded.len() as u64,
        output_format: output_format_name,
        applied_quality,
    })
}

fn encode_jpeg(image: &DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut bytes, quality);
    image
        .write_with_encoder(encoder)
        .map_err(|err| format!("failed to encode JPEG: {err}"))?;
    Ok(bytes)
}

fn encode_png(image: &DynamicImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let encoder =
        PngEncoder::new_with_quality(&mut bytes, CompressionType::Best, FilterType::Adaptive);
    image
        .write_with_encoder(encoder)
        .map_err(|err| format!("failed to encode PNG: {err}"))?;
    Ok(bytes)
}

fn encode_webp(image: &DynamicImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let encoder = WebPEncoder::new_lossless(&mut bytes);
    image
        .write_with_encoder(encoder)
        .map_err(|err| format!("failed to encode WebP: {err}"))?;
    Ok(bytes)
}

fn encode_with_format(image: &DynamicImage, format: ImageFormat) -> Result<Vec<u8>, String> {
    let mut cursor = Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, format)
        .map_err(|err| format!("failed to encode {:?}: {err}", format))?;
    Ok(cursor.into_inner())
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
            "unsupported input extension `{ext}` for convert: {}",
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
        "webp" => Ok(OutputFormat::Webp),
        "bmp" => Ok(OutputFormat::Bmp),
        "gif" => Ok(OutputFormat::Gif),
        "tiff" | "tif" => Ok(OutputFormat::Tiff),
        _ => Err(format!(
            "unsupported output extension `{ext}` for convert: {}",
            path.display()
        )),
    }
}
