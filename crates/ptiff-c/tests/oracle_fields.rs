//! Cross-validation test: `ptiff_open_path_fields` against the C++-produced
//! oracle fixture (`scripts/samples/ptiff_interop_fixture.tif`), which the
//! Go/Python/Ruby bindings consume. Proves the Rust ABI decodes the same
//! `ptiff.*` extension fields (private tags 65001-65005) the C++ oracle
//! writes, in lexicographic order (R4 / Phase 12 cross-validation precursor).

use ptiff_c::metadata::{ptiff_field, ptiff_fields_free, ptiff_open_path_fields};
use std::ffi::CString;
use std::os::raw::{c_char, c_int};

/// Resolves the oracle fixture path relative to this crate.
fn fixture_path() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(manifest)
        .join("../../scripts/samples/ptiff_interop_fixture.tif")
        .canonicalize()
        .expect("oracle fixture exists")
        .to_string_lossy()
        .into_owned()
}

fn read_cstr(p: *mut c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // Safety: p is a NUL-terminated C string owned by us (from the ABI).
    unsafe { std::ffi::CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned()
}

fn read_fields(path: &str) -> Vec<(String, String)> {
    let path_c = CString::new(path).unwrap();
    let mut out: *mut ptiff_field = std::ptr::null_mut();
    let mut out_count: c_int = 0;
    let rc = ptiff_open_path_fields(path_c.as_ptr(), &mut out, &mut out_count);
    assert_eq!(rc, 0, "ptiff_open_path_fields rc for {path:?}");
    assert!(!out.is_null(), "expected fields from oracle fixture");
    // Safety: out is a heap array of out_count ptiff_fields we own.
    let fields = unsafe { std::slice::from_raw_parts(out, out_count as usize) };
    let pairs: Vec<(String, String)> = fields
        .iter()
        .map(|f| (read_cstr(f.key), read_cstr(f.value)))
        .collect();
    ptiff_fields_free(out, out_count);
    pairs
}

#[test]
fn oracle_fixture_decodes_camera_and_spice_fields() {
    let fields = read_fields(&fixture_path());

    let get = |key: &str| {
        fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    };

    // The Go/Python/Ruby binding contract (metadata.go / metadata.py /
    // metadata.rb) — exact flat field names and values the C++ oracle emits.
    let cases = [
        ("ptiff.camera.model", "pinhole"),
        ("ptiff.camera.focal_length_x", "700.0"),
        ("ptiff.camera.focal_length_y", "700.0"),
        ("ptiff.camera.principal_x", "64.000000"),
        ("ptiff.camera.principal_y", "64.000000"),
        ("ptiff.spice.frame", "IAU_MOON"),
    ];
    for (key, want) in cases {
        assert_eq!(get(key).as_deref(), Some(want), "field {key:?}");
    }
}

#[test]
fn oracle_fixture_fields_are_lexicographic() {
    let fields = read_fields(&fixture_path());
    let mut keys: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "fields must be lexicographic by key");
    keys.clear();
}
