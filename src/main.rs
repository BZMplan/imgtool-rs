mod batch;
mod compress;
mod convert;
mod metadata;
mod resize;

use std::path::PathBuf;

use batch::{BatchOptions, run_batch};
use clap::{Args, Parser, Subcommand};
use compress::{CompressOptions, compress_image};
use convert::{ConvertOptions, convert_image};
use metadata::{CaptureInfo, MetadataRecord, ProcessOptions, RecordStatus, process_path};
use resize::{ResizeFilter, ResizeOptions, resize_image};
use serde::Serialize;

const ROOT_AFTER_HELP: &str = "\
Examples:\n\
  imgtool-rs meta ./photos --json\n\
  imgtool-rs resize in.jpg out.jpg --scale 0.5 --filter lanczos3\n\
  imgtool-rs compress in.jpg out.jpg --quality 80 --max-size-kb 200\n\
  imgtool-rs convert in.png out.webp\n\
  imgtool-rs batch ./input ./output --resize-scale 0.5 --compress --quality 75";

const META_AFTER_HELP: &str = "\
Examples:\n\
  imgtool-rs meta ./photo.jpg\n\
  imgtool-rs meta ./photos --json --include-hidden\n\
  imgtool-rs meta ./photos --fail-fast";

const RESIZE_AFTER_HELP: &str = "\
Examples:\n\
  imgtool-rs resize in.png out.png --scale 0.5\n\
  imgtool-rs resize in.jpg out.jpg --scale 2.0 --filter catmullrom";

const COMPRESS_AFTER_HELP: &str = "\
Examples:\n\
  imgtool-rs compress in.jpg out.jpg --quality 70\n\
  imgtool-rs compress in.jpg out.jpg --quality 95 --max-size-kb 200\n\
  imgtool-rs compress in.png out.png";

const CONVERT_AFTER_HELP: &str = "\
Examples:\n\
  imgtool-rs convert in.png out.jpg --quality 80\n\
  imgtool-rs convert in.jpg out.webp\n\
  imgtool-rs convert in.jpg out.tiff";

const BATCH_AFTER_HELP: &str = "\
Examples:\n\
  imgtool-rs batch ./in ./out --resize-scale 0.5\n\
  imgtool-rs batch ./in ./out --compress --quality 70\n\
  imgtool-rs batch ./in ./out --resize-scale 0.5 --compress --max-size-kb 300\n\
  imgtool-rs batch ./in ./out --compress --fail-fast";

#[derive(Parser)]
#[command(
    name = "imgtool-rs",
    version,
    about = "CLI for image metadata, resize, compression, conversion, and batch processing",
    subcommand_required = true,
    arg_required_else_help = true,
    propagate_version = true,
    next_line_help = true,
    after_help = ROOT_AFTER_HELP
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Read metadata from a file or directory")]
    Meta(MetaArgs),
    #[command(about = "Resize a single image by scale ratio")]
    Resize(ResizeArgs),
    #[command(about = "Compress image file size while keeping dimensions")]
    Compress(CompressArgs),
    #[command(about = "Convert image format while preserving dimensions")]
    Convert(ConvertArgs),
    #[command(about = "Run resize/compress pipeline on a directory tree")]
    Batch(BatchArgs),
}

#[derive(Args)]
#[command(next_line_help = true, after_help = META_AFTER_HELP)]
struct MetaArgs {
    /// File or directory path.
    #[arg(value_name = "PATH")]
    path: PathBuf,

    /// Print JSON output instead of text.
    #[arg(long)]
    json: bool,

    /// Hide GPS data even if EXIF includes it.
    #[arg(long = "no-gps")]
    no_gps: bool,

    /// Include hidden files and hidden directories while scanning.
    #[arg(long)]
    include_hidden: bool,

    /// Stop on first failed file.
    #[arg(long)]
    fail_fast: bool,
}

#[derive(Args)]
#[command(next_line_help = true, after_help = RESIZE_AFTER_HELP)]
struct ResizeArgs {
    /// Input image path.
    #[arg(value_name = "INPUT")]
    input: PathBuf,

    /// Output image path.
    #[arg(value_name = "OUTPUT")]
    output: PathBuf,

    /// Scale ratio, e.g. 0.5 for half size or 2.0 for double size.
    #[arg(long)]
    scale: f64,

    /// Resampling filter: nearest, triangle, catmullrom, gaussian, lanczos3.
    #[arg(long, default_value = "lanczos3", value_enum)]
    filter: ResizeFilterArg,
}

#[derive(Args)]
#[command(next_line_help = true, after_help = COMPRESS_AFTER_HELP)]
struct CompressArgs {
    /// Input image path.
    #[arg(value_name = "INPUT")]
    input: PathBuf,

    /// Output image path.
    #[arg(value_name = "OUTPUT")]
    output: PathBuf,

    /// Compression quality for JPEG output, in [1, 100].
    #[arg(long, default_value_t = 75, value_parser = clap::value_parser!(u8).range(1..=100))]
    quality: u8,

    /// Target max output size in KB (JPEG output only).
    #[arg(long = "max-size-kb")]
    max_size_kb: Option<u64>,
}

