use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use image::{ImageBuffer, Rgba};
use serde_json::Value;
use tempfile::tempdir;

fn write_png(path: &Path, width: u32, height: u32) {
    let image: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(width, height, Rgba([120, 30, 220, 255]));
    image.save(path).expect("write png");
}

fn run_json_expect_code(args: &[String], expected_code: i32) -> Value {
    let mut cmd = Command::cargo_bin("imgtool-rs").expect("binary exists");
    let assert = cmd.args(args).assert().code(expected_code);
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    serde_json::from_str(&stdout).expect("valid json output")
}

fn required_top_level_fields(record: &Value) {
    for key in [
        "path",
        "status",
        "error",
        "warnings",
        "file_size_bytes",
        "modified_at",
        "mime",
        "extension",
        "width",
        "height",
        "pixel_count",
        "channels",
        "bit_depth",
        "has_alpha",
        "orientation",
        "color_space",
        "capture",
    ] {
        assert!(
            record.get(key).is_some(),
            "missing required key `{key}` in {record}"
        );
    }
}

#[test]
fn single_png_without_exif_returns_basic_metadata() {
    let dir = tempdir().expect("tempdir");
    let png = dir.path().join("sample.png");
    write_png(&png, 4, 3);

    let args = vec![
        "meta".to_string(),
        png.to_string_lossy().to_string(),
        "--json".to_string(),
    ];
    let json = run_json_expect_code(&args, 0);

    assert_eq!(json["summary"]["total"], 1);
    assert_eq!(json["summary"]["succeeded"], 1);
    assert_eq!(json["summary"]["failed"], 0);

    let record = &json["records"][0];
    required_top_level_fields(record);
    assert_eq!(record["status"], "ok");
    assert_eq!(record["mime"], "image/png");
    assert_eq!(record["width"], 4);
    assert_eq!(record["height"], 3);
    assert_eq!(record["pixel_count"], 12);
    assert_eq!(record["capture"], Value::Null);
}

#[test]
fn exif_fixture_exposes_capture_fields() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("exif_sample.jpg");

    let args = vec![
        "meta".to_string(),
        fixture.to_string_lossy().to_string(),
        "--json".to_string(),
    ];
    let json = run_json_expect_code(&args, 0);
    let record = &json["records"][0];

    assert_eq!(record["status"], "ok");
    assert_eq!(record["mime"], "image/jpeg");
    assert_eq!(record["capture"]["camera_make"], "Canon");
    assert_eq!(record["capture"]["camera_model"], "EOS R5");
    assert_eq!(record["capture"]["lens_model"], "RF24-70mm F2.8 L IS USM");
    assert!(record["capture"]["datetime_original"].is_string());
    assert!(record["capture"]["exposure_time"].is_string());
    assert!(record["capture"]["f_number"].is_string());
    assert!(record["capture"]["iso"].is_string());
    assert!(record["capture"]["focal_length"].is_string());
    assert!(record["orientation"].is_string());
    assert!(record["color_space"].is_string());

    let latitude = record["capture"]["gps"]["latitude"]
        .as_f64()
        .expect("gps latitude");
    let longitude = record["capture"]["gps"]["longitude"]
        .as_f64()
        .expect("gps longitude");
    assert!((latitude - 31.2304).abs() < 0.01, "latitude={latitude}");
    assert!((longitude - 121.4737).abs() < 0.01, "longitude={longitude}");
}

#[test]
fn no_gps_flag_removes_gps_payload() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("exif_sample.jpg");

    let args = vec![
        "meta".to_string(),
        fixture.to_string_lossy().to_string(),
        "--json".to_string(),
        "--no-gps".to_string(),
    ];
    let json = run_json_expect_code(&args, 0);
    let record = &json["records"][0];

    assert!(record["capture"].is_object());
    assert_eq!(record["capture"]["gps"], Value::Null);
}

#[test]
fn directory_scan_skips_hidden_by_default() {
    let dir = tempdir().expect("tempdir");
    let visible = dir.path().join("visible.png");
    let hidden = dir.path().join(".hidden.png");
    write_png(&visible, 2, 2);
    write_png(&hidden, 2, 2);

    let args_default = vec![
        "meta".to_string(),
        dir.path().to_string_lossy().to_string(),
        "--json".to_string(),
    ];
    let json_default = run_json_expect_code(&args_default, 0);
    assert_eq!(json_default["summary"]["total"], 1);

    let args_include_hidden = vec![
        "meta".to_string(),
        dir.path().to_string_lossy().to_string(),
        "--json".to_string(),
        "--include-hidden".to_string(),
    ];
    let json_include_hidden = run_json_expect_code(&args_include_hidden, 0);
    assert_eq!(json_include_hidden["summary"]["total"], 2);
}

