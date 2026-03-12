use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use assert_cmd::Command;
use image::codecs::jpeg::JpegEncoder;
use image::{GenericImageView, ImageBuffer, ImageEncoder, ImageReader, Rgba};
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

fn write_png(path: &Path, width: u32, height: u32) {
    let image: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(width, height, Rgba([20, 120, 220, 255]));
    image.save(path).expect("save png");
}

#[test]
fn batch_resize_and_compress_pipeline_preserves_structure() {
    let dir = tempdir().expect("tempdir");
    let input_root = dir.path().join("input");
    let output_root = dir.path().join("output");
    let input_file = input_root.join("nested").join("sample.jpg");

    fs::create_dir_all(input_file.parent().expect("parent")).expect("mkdir input");
    write_jpeg(&input_file, 300, 200, 95);

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    cmd.args([
        "batch",
        &input_root.to_string_lossy(),
        &output_root.to_string_lossy(),
        "--resize-scale",
        "0.5",
        "--compress",
        "--quality",
        "40",
    ])
    .assert()
    .code(0);

    let output_file = output_root.join("nested").join("sample.jpg");
    assert!(output_file.exists(), "output file should exist");

    let output_img = ImageReader::open(&output_file)
        .expect("open output")
        .decode()
        .expect("decode output");
    assert_eq!(output_img.dimensions(), (150, 100));

    let input_size = fs::metadata(&input_file).expect("input metadata").len();
    let output_size = fs::metadata(&output_file).expect("output metadata").len();
    assert!(
        output_size < input_size,
        "expected output smaller, input={input_size}, output={output_size}"
    );
}

#[test]
fn batch_compress_honors_max_size_target_for_jpeg() {
    let dir = tempdir().expect("tempdir");
    let input_root = dir.path().join("input");
    let output_root = dir.path().join("output");
    let input_file = input_root.join("sample.jpg");

    fs::create_dir_all(&input_root).expect("mkdir input");
    write_jpeg(&input_file, 400, 300, 95);

    let input_size = fs::metadata(&input_file).expect("input metadata").len();
    let target_bytes = (input_size / 2).max(1024);
    let target_kb = target_bytes.div_ceil(1024);

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    cmd.args([
        "batch",
        &input_root.to_string_lossy(),
        &output_root.to_string_lossy(),
        "--compress",
        "--quality",
        "95",
        "--max-size-kb",
        &target_kb.to_string(),
    ])
    .assert()
    .code(0);

    let output_file = output_root.join("sample.jpg");
    let output_size = fs::metadata(&output_file).expect("output metadata").len();
    assert!(
        output_size <= target_kb * 1024,
        "expected output <= target, output={output_size}, target={} bytes",
        target_kb * 1024
    );
}

#[test]
fn batch_fail_fast_stops_after_first_failure() {
    let dir = tempdir().expect("tempdir");
    let input_root = dir.path().join("input");
    let output_root = dir.path().join("output");

    fs::create_dir_all(&input_root).expect("mkdir input");
    write_png(&input_root.join("a.png"), 120, 80);
    write_jpeg(&input_root.join("b.jpg"), 120, 80, 90);

    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    let assert = cmd
        .args([
            "batch",
            &input_root.to_string_lossy(),
            &output_root.to_string_lossy(),
            "--compress",
            "--max-size-kb",
            "10",
            "--fail-fast",
        ])
        .assert()
        .code(1);

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    assert!(stdout.contains("failed=1"), "stdout={stdout}");
    assert!(stdout.contains("aborted=true"), "stdout={stdout}");

    assert!(
        !output_root.join("b.jpg").exists(),
        "b.jpg should not be processed after fail-fast abort"
    );
}
