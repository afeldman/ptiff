//! Systematic cross-validation against the C++-Oracle writer (Phase 12).
//!
//! The fixtures in `crates/ptiff-core/tests/data/oracle/` are produced by the
//! C++ reference implementation (`libptiff`, backup in `../ptiff_back`) via
//! `scripts/gen_interop_fixture`: a tiled `UInt8` TIFF whose pixel payload is
//! the deterministic gradient `(x*3 + y*5) % 256`, and — for the `_meta`
//! variant — the PTIFF extension domains (camera/CRS, private tags 65002 and
//! 65003) plus generic `ptiff.*` records (SPICE/layers/provenance, tags
//! 65001/65004/65005).
//!
//! These tests prove the **read** half of the Phase-12 cross-validation
//! (§11.2 «Binary / Metadata / Pixel / Round-Trip»): the Rust core decodes
//! byte streams produced by the C++-Oracle into the same semantic content.
//! The complementary **write** half — standard readers opening Rust-written
//! files — is exercised by `tests`/`golden.rs` and `scripts/interop.sh`
//! (tiffinfo / gdalinfo / Pillow / OpenCV against Rust-written PTIFFs).
//!
//! **Fixture provenance note:** the checked-in fixtures were written by the
//! compiled `gen_interop_fixture` binary (an earlier `libptiff-0.3.0`-line
//! build; the checked-in `ptiff_back/scripts/gen_interop_fixture.cpp` source
//! has since bumped the embedded `ptiff.provenance.software` to `0.4.0`). The
//! `provenance.software` value below therefore pins `libptiff-0.3.0`, which is
//! consistent with the frozen `scripts/samples/ptiff_interop_fixture.tif`
//! (the oracle the Go/Python/Ruby bindings and `crates/ptiff-c/tests/
//! oracle_fields.rs` target). If the oracle fixtures are regenerated from
//! `gen_interop_fixture.cpp` v0.4.0, update that expectation accordingly.

use ptiff::{Image, PixelType, ProjectionKind, Tiff};

/// C++-Oracle gradient: `value(x, y) = (x*3 + y*5) % 256` (row-major).
fn oracle_gradient(width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height];
    for y in 0..height {
        for x in 0..width {
            out[y * width + x] = ((x * 3 + y * 5) % 256) as u8;
        }
    }
    out
}