#[test]
fn mixed_directory_continues_and_summarizes_errors() {
    let dir = tempdir().expect("tempdir");
    write_png(&dir.path().join("image.png"), 3, 3);
    fs::write(dir.path().join("note.txt"), "hello").expect("write text");
    fs::write(dir.path().join("broken.jpg"), [1_u8, 2, 3, 4, 5]).expect("write broken");

    let args = vec![
        "meta".to_string(),
        dir.path().to_string_lossy().to_string(),
        "--json".to_string(),
    ];
    let json = run_json_expect_code(&args, 2);

    assert_eq!(json["summary"]["total"], 2);
    assert_eq!(json["summary"]["succeeded"], 1);
    assert_eq!(json["summary"]["failed"], 1);
    assert_eq!(json["summary"]["aborted"], false);

    let records = json["records"].as_array().expect("records array");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["status"], "ok");
}

#[test]
fn fail_fast_stops_at_first_error() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("a.jpg"), [1_u8, 2, 3, 4, 5]).expect("write broken");
    write_png(&dir.path().join("b.png"), 2, 2);

    let args = vec![
        "meta".to_string(),
        dir.path().to_string_lossy().to_string(),
        "--json".to_string(),
        "--fail-fast".to_string(),
    ];
    let json = run_json_expect_code(&args, 1);

    assert_eq!(json["summary"]["total"], 1);
    assert_eq!(json["summary"]["succeeded"], 0);
    assert_eq!(json["summary"]["failed"], 1);
    assert_eq!(json["summary"]["aborted"], true);
    assert_eq!(json["records"].as_array().expect("records").len(), 0);
}

#[test]
fn invalid_extension_single_file_only_updates_summary() {
    let dir = tempdir().expect("tempdir");
    let file = dir.path().join("note.txt");
    fs::write(&file, "hello").expect("write text");

    let args = vec![
        "meta".to_string(),
        file.to_string_lossy().to_string(),
        "--json".to_string(),
    ];
    let json = run_json_expect_code(&args, 2);

    assert_eq!(json["summary"]["total"], 1);
    assert_eq!(json["summary"]["succeeded"], 0);
    assert_eq!(json["summary"]["failed"], 1);
    assert_eq!(json["records"].as_array().expect("records").len(), 0);
}

#[test]
fn sony_raw_extension_is_supported_via_exif_metadata() {
    let dir = tempdir().expect("tempdir");
    let raw = dir.path().join("sample.arw");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("exif_sample.jpg");
    fs::copy(fixture, &raw).expect("copy fixture");

    let args = vec![
        "meta".to_string(),
        raw.to_string_lossy().to_string(),
        "--json".to_string(),
    ];
    let json = run_json_expect_code(&args, 0);

    assert_eq!(json["summary"]["total"], 1);
    assert_eq!(json["summary"]["succeeded"], 1);
    assert_eq!(json["summary"]["failed"], 0);

    let record = &json["records"][0];
    assert_eq!(record["mime"], "image/x-sony-arw");
    assert_eq!(record["width"], 2);
    assert_eq!(record["height"], 2);
    assert_eq!(record["capture"]["camera_make"], "Canon");
}

#[test]
fn broken_raw_file_is_counted_as_failure_without_record() {
    let dir = tempdir().expect("tempdir");
    let raw = dir.path().join("broken.nef");
    fs::write(&raw, [1_u8, 2, 3, 4, 5]).expect("write broken raw");

    let args = vec![
        "meta".to_string(),
        raw.to_string_lossy().to_string(),
        "--json".to_string(),
    ];
    let json = run_json_expect_code(&args, 2);

    assert_eq!(json["summary"]["total"], 1);
    assert_eq!(json["summary"]["succeeded"], 0);
    assert_eq!(json["summary"]["failed"], 1);
    assert_eq!(json["records"].as_array().expect("records").len(), 0);
}
