use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader};

const SUPPORTED_RASTER_EXTENSIONS: &[&str] =
    &["jpg", "jpeg", "png", "webp", "tiff", "tif", "bmp", "gif"];

#[derive(Debug, Clone, Copy)]
pub enum ResizeFilter {
    Nearest,
    Triangle,
    CatmullRom,
    Gaussian,
    Lanczos3,
}

impl ResizeFilter {
    pub fn as_filter_type(self) -> FilterType {
        match self {
            ResizeFilter::Nearest => FilterType::Nearest,
            ResizeFilter::Triangle => FilterType::Triangle,
            ResizeFilter::CatmullRom => FilterType::CatmullRom,
            ResizeFilter::Gaussian => FilterType::Gaussian,
            ResizeFilter::Lanczos3 => FilterType::Lanczos3,
        }
    }
}

#[derive(Debug)]
pub struct ResizeOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub scale: f64,
    pub filter: ResizeFilter,
}

#[derive(Debug)]
pub struct ResizeResult {
    pub input: PathBuf,
    pub output: PathBuf,
    pub original_width: u32,
    pub original_height: u32,
    pub scale: f64,
    pub output_width: u32,
    pub output_height: u32,
}

pub fn resize_image(options: &ResizeOptions) -> Result<ResizeResult, String> {
    validate_input_path(&options.input)?;
    validate_extension(&options.input)?;
    validate_output_extension(&options.output)?;

    if !options.scale.is_finite() || options.scale <= 0.0 {
        return Err("scale must be a positive number".to_string());
    }

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

    let format = ImageFormat::from_path(&options.output).map_err(|_| {
        format!(
            "unsupported output image format for path: {}",
            options.output.display()
        )
    })?;

    let input_image = ImageReader::open(&options.input)
        .map_err(|err| {
            format!(
                "failed to open input image {}: {err}",
                options.input.display()
            )
        })?
        .with_guessed_format()
        .map_err(|err| {
            format!(
                "failed to guess input format {}: {err}",
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

    let (original_width, original_height) = input_image.dimensions();
    let (output_width, output_height) =
        compute_scaled_dimensions(original_width, original_height, options.scale)?;

    let resized = resize_exact(input_image, output_width, output_height, options.filter);
    resized
        .save_with_format(&options.output, format)
        .map_err(|err| {
            format!(
                "failed to save resized image {}: {err}",
                options.output.display()
            )
        })?;

    Ok(ResizeResult {
        input: options.input.clone(),
        output: options.output.clone(),
        original_width,
        original_height,
        scale: options.scale,
        output_width,
        output_height,
    })
}

fn resize_exact(
    image: DynamicImage,
    width: u32,
    height: u32,
    filter: ResizeFilter,
) -> DynamicImage {
    image.resize_exact(width, height, filter.as_filter_type())
}

fn compute_scaled_dimensions(width: u32, height: u32, scale: f64) -> Result<(u32, u32), String> {
    let scaled_width = ((width as f64) * scale).round();
    let scaled_height = ((height as f64) * scale).round();

    if !scaled_width.is_finite() || !scaled_height.is_finite() {
        return Err("scaled image size is not finite".to_string());
    }

    let width_u64 = scaled_width.max(1.0) as u64;
    let height_u64 = scaled_height.max(1.0) as u64;

    if width_u64 > u32::MAX as u64 || height_u64 > u32::MAX as u64 {
        return Err("scaled image dimensions exceed u32 range".to_string());
    }

    Ok((width_u64 as u32, height_u64 as u32))
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

fn validate_extension(path: &Path) -> Result<(), String> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| format!("missing file extension: {}", path.display()))?;

    if SUPPORTED_RASTER_EXTENSIONS.contains(&ext.as_str()) {
        Ok(())
    } else {
        Err(format!(
            "unsupported input extension `{ext}` for resize: {}",
            path.display()
        ))
    }
}

fn validate_output_extension(path: &Path) -> Result<(), String> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| format!("missing output extension: {}", path.display()))?;

    if SUPPORTED_RASTER_EXTENSIONS.contains(&ext.as_str()) {
        Ok(())
    } else {
        Err(format!(
            "unsupported output extension `{ext}` for resize: {}",
            path.display()
        ))
    }
}
