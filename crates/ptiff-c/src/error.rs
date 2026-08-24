//! FFI-safe error plumbing.
//!
//! Mirrors the original C ABI error surface (the `ptiff_error_code` enum is a
//! stable, additive list whose ordering must match `ptiff::ErrorCode` (which
//! itself mirrors the C++ `ptiff::ErrorCode`). Every fallible C-ABI operation
//! returns a negative value on failure (`-ptiff_error_code`) and `0` on
//! success, matching the existing `libptiff_c` contract so foreign runtimes
//! (Go, Ruby, Python, Octave) can switch on the error codes unchanged.

use ptiff::{Error, ErrorCode};

/// Mirror of the C `ptiff_error_code` enum (see the generated `ptiff_c.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum ptiff_error_code {
    /// The requested operation/backend is recognized but not implemented yet.
    PTIFF_ERROR_NOT_IMPLEMENTED = 0,
    /// A provided argument/value is malformed or unsupported.
    PTIFF_ERROR_INVALID_ARGUMENT = 1,
    /// An index, offset or coordinate lies outside the legal range.
    PTIFF_ERROR_OUT_OF_RANGE = 2,
    /// The requested entity (tile, field, row, ...) does not exist.
    PTIFF_ERROR_NOT_FOUND = 3,
    /// An unspecified failure with no more specific category.
    PTIFF_ERROR_UNKNOWN = 4,
}

impl From<ErrorCode> for ptiff_error_code {
    fn from(code: ErrorCode) -> Self {
        match code {
            // The C enum ordering intentionally matches `ptiff::ErrorCode`
            // (documented in `ptiff_error.h`).
            ErrorCode::NotImplemented => ptiff_error_code::PTIFF_ERROR_NOT_IMPLEMENTED,
            ErrorCode::InvalidArgument => ptiff_error_code::PTIFF_ERROR_INVALID_ARGUMENT,
            ErrorCode::OutOfRange => ptiff_error_code::PTIFF_ERROR_OUT_OF_RANGE,
            ErrorCode::NotFound => ptiff_error_code::PTIFF_ERROR_NOT_FOUND,
            // The core is additive; anything new maps to the generic catch-all.
            _ => ptiff_error_code::PTIFF_ERROR_UNKNOWN,
        }
    }
}

/// Maps a core [`Error`] to a negative C error code, discarding the message
/// (the C-ABI surface only carries the coarse code; the headless `_fields` /
/// `_camera` entry points that need detail are built on top separately).
pub fn to_c_error(err: &Error) -> i32 {
    -(ptiff_error_code::from(err.code()) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ptiff::{Error, ErrorCode};

    #[test]
    fn error_code_ordering_matches_c_header() {
        // The ABI pins these indices (see the generated `ptiff_c.h`); the
        // mapping must not drift, because foreign runtimes switch on the raw integers.
        assert_eq!(ErrorCode::NotImplemented as i32, 0);
        assert_eq!(ErrorCode::InvalidArgument as i32, 1);
        assert_eq!(ErrorCode::OutOfRange as i32, 2);
        assert_eq!(ErrorCode::NotFound as i32, 3);
        assert_eq!(ErrorCode::Unknown as i32, 4);
    }

    #[test]
    fn to_c_error_is_negative_and_ordered() {
        let e = Error::new(ErrorCode::OutOfRange, "index out of range");
        assert_eq!(to_c_error(&e), -2);
        let e = Error::invalid_argument("bad");
        assert_eq!(to_c_error(&e), -1);
    }
}
