//! Scalar storage type of one channel sample.
//!
//! Mirrors `ptiff::PixelType` (see `libptiff/include/ptiff/image/pixel_type.hpp`).

use std::fmt;

/// The scalar type of one channel sample.
///
/// Identifies the numeric storage type of each pixel sample in an image.
/// Multi-channel images share a single [`PixelType`] across all channels.
///
/// This list is **additive** once released: existing variants are never renamed,
/// renumbered or removed, so downstream code may match on them across versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum PixelType {
    /// Unsigned 8-bit integer sample.
    UInt8,
    /// Unsigned 16-bit integer sample.
    UInt16,
    /// Unsigned 32-bit integer sample.
    UInt32,
    /// IEEE-754 32-bit floating-point sample.
    Float32,
    /// IEEE-754 64-bit floating-point sample.
    Float64,
}

impl PixelType {
    /// Number of bytes per sample.
    #[must_use]
    pub const fn bytes_per_sample(self) -> usize {
        match self {
            PixelType::UInt8 => 1,
            PixelType::UInt16 => 2,
            PixelType::UInt32 => 4,
            PixelType::Float32 => 4,
            PixelType::Float64 => 8,
        }
    }

    /// Whether this type is an integer sample.
    #[must_use]
    pub const fn is_integer(self) -> bool {
        matches!(
            self,
            PixelType::UInt8 | PixelType::UInt16 | PixelType::UInt32
        )
    }

    /// Whether this type is a floating-point sample.
    #[must_use]
    pub const fn is_float(self) -> bool {
        matches!(self, PixelType::Float32 | PixelType::Float64)
    }
}

impl fmt::Display for PixelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PixelType::UInt8 => "uint8",
            PixelType::UInt16 => "uint16",
            PixelType::UInt32 => "uint32",
            PixelType::Float32 => "float32",
            PixelType::Float64 => "float64",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_per_sample() {
        assert_eq!(PixelType::UInt8.bytes_per_sample(), 1);
        assert_eq!(PixelType::UInt16.bytes_per_sample(), 2);
        assert_eq!(PixelType::UInt32.bytes_per_sample(), 4);
        assert_eq!(PixelType::Float32.bytes_per_sample(), 4);
        assert_eq!(PixelType::Float64.bytes_per_sample(), 8);
    }

    #[test]
    fn integer_and_float_classification() {
        for t in [PixelType::UInt8, PixelType::UInt16, PixelType::UInt32] {
            assert!(t.is_integer());
            assert!(!t.is_float());
        }
        for t in [PixelType::Float32, PixelType::Float64] {
            assert!(!t.is_integer());
            assert!(t.is_float());
        }
    }

    #[test]
    fn display() {
        assert_eq!(PixelType::UInt8.to_string(), "uint8");
        assert_eq!(PixelType::Float64.to_string(), "float64");
    }
}
