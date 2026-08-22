//! Golden-file tests (§11.3).
//!
//! These lock the **byte-exact** serialized output of the deterministic TIFF
//! writer behind a fixed SHA-256 digest, mirroring the C++ oracle's golden
//! discipline (`libptiff/tests/golden/tiff_roundtrip_golden_test.cpp`).
//!
//! Rules (from §11.3):
//!   - Lossless codecs + uncompressed: golden = exact bytes (digest in code).
//!   - JPEG: pixel-tolerance only (not here; covered by the codec round-trip
//!     tests), because pure-Rust JPEG is not byte-identical to libjpeg-turbo.
//!   - The digest is bumped ONLY on a deliberate, reviewed format change.
//!
//! Rust is the reference implementation; these digests lock the Rust writer's
//! deterministic bytes. (Cross-read with the C++/image-rs oracle is verified
//! by the existing interop tests, not by byte-equality with a C++ writer that
//! may legitimately differ in field ordering.)
//!
//! To regenerate the digests (only after a reviewed format change):
//!   `PTIFF_GOLDEN_REGEN=1 cargo test -p ptiff-core --test golden`

use ptiff_core::io::backend::tiff::TiffBackend;
use ptiff_core::io::StorageBackend;
use ptiff_core::io::{MemoryBinaryWriter, StorageModel};
use ptiff_core::tile::{Tile, TileExtent, TileIndex, TileRegion};
use ptiff_core::TileId;
use sha2::{Digest, Sha256};

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// A deterministic single-image StripModel (16×16, UInt8, 1 channel).
fn strip_model(width: u32, height: u32, pixel_type: &str, compression: &str) -> StorageModel {
    let mut m = StorageModel::new();
    m.set_field("imageWidth", width.to_string());
    m.set_field("imageHeight", height.to_string());
    m.set_field("samplesPerPixel", "1");
    m.set_field("pixelType", pixel_type);
    m.set_field("compression", compression);
    m
}

/// Deterministic payload: a repeating ramp (period 251) so the bytes are
/// fixed, but not trivially uniform.
fn ramp_payload(width: u32, height: u32) -> Vec<u8> {
    (0..(width as usize * height as usize))
        .map(|i| (i % 251) as u8)
        .collect()
}

/// Serializes a single-image model plus its payload into bytes via the tiff
/// backend (header + IFD + one strip).
fn serialize_single_image(model: &StorageModel, payload: &[u8]) -> Vec<u8> {
    let backend = TiffBackend;
    let mut writer = MemoryBinaryWriter::new();
    backend
        .serialize_model(model, &mut writer)
        .expect("serialize");

    let mut sink = backend
        .open_image_sink(&mut writer, model)
        .expect("open sink");
    let layout = sink.layout();
    let region = TileRegion::new(
        0,
        0,
        TileExtent::new(layout.tile_size.width, layout.tile_size.height),
    );
    let tile = Tile::new(TileId::new(0), TileIndex::new(0, 0, 0), region, payload);
    sink.write_tile(&tile).expect("write tile");
    drop(sink);

    writer.take_buffer()
}

/// Serializes a model carrying PTIFF extension metadata through the private
/// tags (65001–65005). No pixel payload is written (matches `serialize_model`).
fn serialize_metadata_only(model: &StorageModel) -> Vec<u8> {
    let backend = TiffBackend;
    let mut writer = MemoryBinaryWriter::new();
    backend
        .serialize_model(model, &mut writer)
        .expect("serialize");
    writer.take_buffer()
}

/// Whether the caller asked to regenerate (rather than assert) the golden
/// digests. Follows the C++ oracle's "empty digest ⇒ warn" idea, but uses an
/// explicit env flag so the lock can never silently loosen.
fn regenerate() -> bool {
    std::env::var("PTIFF_GOLDEN_REGEN").as_deref() == Ok("1")
}

/// Asserts `actual`'s digest equals the locked golden `expected`, printing (and
/// optionally letting the caller see) the value for regeneration.
fn check_golden(name: &str, expected: &str, bytes: &[u8]) {
    let digest = sha256_hex(bytes);
    if regenerate() {
        eprintln!("PTIFF_GOLDEN[{name}] = {digest}");
        eprintln!("! Regeneration active; not asserting.");
        return;
    }
    assert_eq!(
        digest, expected,
        "golden digest mismatch for `{name}`; bump the constant only on a deliberate format change"
    );
}

// ---------------------------------------------------------------------------
// Golden digests (SHA-256, §11.3). Locked 2026-08-22. Bump ONLY on a
// deliberate, reviewed format change (like the C++ `kGolden*Sha256`).
// ---------------------------------------------------------------------------
const GOLDEN_UNCOMPRESSED_16X16: &str =
    "305ccc4a52a55bad1473fae1416b26d5f34f3489731284f4f40267a357b32f10";
const GOLDEN_PACKBITS_16X16: &str =
    "c6ddda80b143dacc7c25f7f80d91b51a6fd2f371ba3a7ac8cdfdaf2acd699ae6";
const GOLDEN_LZW_16X16: &str = "317642bb0d80d1a364dd0e8c6a30b857520b0ec29275a82460e8bf24f73ed5f8";
const GOLDEN_PTIFF_TAGS: &str = "db06570734702f56aca38affc5395e15d3835e482d8852e750fc06211c16577c";
const GOLDEN_CAMERA_CRS_TAGS: &str =
    "efbd0121761d7cdcbea029397f11aed6c5fd9782aa3b159f0c954b6e427e52e1";

