//! Numeric constants mirroring the PTIFF C ABI surface (the SWIG Python
//! binding exposes the same `PTIFF_PIXEL_*`/`PTIFF_COMPRESSION_*`/`PTIFF_LOG_*`
//! integer ordering). Pinned by the test-suite ordering pins so a future
//! re-numbering is caught.

use pyo3::prelude::*;

/// Monotonic C-ABI break-counter (§7.5). The PyO3 binding is a direct core
/// binding, not the C ABI, but exposing the constant keeps the surface
/// uniform for consumers that branch on it.
pub(crate) const PTIFF_ABI_VERSION: u32 = 1;

/// PTIFF_PIXEL_* sample-type codes (matches the core's `PixelType` discr).
pub(crate) const PTIFF_PIXEL_UINT8: u32 = 0;
pub(crate) const PTIFF_PIXEL_UINT16: u32 = 1;
pub(crate) const PTIFF_PIXEL_UINT32: u32 = 2;
pub(crate) const PTIFF_PIXEL_FLOAT32: u32 = 3;
pub(crate) const PTIFF_PIXEL_FLOAT64: u32 = 4;

/// PTIFF_COMPRESSION_* codes (matches the core's `CompressionKind` discr).
pub(crate) const PTIFF_COMPRESSION_NONE: u32 = 0;
pub(crate) const PTIFF_COMPRESSION_LZW: u32 = 1;
pub(crate) const PTIFF_COMPRESSION_DEFLATE: u32 = 2;
pub(crate) const PTIFF_COMPRESSION_JPEG: u32 = 3;

/// PTIFF_LOG_* level codes (matches the core logger `LogLevel` ordering).
pub(crate) const PTIFF_LOG_TRACE: u32 = 0;
pub(crate) const PTIFF_LOG_DEBUG: u32 = 1;
pub(crate) const PTIFF_LOG_INFO: u32 = 2;
pub(crate) const PTIFF_LOG_WARN: u32 = 3;
pub(crate) const PTIFF_LOG_ERROR: u32 = 4;
pub(crate) const PTIFF_LOG_CRITICAL: u32 = 5;
pub(crate) const PTIFF_LOG_OFF: u32 = 6;

pub(crate) fn register_constants(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("PTIFF_ABI_VERSION", PTIFF_ABI_VERSION)?;
    m.add("PTIFF_PIXEL_UINT8", PTIFF_PIXEL_UINT8)?;
    m.add("PTIFF_PIXEL_UINT16", PTIFF_PIXEL_UINT16)?;
    m.add("PTIFF_PIXEL_UINT32", PTIFF_PIXEL_UINT32)?;
    m.add("PTIFF_PIXEL_FLOAT32", PTIFF_PIXEL_FLOAT32)?;
    m.add("PTIFF_PIXEL_FLOAT64", PTIFF_PIXEL_FLOAT64)?;
    m.add("PTIFF_COMPRESSION_NONE", PTIFF_COMPRESSION_NONE)?;
    m.add("PTIFF_COMPRESSION_LZW", PTIFF_COMPRESSION_LZW)?;
    m.add("PTIFF_COMPRESSION_DEFLATE", PTIFF_COMPRESSION_DEFLATE)?;
    m.add("PTIFF_COMPRESSION_JPEG", PTIFF_COMPRESSION_JPEG)?;
    m.add("PTIFF_LOG_TRACE", PTIFF_LOG_TRACE)?;
    m.add("PTIFF_LOG_DEBUG", PTIFF_LOG_DEBUG)?;
    m.add("PTIFF_LOG_INFO", PTIFF_LOG_INFO)?;
    m.add("PTIFF_LOG_WARN", PTIFF_LOG_WARN)?;
    m.add("PTIFF_LOG_ERROR", PTIFF_LOG_ERROR)?;
    m.add("PTIFF_LOG_CRITICAL", PTIFF_LOG_CRITICAL)?;
    m.add("PTIFF_LOG_OFF", PTIFF_LOG_OFF)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_version_is_initial() {
        assert_eq!(PTIFF_ABI_VERSION, 1);
    }

    #[test]
    fn pixel_codes_are_contiguous() {
        assert_eq!(PTIFF_PIXEL_FLOAT64 - PTIFF_PIXEL_UINT8, 4);
        assert_eq!(PTIFF_COMPRESSION_JPEG, 3);
        assert_eq!(PTIFF_LOG_OFF, 6);
    }
}
