use std::fs;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{DynamicImage, GenericImageView, ImageReader};

const SUPPORTED_INPUT_EXTENSIONS: &[&str] =
    &["jpg", "jpeg", "png", "webp", "tiff", "tif", "bmp", "gif"];

#[derive(Debug)]
pub struct CompressOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub quality: u8,
    pub max_size_kb: Option<u64>,
}

#[derive(Debug)]
pub struct CompressResult {
    pub input: PathBuf,
    pub output: PathBuf,
    pub width: u32,
    pub height: u32,
    pub input_size_bytes: u64,
    pub output_size_bytes: u64,
    pub applied_quality: Option<u8>,
    pub target_size_bytes: Option<u64>,
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

    if options.max_size_kb == Some(0) {
        return Err("max-size-kb must be greater than 0".to_string());
    }

    let output_format = infer_output_format(&options.output)?;
    if options.max_size_kb.is_some() && !matches!(output_format, OutputFormat::Jpeg) {
        return Err("--max-size-kb is currently supported only for jpg/jpeg output".to_string());
    }

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

    let target_size_bytes = options.max_size_kb.map(|kb| kb.saturating_mul(1024));

    let (encoded, applied_quality) = match output_format {
        OutputFormat::Jpeg => {
            if let Some(target) = target_size_bytes {
                let (bytes, quality) = find_jpeg_bytes_for_target(&image, options.quality, target)?;
                (bytes, Some(quality))
            } else {
                (encode_jpeg(&image, options.quality)?, Some(options.quality))
            }
        }
        OutputFormat::Png => (encode_png(&image)?, None),
    };

    fs::write(&options.output, &encoded).map_err(|err| {
        format!(
            "failed to write output file {}: {err}",
            options.output.display()
        )
    })?;

    let output_size_bytes = encoded.len() as u64;

    Ok(CompressResult {
        input: options.input.clone(),
        output: options.output.clone(),
        width,
        height,
        input_size_bytes,
        output_size_bytes,
        applied_quality,
        target_size_bytes,
    })
}

fn find_jpeg_bytes_for_target(
    image: &DynamicImage,
    preferred_max_quality: u8,
    target_size_bytes: u64,
) -> Result<(Vec<u8>, u8), String> {
    let max_quality = preferred_max_quality.clamp(1, 100);

    let max_quality_bytes = encode_jpeg(image, max_quality)?;
    if (max_quality_bytes.len() as u64) <= target_size_bytes {
        return Ok((max_quality_bytes, max_quality));
    }

    let mut low = 1_i32;
    let mut high = max_quality as i32 - 1;
    let mut best: Option<(Vec<u8>, u8)> = None;

    while low <= high {
        let mid = ((low + high) / 2) as u8;
        let bytes = encode_jpeg(image, mid)?;

        if (bytes.len() as u64) <= target_size_bytes {
            best = Some((bytes, mid));
            low = mid as i32 + 1;
        } else {
            high = mid as i32 - 1;
        }
    }

    best.ok_or_else(|| {
        format!(
            "cannot reach target size {} bytes even with JPEG quality=1",
            target_size_bytes
        )
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
