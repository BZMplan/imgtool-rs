use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessOutput {
    pub records: Vec<MetadataRecord>,
    pub summary: BatchSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchSummary {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub aborted: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RecordStatus {
    Ok,
    Error,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetadataRecord {
    pub path: String,
    pub status: RecordStatus,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub file_size_bytes: Option<u64>,
    pub modified_at: Option<String>,
    pub mime: Option<String>,
    pub extension: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub pixel_count: Option<u64>,
    pub channels: Option<u8>,
    pub bit_depth: Option<u8>,
    pub has_alpha: Option<bool>,
    pub orientation: Option<String>,
    pub color_space: Option<String>,
    pub capture: Option<CaptureInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CaptureInfo {
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub datetime_original: Option<String>,
    pub exposure_time: Option<String>,
    pub f_number: Option<String>,
    pub iso: Option<String>,
    pub focal_length: Option<String>,
    pub gps: Option<GpsInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GpsInfo {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude_m: Option<f64>,
}
