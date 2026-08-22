//! Overflow-checked arithmetic for the TIFF parser.
//!
//! Mirrors `ptiff::io::backend::tiff::checked_arithmetic.hpp`.

use crate::{Error, Result};

/// Upper bound on the number of elements a single tag-array / strip-tile table
/// may claim from a file. Guards against resource-exhaustion from crafted
/// counts (RFC-0001 §13). Real baseline TIFF structures never come close; this
/// only bounds hostile input.
pub const K_MAX_TAG_COUNT: u64 = 64 * 1024 * 1024; // 64M

/// Adds `a` + `b`, rejecting overflow instead of wrapping.
///
/// TIFF metadata (offsets, counts, lengths) comes from untrusted file contents,
/// so any byte-size or file-position arithmetic derived from it MUST be checked
/// against integer overflow before use (RFC-0001 §13). Returns
/// [`crate::ErrorCode::InvalidArgument`] on overflow so the caller can reject
/// the malformed file instead of silently wrapping.
pub fn checked_add_u64(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or_else(|| Error::invalid_argument("checked_add_u64: unsigned addition overflows"))
}

/// Multiplies `a` * `b`, rejecting overflow instead of wrapping.
pub fn checked_mul_u64(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(|| {
        Error::invalid_argument("checked_mul_u64: unsigned multiplication overflows")
    })
}

/// Adds `a` + `b` (`usize`), rejecting overflow instead of wrapping.
pub fn checked_add_size(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| Error::invalid_argument("checked_add_size: size addition overflows"))
}

/// Multiplies `a` * `b` (`usize`), rejecting overflow instead of wrapping.
pub fn checked_mul_size(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| Error::invalid_argument("checked_mul_size: size multiplication overflows"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addition_ok_and_overflow() {
        assert_eq!(checked_add_u64(1, 2).unwrap(), 3);
        assert!(checked_add_u64(u64::MAX, 1).is_err());
        assert!(checked_add_size(usize::MAX, 1).is_err());
    }

    #[test]
    fn multiplication_ok_and_overflow() {
        assert_eq!(checked_mul_u64(6, 7).unwrap(), 42);
        assert!(checked_mul_u64(u64::MAX, 2).is_err());
        assert!(checked_mul_size(usize::MAX, 2).is_err());
    }

    #[test]
    fn zero_multiplication_never_overflows() {
        assert_eq!(checked_mul_u64(0, u64::MAX).unwrap(), 0);
        assert_eq!(checked_mul_size(0, usize::MAX).unwrap(), 0);
    }

    #[test]
    fn max_tag_count_is_sane() {
        assert_eq!(K_MAX_TAG_COUNT, 64 * 1024 * 1024);
    }
}
