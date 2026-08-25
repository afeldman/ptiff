//! End-to-end CLI behavior tests: spawn the real `ptiff` binary against a
//! temp TIFF written through the `ptiff` crate, and assert on both the exit
//! status and the parsed output (text / JSON / re-read pixels).
//!
//! `CARGO_BIN_EXE_ptiff` is supplied by Cargo for integration tests in this
//! `tests/` directory, so these are genuine CLI-invocation checks.

use std::path::{Path, PathBuf};
use std::process::Command;

use ptiff::{
    CoordinateReferenceSystem, Ellipsoid, Extrinsics, ImageDescriptorBuilder, Intrinsics,
    PixelType, Projection, ProjectionKind, Quaternion, Scene, Tiff, Vec3, VERSION_STR,
};

const BIN: &str = env!("CARGO_BIN_EXE_ptiff");

/// Builds a two-image TIFF: a UInt8 grayscale image (with a camera + CRS) and
/// a UInt16 RGB image. Returns the path of the written file inside `dir`.
fn write_fixture(dir: &Path) -> PathBuf {
    let camera = ptiff::Camera::from_model(
        "pinhole",
        Intrinsics::new(700.0, 715.0, 32.0, 24.0),
        Extrinsics::new(
            Quaternion::new(1.0, 0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 10.0),
        ),
        "2026-08-21T12:34:56.000Z",
    );
    let crs = CoordinateReferenceSystem::new(
        ptiff::Planet::new(
            "Moon",
            "301",
            Ellipsoid::UNSPECIFIED,
            ptiff::Frame::IAU_MOON,
        ),
        None,
        Projection::new(ProjectionKind::Sinusoidal),
    );

    let mut scene = Scene::new();
    let img0 = ImageDescriptorBuilder::new(40, 30)
        .camera(Some(camera))
        .crs(Some(crs))
        .build();
    scene.add_image(img0).unwrap();
    let img1 = ImageDescriptorBuilder::new(20, 16)
        .pixel_type(PixelType::UInt16)
        .channel_count(3)
        .build();
    scene.add_image(img1).unwrap();

    // Raster 0: 40*30 gray bytes (values = index for inspection).
    let raster0: Vec<u8> = (0..40 * 30).map(|i| (i % 256) as u8).collect();
    // Raster 1: 20*16*3 UInt16 (2 bytes each) = 1920 bytes.
    let raster1: Vec<u8> = (0..20 * 16 * 3 * 2).map(|i| (i % 256) as u8).collect();

    let bytes = Tiff::to_bytes_with_pixels(&scene, &[&raster0, &raster1]).unwrap();
    let path = dir.join("fixture.ptiff");
    std::fs::write(&path, bytes).unwrap();
    path
}

fn ptiff(args: &[&str]) -> std::process::Output {
    Command::new(BIN).args(args).output().expect("spawn ptiff")
}

#[test]
fn version_subcommand_reports_core_version() {
    let out = ptiff(&["version"]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("ptiff "));
    assert!(text.contains(VERSION_STR));
}

