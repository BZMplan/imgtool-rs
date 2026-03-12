mod compress;
mod metadata;
mod resize;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use compress::{CompressOptions, compress_image};
use metadata::{CaptureInfo, MetadataRecord, ProcessOptions, RecordStatus, process_path};
use resize::{ResizeFilter, ResizeOptions, resize_image};
use serde::Serialize;

#[derive(Parser)]
#[command(name = "imgtool-rs", version, about = "Image metadata inspection CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Meta(MetaArgs),
    Resize(ResizeArgs),
    Compress(CompressArgs),
}

#[derive(Args)]
struct MetaArgs {
    /// File or directory path.
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
struct ResizeArgs {
    /// Input image path.
    input: PathBuf,

    /// Output image path.
    output: PathBuf,

    /// Scale ratio, e.g. 0.5 for half size or 2.0 for double size.
    #[arg(long)]
    scale: f64,

    /// Resampling filter: nearest, triangle, catmullrom, gaussian, lanczos3.
    #[arg(long, default_value = "lanczos3", value_enum)]
    filter: ResizeFilterArg,
}

#[derive(Args)]
struct CompressArgs {
    /// Input image path.
    input: PathBuf,

    /// Output image path.
    output: PathBuf,

    /// Compression quality for JPEG output, in [1, 100].
    #[arg(long, default_value_t = 75, value_parser = clap::value_parser!(u8).range(1..=100))]
    quality: u8,
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
    };

    std::process::exit(exit_code);
}

fn run_compress(args: CompressArgs) -> i32 {
    let options = CompressOptions {
        input: args.input,
        output: args.output,
        quality: args.quality,
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
                "Compressed: {} -> {} | dimensions={}x{} | size={}B -> {}B | delta={}B ({:.2}%)",
                result.input.display(),
                result.output.display(),
                result.width,
                result.height,
                result.input_size_bytes,
                result.output_size_bytes,
                delta,
                ratio
            );
            0
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