#[derive(Args)]
#[command(next_line_help = true, after_help = CONVERT_AFTER_HELP)]
struct ConvertArgs {
    /// Input image path.
    #[arg(value_name = "INPUT")]
    input: PathBuf,

    /// Output image path.
    #[arg(value_name = "OUTPUT")]
    output: PathBuf,

    /// Quality used for JPEG output, in [1, 100].
    #[arg(long, default_value_t = 85, value_parser = clap::value_parser!(u8).range(1..=100))]
    quality: u8,
}

#[derive(Args)]
#[command(next_line_help = true, after_help = BATCH_AFTER_HELP)]
struct BatchArgs {
    /// Input directory path.
    #[arg(value_name = "INPUT_DIR")]
    input_dir: PathBuf,

    /// Output directory path.
    #[arg(value_name = "OUTPUT_DIR")]
    output_dir: PathBuf,

    /// Optional resize scale ratio.
    #[arg(long = "resize-scale")]
    resize_scale: Option<f64>,

    /// Resampling filter for resize stage.
    #[arg(long = "resize-filter", default_value = "lanczos3", value_enum)]
    resize_filter: ResizeFilterArg,

    /// Enable compress stage.
    #[arg(long)]
    compress: bool,

    /// Compression quality for JPEG output, in [1, 100].
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=100))]
    quality: Option<u8>,

    /// Target max output size in KB (JPEG output only).
    #[arg(long = "max-size-kb")]
    max_size_kb: Option<u64>,

    /// Include hidden files and hidden directories.
    #[arg(long)]
    include_hidden: bool,

    /// Stop batch on first failure.
    #[arg(long)]
    fail_fast: bool,
}

#[derive(clap::ValueEnum, Clone, Copy)]
enum ResizeFilterArg {
    Nearest,
    Triangle,
    Catmullrom,
    Gaussian,
    Lanczos3,
}

impl From<ResizeFilterArg> for ResizeFilter {
    fn from(value: ResizeFilterArg) -> Self {
        match value {
            ResizeFilterArg::Nearest => ResizeFilter::Nearest,
            ResizeFilterArg::Triangle => ResizeFilter::Triangle,
            ResizeFilterArg::Catmullrom => ResizeFilter::CatmullRom,
            ResizeFilterArg::Gaussian => ResizeFilter::Gaussian,
            ResizeFilterArg::Lanczos3 => ResizeFilter::Lanczos3,
        }
    }
}

#[derive(Serialize)]
struct JsonEnvelope<'a> {
    summary: &'a metadata::BatchSummary,
    records: &'a [MetadataRecord],
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Meta(args) => run_meta(args),
        Commands::Resize(args) => run_resize(args),
        Commands::Compress(args) => run_compress(args),
        Commands::Convert(args) => run_convert(args),
        Commands::Batch(args) => run_batch_command(args),
    };

    std::process::exit(exit_code);
}

fn run_convert(args: ConvertArgs) -> i32 {
    let options = ConvertOptions {
        input: args.input,
        output: args.output,
        quality: args.quality,
    };

    match convert_image(&options) {
        Ok(result) => {
            println!(
                "Converted: {} -> {} | format={} | dimensions={}x{} | size={}B -> {}B{}",
                result.input.display(),
                result.output.display(),
                result.output_format,
                result.width,
                result.height,
                result.input_size_bytes,
                result.output_size_bytes,
                result
                    .applied_quality
                    .map(|q| format!(" | quality={q}"))
                    .unwrap_or_default()
            );
            0
        }
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    }
}

fn run_compress(args: CompressArgs) -> i32 {
    let options = CompressOptions {
        input: args.input,
        output: args.output,
        quality: args.quality,
        max_size_kb: args.max_size_kb,
    };

    match compress_image(&options) {
        Ok(result) => {
            let delta = result.input_size_bytes as i128 - result.output_size_bytes as i128;
            let ratio = if result.input_size_bytes == 0 {
                0.0
            } else {
                (delta as f64 / result.input_size_bytes as f64) * 100.0
            };
            println!(
                "Compressed: {} -> {} | dimensions={}x{} | size={}B -> {}B | delta={}B ({:.2}%) | quality={}{}",
                result.input.display(),
                result.output.display(),
                result.width,
                result.height,
                result.input_size_bytes,
                result.output_size_bytes,
                delta,
                ratio,
                result.applied_quality.unwrap_or(args.quality),
                result
                    .target_size_bytes
                    .map(|bytes| format!(", target<={}B", bytes))
                    .unwrap_or_default()
            );
            0
        }
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    }
}

