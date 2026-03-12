use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use exif::{In, Rational, Reader as ExifReader, SRational, Tag, Value};

use super::model::{CaptureInfo, GpsInfo};

#[derive(Debug)]
pub struct ExifBundle {
    pub orientation: Option<String>,
    pub color_space: Option<String>,
    pub capture: Option<CaptureInfo>,
    pub dimensions: Option<(u32, u32)>,
}

pub fn extract_exif_bundle(path: &Path, include_gps: bool) -> Result<Option<ExifBundle>, String> {
    let file = File::open(path).map_err(|err| err.to_string())?;
    let mut reader = BufReader::new(file);
    let exif = ExifReader::new()
        .read_from_container(&mut reader)
        .map_err(|err| err.to_string())?;

    let camera_make = field_text(&exif, Tag::Make);
    let camera_model = field_text(&exif, Tag::Model);
    let lens_model = field_text(&exif, Tag::LensModel);
    let datetime_original = field_text(&exif, Tag::DateTimeOriginal);
    let exposure_time = field_text(&exif, Tag::ExposureTime);
    let f_number = field_text(&exif, Tag::FNumber);
    let iso = field_text(&exif, Tag::PhotographicSensitivity)
        .or_else(|| field_text(&exif, Tag::ISOSpeed));
    let focal_length = field_text(&exif, Tag::FocalLength);
    let gps = if include_gps { parse_gps(&exif) } else { None };

    let capture_has_data = camera_make.is_some()
        || camera_model.is_some()
        || lens_model.is_some()
        || datetime_original.is_some()
        || exposure_time.is_some()
        || f_number.is_some()
        || iso.is_some()
        || focal_length.is_some()
        || gps.is_some();

    let capture = if capture_has_data {
        Some(CaptureInfo {
            camera_make,
            camera_model,
            lens_model,
            datetime_original,
            exposure_time,
            f_number,
            iso,
            focal_length,
            gps,
        })
    } else {
        None
    };

    let orientation = field_text(&exif, Tag::Orientation);
    let color_space = field_text(&exif, Tag::ColorSpace);
    let dimensions = parse_dimensions(&exif);

    if orientation.is_none() && color_space.is_none() && capture.is_none() && dimensions.is_none() {
        return Ok(None);
    }

    Ok(Some(ExifBundle {
        orientation,
        color_space,
        capture,
        dimensions,
    }))
}

pub fn is_no_exif_error(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("no exif") || message.contains("not found")
}

fn parse_dimensions(exif: &exif::Exif) -> Option<(u32, u32)> {
    let width = field_u32(exif, Tag::PixelXDimension).or_else(|| field_u32(exif, Tag::ImageWidth));

    let height =
        field_u32(exif, Tag::PixelYDimension).or_else(|| field_u32(exif, Tag::ImageLength));

    match (width, height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => Some((width, height)),
        _ => None,
    }
}

fn field_text(exif: &exif::Exif, tag: Tag) -> Option<String> {
    exif.get_field(tag, In::PRIMARY)
        .map(|field| field.display_value().with_unit(exif).to_string())
        .and_then(normalize_text)
}

fn field_u32(exif: &exif::Exif, tag: Tag) -> Option<u32> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    match &field.value {
        Value::Byte(values) if !values.is_empty() => Some(values[0] as u32),
        Value::Short(values) if !values.is_empty() => Some(values[0] as u32),
        Value::Long(values) if !values.is_empty() => Some(values[0]),
        Value::SByte(values) if !values.is_empty() => u32::try_from(values[0]).ok(),
        Value::SShort(values) if !values.is_empty() => u32::try_from(values[0]).ok(),
        Value::SLong(values) if !values.is_empty() => u32::try_from(values[0]).ok(),
        Value::Rational(values) if !values.is_empty() => {
            rational_to_f64(values[0]).map(|v| v as u32)
        }
        Value::SRational(values) if !values.is_empty() => {
            srational_to_f64(values[0]).and_then(|v| if v >= 0.0 { Some(v as u32) } else { None })
        }
        _ => None,
    }
}

fn parse_gps(exif: &exif::Exif) -> Option<GpsInfo> {
    let latitude = parse_gps_coordinate(exif, Tag::GPSLatitude, Tag::GPSLatitudeRef);
    let longitude = parse_gps_coordinate(exif, Tag::GPSLongitude, Tag::GPSLongitudeRef);
    let altitude = parse_gps_altitude(exif);

    if latitude.is_none() && longitude.is_none() && altitude.is_none() {
        return None;
    }

    Some(GpsInfo {
        latitude,
        longitude,
        altitude_m: altitude,
    })
}

fn parse_gps_coordinate(exif: &exif::Exif, coord_tag: Tag, ref_tag: Tag) -> Option<f64> {
    let coord_field = exif.get_field(coord_tag, In::PRIMARY)?;

    let values = match &coord_field.value {
        Value::Rational(values) if values.len() >= 3 => values,
        _ => return None,
    };

    let d = rational_to_f64(values[0])?;
    let m = rational_to_f64(values[1])?;
    let s = rational_to_f64(values[2])?;

    let mut decimal = d + m / 60.0 + s / 3600.0;
    if let Some(ref_value) = field_text(exif, ref_tag) {
        let first = ref_value.chars().next().unwrap_or_default();
        if matches!(first, 'S' | 'W' | 's' | 'w') {
            decimal *= -1.0;
        }
    }

    Some(decimal)
}

fn parse_gps_altitude(exif: &exif::Exif) -> Option<f64> {
    let altitude_field = exif.get_field(Tag::GPSAltitude, In::PRIMARY)?;
    let base = match &altitude_field.value {
        Value::Rational(values) if !values.is_empty() => rational_to_f64(values[0])?,
        _ => return None,
    };

    let below_sea_level = exif
        .get_field(Tag::GPSAltitudeRef, In::PRIMARY)
        .and_then(|field| match field.value {
            Value::Byte(ref bytes) if !bytes.is_empty() => Some(bytes[0] == 1),
            _ => None,
        })
        .unwrap_or(false);

    Some(if below_sea_level { -base } else { base })
}

fn rational_to_f64(rational: Rational) -> Option<f64> {
    if rational.denom == 0 {
        return None;
    }
    Some(rational.num as f64 / rational.denom as f64)
}

fn srational_to_f64(rational: SRational) -> Option<f64> {
    if rational.denom == 0 {
        return None;
    }
    Some(rational.num as f64 / rational.denom as f64)
}

fn normalize_text(raw: String) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(trimmed);

    if unquoted.is_empty() {
        None
    } else {
        Some(unquoted.to_string())
    }
}
