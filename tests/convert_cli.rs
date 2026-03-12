use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use assert_cmd::Command;
use image::codecs::jpeg::JpegEncoder;
use image::{GenericImageView, ImageBuffer, ImageEncoder, ImageReader, Rgba};
use tempfile::tempdir;

fn write_png(path: &Path, width: u32, height: u32) {
    let image: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(width, height, Rgba([30, 180, 90, 255]));
    image.save(path).expect("write png");
}

fn write_jpeg(path: &Path, width: u32, height: u32, quality: u8) {
    let mut raw = Vec::with_capacity((width as usize) * (height as usize) * 3);

    for y in 0..height {
        for x in 0..width {
            let r = ((x * 9 + y * 5) % 256) as u8;
            let g = ((x * 17 + y * 3) % 256) as u8;
            let b = ((x * 11 + y * 7) % 256) as u8;
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

#[test]
fn convert_png_to_jpeg_keeps_dimensions() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.png");
    let output = dir.path().join("output.jpg");

    write_png(&input, 128, 96);

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    cmd.args([
        "convert",
        &input.to_string_lossy(),
        &output.to_string_lossy(),
        "--quality",
        "70",
    ])
    .assert()
    .code(0);

    let out = ImageReader::open(&output)
        .expect("open output")
        .decode()
        .expect("decode output");
    assert_eq!(out.dimensions(), (128, 96));
}

#[test]
fn convert_jpeg_to_webp_keeps_dimensions() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.jpg");
    let output = dir.path().join("output.webp");

    write_jpeg(&input, 160, 120, 90);

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    cmd.args([
        "convert",
        &input.to_string_lossy(),
        &output.to_string_lossy(),
    ])
    .assert()
    .code(0);

    let out = ImageReader::open(&output)
        .expect("open output")
        .decode()
        .expect("decode output");
    assert_eq!(out.dimensions(), (160, 120));
}

#[test]
fn convert_rejects_unsupported_output_extension() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.png");
    let output = dir.path().join("output.avif");

    write_png(&input, 64, 64);

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    cmd.args([
        "convert",
        &input.to_string_lossy(),
        &output.to_string_lossy(),
    ])
    .assert()
    .code(1);
}

#[test]
fn convert_to_tiff_keeps_dimensions() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.jpg");
    let output = dir.path().join("output.tiff");

    write_jpeg(&input, 90, 70, 85);

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    cmd.args([
        "convert",
        &input.to_string_lossy(),
        &output.to_string_lossy(),
    ])
    .assert()
    .code(0);

    let out = ImageReader::open(&output)
        .expect("open output")
        .decode()
        .expect("decode output");
    assert_eq!(out.dimensions(), (90, 70));
}
