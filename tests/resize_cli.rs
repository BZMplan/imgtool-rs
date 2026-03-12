use std::path::Path;

use assert_cmd::Command;
use image::{GenericImageView, ImageBuffer, ImageReader, Rgba};
use tempfile::tempdir;

fn write_png(path: &Path, width: u32, height: u32) {
    let image: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(width, height, Rgba([10, 120, 200, 255]));
    image.save(path).expect("write png");
}

#[test]
fn resize_command_changes_image_dimensions() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("in.png");
    let output = dir.path().join("out.png");
    write_png(&input, 12, 8);

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    cmd.args([
        "resize",
        &input.to_string_lossy(),
        &output.to_string_lossy(),
        "--scale",
        "0.5",
        "--filter",
        "triangle",
    ])
    .assert()
    .code(0);

    let decoded = ImageReader::open(&output)
        .expect("open output")
        .decode()
        .expect("decode output");
    let (width, height) = decoded.dimensions();
    assert_eq!(width, 6);
    assert_eq!(height, 4);
}

#[test]
fn resize_command_rejects_invalid_input_extension() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("in.txt");
    let output = dir.path().join("out.png");
    std::fs::write(&input, "hello").expect("write text");

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    cmd.args([
        "resize",
        &input.to_string_lossy(),
        &output.to_string_lossy(),
        "--scale",
        "1.0",
    ])
    .assert()
    .code(1);
}

#[test]
fn resize_command_rejects_zero_scale() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("in.png");
    let output = dir.path().join("out.png");
    write_png(&input, 10, 10);

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    cmd.args([
        "resize",
        &input.to_string_lossy(),
        &output.to_string_lossy(),
        "--scale",
        "0",
    ])
    .assert()
    .code(1);
}
