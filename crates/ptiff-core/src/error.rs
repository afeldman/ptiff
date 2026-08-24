//! Recoverable, descriptive error model.
//!
//! This mirrors the C++ `ptiff::Error` / `ptiff::ErrorCode` / `ptiff::Result`
//! (see `libptiff/include/ptiff/core/{error,result}.hpp`) in a Rust-native form:
//! a stable, additive [`ErrorCode`], an [`Error`] carrying a code and a
//! message, and a standard [`Result`] alias.
//!
//! The error model is deliberately small and stable: it is the uniform way the
//! core reports *recoverable* (domain) failures. Contract violations are
//! expressed as panics / invariants, not as `Error` values.

use std::fmt;

/// Coarse, stable category of a recoverable error.
///
/// Mirrors `ptiff::ErrorCode`. Deliberately small and **additive**: once
/// released, new variants are appended and existing ones are never renamed,
/// renumbered, or removed — downstream code may match on them across versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCode {
    /// The requested operation/backend is recognized but not implemented yet.
    NotImplemented,
    /// A provided argument/value is malformed or unsupported.
    InvalidArgument,
    /// An index, offset or coordinate lies outside the legal range.
    OutOfRange,
    /// The requested entity (tile, field, row, ...) does not exist.
    NotFound,
    /// An unspecified failure with no more specific category.
    Unknown,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ErrorCode::NotImplemented => "not implemented",
            ErrorCode::InvalidArgument => "invalid argument",
            ErrorCode::OutOfRange => "out of range",
            ErrorCode::NotFound => "not found",
            ErrorCode::Unknown => "unknown error",
        };
        f.write_str(s)
    }
}

/// A recoverable, descriptive error.
///
/// Mirrors `ptiff::Error`: a stable [`ErrorCode`] category plus a human-readable
/// message. It implements `std::error::Error` so it interoperates with the wider
/// Rust ecosystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    code: ErrorCode,
    message: String,
}

impl Error {
    /// Builds an error from a category and a human-readable description.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Returns the coarse category of this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    /// Returns the human-readable description of this error.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

/// Convenience constructors for the common error categories.
impl Error {
    /// Builds an [`ErrorCode::InvalidArgument`] error.
    #[must_use]
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidArgument, message)
    }

    /// Builds an [`ErrorCode::OutOfRange`] error.
    #[must_use]
    pub fn out_of_range(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::OutOfRange, message)
    }

    /// Builds an [`ErrorCode::NotFound`] error.
    #[must_use]
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    /// Builds an [`ErrorCode::NotImplemented`] error.
    #[must_use]
    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotImplemented, message)
    }

    /// Builds an [`ErrorCode::Unknown`] error.
    #[must_use]
    pub fn unknown(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unknown, message)
    }
}

/// Convenient construction of common errors via `thiserror`.
impl From<ErrorCode> for Error {
    fn from(code: ErrorCode) -> Self {
        Self::new(code, code.to_string())
    }
}

/// `Result<T>` is the standard return type for every fallible public API call
/// in `ptiff-core`: a function either *has* a `T` on success or carries an
/// [`Error`] describing why it failed.
pub type Result<T> = std::result::Result<T, Error>;