#[test]
fn golden_uncompressed_16x16() {
    let model = strip_model(16, 16, "UInt8", "None");
    let bytes = serialize_single_image(&model, &ramp_payload(16, 16));
    check_golden("uncompressed_16x16", GOLDEN_UNCOMPRESSED_16X16, &bytes);
}

#[test]
fn golden_packbits_16x16() {
    let model = strip_model(16, 16, "UInt8", "PackBits");
    let bytes = serialize_single_image(&model, &ramp_payload(16, 16));
    check_golden("packbits_16x16", GOLDEN_PACKBITS_16X16, &bytes);
}

#[test]
fn golden_lzw_16x16() {
    let model = strip_model(16, 16, "UInt8", "LZW");
    let bytes = serialize_single_image(&model, &ramp_payload(16, 16));
    check_golden("lzw_16x16", GOLDEN_LZW_16X16, &bytes);
}

#[test]
fn golden_ptiff_metadata_tags() {
    // Deterministic storage model exercising the container-level private tags
    // (65001–65005), mirroring the C++ `kGoldenPtiffTagsSha256` scenario.
    let mut model = StorageModel::new();
    model.set_field("imageWidth", "16");
    model.set_field("imageHeight", "16");
    model.set_field("samplesPerPixel", "1");
    model.set_field("pixelType", "UInt8");
    model.set_field("ptiff.spice.frame", "IAU_MOON");
    model.set_field("ptiff.spice.time_system", "TDB");
    model.set_field("ptiff.layers.dem", "dem");
    model.set_field("ptiff.provenance.software", "libptiff");
    let bytes = serialize_metadata_only(&model);
    check_golden("ptiff_tags", GOLDEN_PTIFF_TAGS, &bytes);
}

#[test]
fn golden_camera_and_crs_tags() {
    // The RFC-provisional typed camera + CRS schema (Punkt 2) serialized
    // through the private camera (65002) / CRS (65003) tags.
    let mut model = StorageModel::new();
    model.set_field("imageWidth", "16");
    model.set_field("imageHeight", "16");
    model.set_field("samplesPerPixel", "1");
    model.set_field("pixelType", "UInt8");
    model.set_field("ptiff.camera.model", "pinhole");
    model.set_field("ptiff.camera.timestamp", "12345.0");
    model.set_field("ptiff.camera.focal_px", "100.0");
    model.set_field("ptiff.camera.focal_py", "100.0");
    model.set_field("ptiff.camera.principal_x", "8.0");
    model.set_field("ptiff.camera.principal_y", "8.0");
    model.set_field("ptiff.camera.rot_w", "1.0");
    model.set_field("ptiff.camera.rot_x", "0.0");
    model.set_field("ptiff.camera.rot_y", "0.0");
    model.set_field("ptiff.camera.rot_z", "0.0");
    model.set_field("ptiff.camera.pos_x", "0.0");
    model.set_field("ptiff.camera.pos_y", "0.0");
    model.set_field("ptiff.camera.pos_z", "0.0");
    model.set_field("ptiff.crs.planet_name", "Moon");
    model.set_field("ptiff.crs.planet_iau_id", "301");
    model.set_field("ptiff.crs.planet_semi_major_m", "1737400.0");
    model.set_field("ptiff.crs.planet_semi_minor_m", "1737400.0");
    model.set_field("ptiff.crs.frame", "IAU_MOON");
    model.set_field("ptiff.crs.projection", "equirectangular");
    model.set_field("ptiff.crs.param.central_meridian", "0.0");
    let bytes = serialize_metadata_only(&model);
    check_golden("camera_crs_tags", GOLDEN_CAMERA_CRS_TAGS, &bytes);
}

// ---------------------------------------------------------------------------
// Cross-oracle: the same files our golden digests lock are also decodeable by
// image-rs' independent `tiff` crate with the *same* pixels. This ties the
// byte-exact golden to an external compatibility check (§11.2 Binary/Kompression).
// ---------------------------------------------------------------------------
#[test]
fn golden_files_cross_read_with_image_rs_oracle() {
    use std::io::Cursor;
    use tiff::decoder::{Decoder, DecodingResult};

    fn decode(file: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut dec = Decoder::new(Cursor::new(file)).expect("tiff crate opens stream");
        let (w, h) = dec.dimensions().expect("dimensions");
        let img = dec.read_image().expect("decode image");
        let bytes = match img {
            DecodingResult::U8(v) => v,
            other => panic!("expected U8 samples, got {other:?}"),
        };
        (w, h, bytes)
    }

    let payload = ramp_payload(16, 16);

    for compression in ["None", "PackBits", "LZW"] {
        let model = strip_model(16, 16, "UInt8", compression);
        let bytes = serialize_single_image(&model, &payload);
        let (w, h, samples) = decode(&bytes);
        assert_eq!((w, h), (16, 16), "{compression}: dims");
        assert_eq!(samples, payload, "{compression}: pixel bytes");
    }

    // Metadata-only files (no pixel payload) must still expose the PTIFF
    // extension tags to the oracle's unknown-tag handling without erroring.
    let mut tags = StorageModel::new();
    tags.set_field("imageWidth", "16");
    tags.set_field("imageHeight", "16");
    tags.set_field("samplesPerPixel", "1");
    tags.set_field("pixelType", "UInt8");
    tags.set_field("ptiff.camera.model", "pinhole");
    let bytes = serialize_metadata_only(&tags);
    let mut dec = Decoder::new(Cursor::new(bytes)).expect("tiff crate opens tags file");
    let (w, h) = dec.dimensions().expect("tags file dims");
    assert_eq!((w, h), (16, 16));
}