fn fixture(name: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(manifest)
        .join("../ptiff-core/tests/data/oracle")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn decoded_pixels(t: &Tiff, index: usize) -> Vec<u8> {
    t.read_image_pixels(index).expect("decode oracle pixels")
}

#[test]
fn oracle_meta_pixels_match_cpp_gradient() {
    let t = Tiff::open(fixture("oracle_fixture_meta.tif")).expect("open oracle meta");

    // Dimensions from the C++ model (imageWidth/Height = 128).
    let img: &Image = t.image(0).expect("image 0");
    assert_eq!(img.width(), 128);
    assert_eq!(img.height(), 128);
    assert_eq!(img.pixel_type(), PixelType::UInt8);
    assert_eq!(img.channel_count(), 1);

    // Pixel payload → must equal the deterministic (x*3 + y*5) % 256 gradient
    // byte-for-byte, proving the Rust reader decodes the C++ byte stream
    // identically (§11.2 «Pixel comparison»).
    assert_eq!(decoded_pixels(&t, 0), oracle_gradient(128, 128));
}

#[test]
fn oracle_meta_camera_and_crs_decoded() {
    let t = Tiff::open(fixture("oracle_fixture_meta.tif")).expect("open oracle meta");
    let img: &Image = t.image(0).expect("image 0");

    // Camera (private tag 65002) — pinhole, 700 px focal, principal point 64,64.
    let camera = img.camera().expect("camera present");
    assert_eq!(camera.model_name(), "pinhole");
    let intr = camera.intrinsics();
    assert_eq!(intr.focal_length_pixels_x, 700.0);
    assert_eq!(intr.focal_length_pixels_y, 700.0);
    assert_eq!(intr.principal_point_x, 64.0);
    assert_eq!(intr.principal_point_y, 64.0);

    // CRS (private tag 65003) — NAIF body 301 = Moon, equirectangular.
    let crs = img.crs().expect("crs present");
    assert_eq!(crs.planet().name(), "Moon");
    assert_eq!(crs.projection().kind(), ProjectionKind::Equirectangular);
}

#[test]
fn oracle_meta_exposes_generic_ptiff_fields() {
    let t = Tiff::open(fixture("oracle_fixture_meta.tif")).expect("open oracle meta");
    let img: &Image = t.image(0).expect("image 0");

    // Generic `ptiff.<domain>.<key>` records from the C++ oracle
    // (SPICE 65001 / layers 65004 / provenance 65005).
    assert_eq!(img.metadata_value("ptiff.spice.frame"), Some("IAU_MOON"));
    assert_eq!(
        img.metadata_value("ptiff.spice.instrument"),
        Some("LROC_NAC")
    );
    assert_eq!(
        img.metadata_value("ptiff.provenance.software"),
        Some("libptiff-0.3.0")
    );
    // The class-based camera/CRS fields are NOT re-exposed in the generic map.
    assert_eq!(img.metadata_value("ptiff.camera.model"), None);
}

#[test]
fn oracle_nometa_has_no_extension_domains() {
    let t = Tiff::open(fixture("oracle_fixture_nometa.tif")).expect("open oracle nometa");
    let img: &Image = t.image(0).expect("image 0");

    assert_eq!(img.width(), 128);
    assert_eq!(img.height(), 128);
    assert!(img.camera().is_none(), "no camera without ptiff fields");
    assert!(img.crs().is_none(), "no crs without ptiff fields");
    assert!(img.metadata().is_empty(), "no generic ptiff.* fields");

    // Same deterministic gradient as the meta variant.
    assert_eq!(decoded_pixels(&t, 0), oracle_gradient(128, 128));
}

/// Round-trip write half: a Rust-written TIFF carrying the same camera + CRS
/// must decode to the same typed camera/CRS the C++ oracle encoded (the write
/// side of the Phase-12 cross-validation against the shared schema).
#[test]
fn rust_round_trip_reproduces_oracle_extension_schema() {
    use ptiff::{Camera, Extrinsics, ImageDescriptorBuilder, Intrinsics, Scene, TileInfo, Vec3};

    let mut scene = Scene::new();
    let builder = ImageDescriptorBuilder::new(128, 128)
        .pixel_type(PixelType::UInt8)
        .channel_count(1)
        .tile_info(Some(TileInfo::new(64, 64)))
        .camera(Some(Camera::from_model(
            "pinhole",
            Intrinsics::new(700.0, 700.0, 64.0, 64.0),
            Extrinsics::new(ptiff::Quaternion::IDENTITY, Vec3::new(0.0, 0.0, 0.0)),
            "",
        )));
    scene
        .add_image(builder.build())
        .expect("add oracle-shaped image");

    let bytes = Tiff::to_bytes_with_pixels(&scene, &[oracle_gradient(128, 128).as_slice()])
        .expect("serialize rust oracle-shaped tiff");

    let t = Tiff::from_bytes(&bytes).expect("reopen rust-written tiff");
    let img: &Image = t.image(0).expect("image 0");
    assert_eq!(img.width(), 128);
    assert_eq!(img.height(), 128);
    assert_eq!(decoded_pixels(&t, 0), oracle_gradient(128, 128));

    let camera = img.camera().expect("camera survives rust round-trip");
    assert_eq!(camera.model_name(), "pinhole");
    let intr = camera.intrinsics();
    assert_eq!(intr.focal_length_pixels_x, 700.0);
    assert_eq!(intr.principal_point_x, 64.0);
}
