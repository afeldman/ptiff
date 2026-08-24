//! Crate version, queryable at runtime.
//!
//! Both constants are derived from `Cargo.toml`'s `version` field
//! (`CARGO_PKG_VERSION`) at compile time, so they always match the crate's
//! actual published/build version -- no separate value to keep in sync.

use std::sync::LazyLock;

use semver::Version;

/// The crate version as the raw `Cargo.toml` string (e.g. `"0.1.0"`).
///
/// Use this for display/logging where a parsed [`semver::Version`] isn't
/// needed.
pub const VERSION_STR: &str = env!("CARGO_PKG_VERSION");

/// The crate version as a parsed, comparable [`semver::Version`].
///
/// Use this when you need to compare versions (e.g. `APP_VERSION.major`) or
/// enforce a minimum-version check. Parsing happens once, lazily, on first
/// access.
pub static APP_VERSION: LazyLock<Version> =
    LazyLock::new(|| Version::parse(VERSION_STR).expect("CARGO_PKG_VERSION is not valid SemVer"));
