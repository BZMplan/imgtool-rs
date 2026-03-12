use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use assert_cmd::Command;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{GenericImageView, ImageBuffer, ImageEncoder, ImageReader, Rgb};
use tempfile::tempdir;

fn write_jpeg(path: &Path, width: u32, height: u32, quality: u8) {
    let mut raw = Vec::with_capacity((width as usize) * (height as usize) * 3);

    for y in 0..height {
        for x in 0..width {
            let r = ((x * 31 + y * 17) % 256) as u8;
            let g = ((x * 13 + y * 29) % 256) as u8;
            let b = ((x * 7 + y * 19) % 256) as u8;
            raw.extend_from_slice(&[r, g, b]);
        }
    }

    let file = File::create(path).expect("create jpeg");
    let writer = BufWriter::new(file);
    let encoder = JpegEncoder::new_with_quality(writer, quality);
    encoder
        .write_image(&raw, width, height, image::ExtendedColorType::Rgb8)
        .expect("encode jpeg");
}

fn write_png_uncompressed(path: &Path, width: u32, height: u32) {
    let image: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(width, height, Rgb([40, 200, 120]));

    let file = File::create(path).expect("create png");
    let writer = BufWriter::new(file);
    let encoder =
        PngEncoder::new_with_quality(writer, CompressionType::Uncompressed, FilterType::NoFilter);
    encoder
        .write_image(
            image.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgb8,
        )
        .expect("encode png");
}

#[test]
fn compress_jpeg_keeps_dimensions_and_reduces_size() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.jpg");
    let output = dir.path().join("output.jpg");

    write_jpeg(&input, 320, 240, 95);

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    cmd.args([
        "compress",
        &input.to_string_lossy(),
        &output.to_string_lossy(),
        "--quality",
        "40",
    ])
    .assert()
    .code(0);

    let input_size = fs::metadata(&input).expect("input metadata").len();
    let output_size = fs::metadata(&output).expect("output metadata").len();
    assert!(
        output_size < input_size,
        "expected output smaller, input={input_size}, output={output_size}"
    );

    let in_img = ImageReader::open(&input)
        .expect("open input")
        .decode()
        .expect("decode input");
    let out_img = ImageReader::open(&output)
        .expect("open output")
        .decode()
        .expect("decode output");

    assert_eq!(in_img.dimensions(), out_img.dimensions());
}

#[test]
fn compress_png_keeps_dimensions_and_reduces_size() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.png");
    let output = dir.path().join("output.png");

    write_png_uncompressed(&input, 256, 256);

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    cmd.args([
        "compress",
        &input.to_string_lossy(),
        &output.to_string_lossy(),
    ])
    .assert()
    .code(0);

    let input_size = fs::metadata(&input).expect("input metadata").len();
    let output_size = fs::metadata(&output).expect("output metadata").len();
    assert!(
        output_size < input_size,
        "expected output smaller, input={input_size}, output={output_size}"
    );

    let in_img = ImageReader::open(&input)
        .expect("open input")
        .decode()
        .expect("decode input");
    let out_img = ImageReader::open(&output)
        .expect("open output")
        .decode()
        .expect("decode output");

    assert_eq!(in_img.dimensions(), out_img.dimensions());
}

#[test]
fn compress_rejects_unsupported_output_extension() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.jpg");
    let output = dir.path().join("output.webp");

    write_jpeg(&input, 100, 80, 90);

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    cmd.args([
        "compress",
        &input.to_string_lossy(),
        &output.to_string_lossy(),
    ])
    .assert()
    .code(1);
}

#[test]
fn compress_jpeg_respects_max_size_target() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.jpg");
    let output = dir.path().join("output.jpg");

    write_jpeg(&input, 400, 300, 95);
    let input_size = fs::metadata(&input).expect("input metadata").len();
    let target_bytes = (input_size / 2).max(1024);
    let target_kb = target_bytes.div_ceil(1024);

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    cmd.args([
        "compress",
        &input.to_string_lossy(),
        &output.to_string_lossy(),
        "--quality",
        "95",
        "--max-size-kb",
        &target_kb.to_string(),
    ])
    .assert()
    .code(0);

    let output_size = fs::metadata(&output).expect("output metadata").len();
    assert!(
        output_size <= target_kb * 1024,
        "expected output <= target, output={output_size}, target={} bytes",
        target_kb * 1024
    );

    let in_img = ImageReader::open(&input)
        .expect("open input")
        .decode()
        .expect("decode input");
    let out_img = ImageReader::open(&output)
        .expect("open output")
        .decode()
        .expect("decode output");
    assert_eq!(in_img.dimensions(), out_img.dimensions());
}

#[test]
fn compress_png_rejects_max_size_target() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.png");
    let output = dir.path().join("output.png");

    write_png_uncompressed(&input, 128, 128);

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    cmd.args([
        "compress",
        &input.to_string_lossy(),
        &output.to_string_lossy(),
        "--max-size-kb",
        "10",
    ])
    .assert()
    .code(1);
}