fn run_batch_command(args: BatchArgs) -> i32 {
    let compress_enabled = args.compress || args.quality.is_some() || args.max_size_kb.is_some();

    let options = BatchOptions {
        input_dir: args.input_dir,
        output_dir: args.output_dir,
        resize_scale: args.resize_scale,
        resize_filter: args.resize_filter.into(),
        do_compress: compress_enabled,
        quality: args.quality.unwrap_or(75),
        max_size_kb: args.max_size_kb,
        include_hidden: args.include_hidden,
        fail_fast: args.fail_fast,
    };

    match run_batch(&options) {
        Ok(result) => {
            println!(
                "Batch summary: total={}, succeeded={}, failed={}, aborted={}",
                result.total, result.succeeded, result.failed, result.aborted
            );
            if !result.errors.is_empty() {
                for err in &result.errors {
                    eprintln!("error: {err}");
                }
            }
            if result.failed == 0 { 0 } else { 1 }
        }
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    }
}

fn run_resize(args: ResizeArgs) -> i32 {
    let options = ResizeOptions {
        input: args.input,
        output: args.output,
        scale: args.scale,
        filter: args.filter.into(),
    };

    match resize_image(&options) {
        Ok(result) => {
            println!(
                "Resized: {} ({}x{}) -> {} ({}x{}), scale={}",
                result.input.display(),
                result.original_width,
                result.original_height,
                result.output.display(),
                result.output_width,
                result.output_height,
                result.scale
            );
            0
        }
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    }
}

fn run_meta(args: MetaArgs) -> i32 {
    let options = ProcessOptions {
        include_gps: !args.no_gps,
        include_hidden: args.include_hidden,
        fail_fast: args.fail_fast,
    };

    let output = match process_path(&args.path, options) {
        Ok(output) => output,
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    };

    if args.json {
        let envelope = JsonEnvelope {
            summary: &output.summary,
            records: &output.records,
        };
        match serde_json::to_string_pretty(&envelope) {
            Ok(json) => println!("{json}"),
            Err(err) => {
                eprintln!("error: failed to serialize json: {err}");
                return 1;
            }
        }
    } else {
        print_text(&output.records, &output.summary);
    }

    if output.summary.failed > 0 {
        if output.summary.aborted { 1 } else { 2 }
    } else {
        0
    }
}

fn print_text(records: &[MetadataRecord], summary: &metadata::BatchSummary) {
    for record in records {
        println!("Path: {}", record.path);
        println!("Status: {}", status_label(record.status));

        if let Some(error) = &record.error {
            println!("Error: {error}");
        }

        if let Some(size) = record.file_size_bytes {
            println!("File size: {} bytes", size);
        }
        if let Some(modified) = &record.modified_at {
            println!("Modified at (unix): {modified}");
        }
        if let Some(mime) = &record.mime {
            println!("MIME: {mime}");
        }
        if let Some(extension) = &record.extension {
            println!("Extension: {extension}");
        }
        if let (Some(width), Some(height)) = (record.width, record.height) {
            println!("Dimensions: {}x{}", width, height);
        }
        if let Some(pixel_count) = record.pixel_count {
            println!("Pixel count: {pixel_count}");
        }
        if let Some(channels) = record.channels {
            println!("Channels: {channels}");
        }
        if let Some(bit_depth) = record.bit_depth {
            println!("Bit depth: {bit_depth}");
        }
        if let Some(has_alpha) = record.has_alpha {
            println!("Has alpha: {has_alpha}");
        }
        if let Some(orientation) = &record.orientation {
            println!("Orientation: {orientation}");
        }
        if let Some(color_space) = &record.color_space {
            println!("Color space: {color_space}");
        }

        if let Some(capture) = &record.capture {
            print_capture(capture);
        }

        if !record.warnings.is_empty() {
            println!("Warnings:");
            for warning in &record.warnings {
                println!("  - {warning}");
            }
        }

        println!();
    }

    println!(
        "Summary: succeeded={}, failed={}",
        summary.succeeded, summary.failed
    );
}

fn print_capture(capture: &CaptureInfo) {
    println!("Capture:");

    if let Some(camera_make) = &capture.camera_make {
        println!("  Camera make: {camera_make}");
    }
    if let Some(camera_model) = &capture.camera_model {
        println!("  Camera model: {camera_model}");
    }
    if let Some(lens_model) = &capture.lens_model {
        println!("  Lens model: {lens_model}");
    }
    if let Some(datetime_original) = &capture.datetime_original {
        println!("  Date/time original: {datetime_original}");
    }
    if let Some(exposure_time) = &capture.exposure_time {
        println!("  Exposure time: {exposure_time}");
    }
    if let Some(f_number) = &capture.f_number {
        println!("  F-number: {f_number}");
    }
    if let Some(iso) = &capture.iso {
        println!("  ISO: {iso}");
    }
    if let Some(focal_length) = &capture.focal_length {
        println!("  Focal length: {focal_length}");
    }
    if let Some(gps) = &capture.gps {
        if let Some(latitude) = gps.latitude {
            println!("  GPS latitude: {:.6}", latitude);
        }
        if let Some(longitude) = gps.longitude {
            println!("  GPS longitude: {:.6}", longitude);
        }
        if let Some(altitude) = gps.altitude_m {
            println!("  GPS altitude(m): {:.2}", altitude);
        }
    }
}

fn status_label(status: RecordStatus) -> &'static str {
    match status {
        RecordStatus::Ok => "ok",
        RecordStatus::Error => "error",
    }
}
