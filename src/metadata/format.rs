use std::ffi::OsStr;
use std::path::Path;

use image::ImageFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Raster,
    Raw,
}

#[derive(Debug, Clone, Copy)]
pub struct FormatInfo {
    pub kind: FileKind,
    pub mime: &'static str,
    pub canonical_ext: &'static str,
}

pub fn format_info_from_path(path: &Path) -> Option<FormatInfo> {
    let ext = path
        .extension()
        .and_then(OsStr::to_str)
        .map(|value| value.to_ascii_lowercase())?;

    let info = match ext.as_str() {
        "jpg" | "jpeg" => FormatInfo {
            kind: FileKind::Raster,
            mime: "image/jpeg",
            canonical_ext: "jpg",
        },
        "png" => FormatInfo {
            kind: FileKind::Raster,
            mime: "image/png",
            canonical_ext: "png",
        },
        "webp" => FormatInfo {
            kind: FileKind::Raster,
            mime: "image/webp",
            canonical_ext: "webp",
        },
        "tiff" | "tif" => FormatInfo {
            kind: FileKind::Raster,
            mime: "image/tiff",
            canonical_ext: "tiff",
        },
        "bmp" => FormatInfo {
            kind: FileKind::Raster,
            mime: "image/bmp",
            canonical_ext: "bmp",
        },
        "gif" => FormatInfo {
            kind: FileKind::Raster,
            mime: "image/gif",
            canonical_ext: "gif",
        },
        // Sony RAW
        "arw" => FormatInfo {
            kind: FileKind::Raw,
            mime: "image/x-sony-arw",
            canonical_ext: "arw",
        },
        "sr2" => FormatInfo {
            kind: FileKind::Raw,
            mime: "image/x-sony-sr2",
            canonical_ext: "sr2",
        },
        "srf" => FormatInfo {
            kind: FileKind::Raw,
            mime: "image/x-sony-srf",
            canonical_ext: "srf",
        },
        // Nikon RAW
        "nef" => FormatInfo {
            kind: FileKind::Raw,
            mime: "image/x-nikon-nef",
            canonical_ext: "nef",
        },
        "nrw" => FormatInfo {
            kind: FileKind::Raw,
            mime: "image/x-nikon-nrw",
            canonical_ext: "nrw",
        },
        // Canon RAW
        "cr2" => FormatInfo {
            kind: FileKind::Raw,
            mime: "image/x-canon-cr2",
            canonical_ext: "cr2",
        },
        "cr3" => FormatInfo {
            kind: FileKind::Raw,
            mime: "image/x-canon-cr3",
            canonical_ext: "cr3",
        },
        "crw" => FormatInfo {
            kind: FileKind::Raw,
            mime: "image/x-canon-crw",
            canonical_ext: "crw",
        },
        // Common digital negative RAW
        "dng" => FormatInfo {
            kind: FileKind::Raw,
            mime: "image/x-adobe-dng",
            canonical_ext: "dng",
        },
        _ => return None,
    };

    Some(info)
}

pub fn has_supported_extension(path: &Path) -> bool {
    format_info_from_path(path).is_some()
}

pub fn is_supported_raster_format(format: ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::Jpeg
            | ImageFormat::Png
            | ImageFormat::WebP
            | ImageFormat::Tiff
            | ImageFormat::Bmp
            | ImageFormat::Gif
    )
}

pub fn raster_format_mime_and_ext(format: ImageFormat) -> Option<(&'static str, &'static str)> {
    let value = match format {
        ImageFormat::Jpeg => ("image/jpeg", "jpg"),
        ImageFormat::Png => ("image/png", "png"),
        ImageFormat::WebP => ("image/webp", "webp"),
        ImageFormat::Tiff => ("image/tiff", "tiff"),
        ImageFormat::Bmp => ("image/bmp", "bmp"),
        ImageFormat::Gif => ("image/gif", "gif"),
        _ => return None,
    };

    Some(value)
}
