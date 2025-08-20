use std::fmt;

use crate::spec::enums::MediaType;

/// Error returned when we cannot determine the media type.
#[derive(Debug)]
pub struct DetectError;

impl fmt::Display for DetectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown or unsupported archive media type")
    }
}

impl std::error::Error for DetectError {}

/// Detect a media type for a buffer (works with `&[u8]` and `&Bytes`).
///
/// Detection rules:
/// - If the buffer starts with gzip magic (0x1F 0x8B) => TarGz
/// - If the buffer starts with zstd frame magic (0x28 B5 2F FD) => TarZstd
/// - If the buffer contains the tar "ustar" magic at offset 257 => Tar
/// - Otherwise returns an error.
///
/// Note: this function only inspects headers/magic bytes; it does not fully validate
/// that a gzip/zstd stream actually contains a tar archive.
pub fn detect_media_type<B: AsRef<[u8]>>(buf: B) -> Result<MediaType, DetectError> {
    let data = buf.as_ref();

    // gzip magic: 0x1F 0x8B
    if data.len() >= 2 && data[0] == 0x1F && data[1] == 0x8B {
        return Ok(MediaType::OciImageLayerV1TarGzip);
    }

    // zstd magic (frame header): 0x28 B5 2F FD (little-endian)
    if data.len() >= 4 && data[0] == 0x28 && data[1] == 0xB5 && data[2] == 0x2F && data[3] == 0xFD {
        return Ok(MediaType::OciImageLayerV1TarZstd);
    }

    // tar ustar magic at offset 257 (POSIX tar)
    if data.len() > 257 + 5 && &data[257..257 + 5] == b"ustar" {
        return Ok(MediaType::OciImageLayerV1Tar);
    }

    Err(DetectError)
}