#[test]
fn info_text_reports_images_and_extension_domains() {
    let dir = tempfile::tempdir().unwrap();
    let file = write_fixture(dir.path());
    let out = ptiff(&["info", file.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("image[0]: 40x30"));
    assert!(text.contains("image[1]: 20x16"));
    assert!(text.contains("3x")); // channel count of image[1]
    assert!(text.contains("camera:       present"));
    assert!(text.contains("crs:          present"));
}

#[test]
fn info_json_carries_image_and_extension_fields() {
    let dir = tempfile::tempdir().unwrap();
    let file = write_fixture(dir.path());
    let out = ptiff(&["info", "--json", file.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("info --json parses");
    let images = v["images"].as_array().expect("images array");
    assert_eq!(images.len(), 2);
    assert_eq!(images[0]["width"], 40);
    assert_eq!(images[0]["height"], 30);
    assert_eq!(images[0]["pixel_type"], "uint8");
    assert_eq!(images[1]["pixel_type"], "uint16");
    assert_eq!(images[1]["channel_count"], 3);
    assert_eq!(images[0]["camera"]["model"], "pinhole");
    assert_eq!(images[0]["crs"]["planet"], "Moon");
}

#[test]
fn tile_list_reports_grid() {
    let dir = tempfile::tempdir().unwrap();
    let file = write_fixture(dir.path());
    let out = ptiff(&["tile-list", file.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("2 image(s)"));
    assert!(text.contains("image[0]: 40x30px, 1x1 tiles"));
    assert!(text.contains("image[1]: 20x16px, 1x1 tiles"));
}

#[test]
fn metadata_json_dumps_camera_and_crs() {
    let dir = tempfile::tempdir().unwrap();
    let file = write_fixture(dir.path());
    let out = ptiff(&["metadata", file.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("metadata parses");
    let images = v["images"].as_array().unwrap();
    assert_eq!(images[0]["camera"]["model"], "pinhole");
    assert_eq!(images[0]["camera"]["timestamp"], "2026-08-21T12:34:56.000Z");
    assert_eq!(images[0]["crs"]["planet"], "Moon");
    assert_eq!(images[0]["crs"]["projection"]["kind"], "sinusoidal");
}

#[test]
fn copy_produces_a_pixel_identical_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = write_fixture(dir.path());
    let dst = dir.path().join("copy.ptiff");
    let out = ptiff(&["copy", file.to_str().unwrap(), dst.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let src_tiff = Tiff::open(&file).unwrap();
    let dst_tiff = Tiff::open(&dst).unwrap();
    assert_eq!(dst_tiff.images().count(), 2);
    for i in 0..2 {
        assert_eq!(
            dst_tiff.read_image_pixels(i).unwrap(),
            src_tiff.read_image_pixels(i).unwrap()
        );
    }
}

#[test]
fn copy_preserves_single_oversized_tile_of_small_pyramid_level() {
    // Regression for `ptiff copy` on a tiled image whose last pyramid level is
    // a single padded tile *larger* than the image in every axis (42x222 in a
    // 256x256 tile, like the top level of a real 616-tile LRO-NAC pyramid).
    // `read_image_pixels` returns the full 256x256 block, so the copy must
    // surface that on-disk grid rather than reconstructing a 42x222 tile that
    // mismatches `to_bytes_with_pixels`' expected raster length.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("small_pyr_level.ptiff");
    let mut scene = Scene::new();
    scene
        .add_image(ImageDescriptorBuilder::new(42, 222).tile(256, 256).build())
        .unwrap();
    // 1x1 grid of 256x256 tiles = 65536 bytes.
    let raster: Vec<u8> = (0..256 * 256).map(|i| (i % 256) as u8).collect();
    let bytes = Tiff::to_bytes_with_pixels(&scene, &[&raster]).unwrap();
    std::fs::write(&src, bytes).unwrap();

    let dst = dir.path().join("copy.ptiff");
    let out = ptiff(&["copy", src.to_str().unwrap(), dst.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "copy failed: stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let src_tiff = Tiff::open(&src).unwrap();
    let dst_tiff = Tiff::open(&dst).unwrap();
    assert_eq!(dst_tiff.images().count(), 1);
    assert_eq!(
        dst_tiff.read_image_pixels(0).unwrap(),
        src_tiff.read_image_pixels(0).unwrap()
    );
    // The copied descriptor must keep the 256x256 tiling so a second copy
    // (and the benchmark write path) round-trips identically.
    assert_eq!(
        dst_tiff.tile_layout(0).unwrap().tile_size,
        ptiff::TileExtent::new(256, 256)
    );
}

#[test]
fn thumbnail_downscales_the_first_image() {
    let dir = tempfile::tempdir().unwrap();
    let file = write_fixture(dir.path());
    let dst = dir.path().join("thumb.ptiff");
    let out = ptiff(&[
        "thumbnail",
        file.to_str().unwrap(),
        dst.to_str().unwrap(),
        "--width",
        "20",
        "--height",
        "15",
    ]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let tiff = Tiff::open(&dst).unwrap();
    assert_eq!(tiff.images().count(), 1);
    let img = tiff.images().next().unwrap();
    assert_eq!(img.width(), 20);
    assert_eq!(img.height(), 15);
    assert_eq!(img.pixel_type(), PixelType::UInt8);
    // The downsampled raster has exactly out_w * out_h grayscale samples.
    assert_eq!(tiff.read_image_pixels(0).unwrap().len(), 20 * 15);
}

#[test]
fn tile_kind_lists_a_real_grid_for_a_tiled_image() {
    let dir = tempfile::tempdir().unwrap();
    // 34x34 image with 16x16 tiles -> a 3x3 grid (padded edge tiles). The
    // raster handed to to_bytes_with_pixels is the concatenated grid: each of
    // the 9 tiles is 16x16 samples, so 3*16 * 3*16 = 2304 sample bytes (the
    // same layout read_image_pixels later reports back).
    let mut scene = Scene::new();
    let desc = ImageDescriptorBuilder::new(34, 34).tile(16, 16).build();
    scene.add_image(desc).unwrap();
    let raster: Vec<u8> = (0..2304).map(|i| (i % 256) as u8).collect();
    let bytes = Tiff::to_bytes_with_pixels(&scene, &[&raster]).unwrap();
    let file = dir.path().join("tiled.ptiff");
    std::fs::write(&file, bytes).unwrap();

    let out = ptiff(&["tile-list", file.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("1 image(s)"));
    assert!(text.contains("image[0]: 34x34px, 3x3 tiles (9 total)"));

    // Copy must round-trip a *tiled* input pixel-identically.
    let dst = dir.path().join("tiled-copy.ptiff");
    let out = ptiff(&["copy", file.to_str().unwrap(), dst.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        Tiff::open(&dst).unwrap().read_image_pixels(0).unwrap(),
        Tiff::open(&file).unwrap().read_image_pixels(0).unwrap()
    );
}
