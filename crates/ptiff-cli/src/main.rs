//! `ptiff` — command-line interface for the PTIFF planetary image standard.
//!
//! A thin, idiomatic CLI over the [`ptiff`](https://docs.rs/ptiff) crate (the
//! Rust core's idiomatic frontend). Every subcommand marshals typed core
//! types (`Scene`, `Image`, `ImageDescriptor`, `TileLayout`) through
//! `Tiff::open`, `read_image_pixels`, `tile_layout` and
//! `to_bytes_with_pixels` — no C ABI, no C++.
//!
//! # Command layout
//!
//! ```text
//! ptiff version                 runtime + compile-time version of the core
//! ptiff info <FILE> [--json]    read per-image metadata from a TIFF file
//! ptiff tile-list <FILE>        print the per-image tile grid layout
//! ptiff metadata <FILE>         dump PTIFF camera/CRS extension domains (JSON)
//! ptiff copy <SRC> <DST>        copy every image (metadata + pixels) to a new TIFF
//! ptiff make <W> <H> <PIX> ...  construct and inspect an ImageDescriptor
//! ptiff thumbnail <SRC> <DST>   nearest-neighbour downsample of the first image
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use ptiff::{
    CompressionKind, CoordinateReferenceSystem, ImageDescriptor, ImageDescriptorBuilder, PixelType,
    Projection, ProjectionKind, Scene, Tiff, TileInfo, APP_VERSION, VERSION_STR,
};

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "ptiff",
    version,
    about = "PTIFF planetary image command-line tool",
    long_about = None
)]
struct Cli {
    /// Set the process-wide logger level (accepted for CLI compatibility; the
    /// core emits no process-wide log stream yet).
    #[arg(long, global = true, value_name = "LEVEL")]
    log_level: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show the runtime and compile-time version of the linked PTIFF core.
    Version,
    /// Read a TIFF/BigTIFF file's per-image metadata.
    Info {
        path: PathBuf,
        /// Emit each image's metadata as JSON, including the PTIFF extension
        /// (camera, CRS) domains when present.
        #[arg(long)]
        json: bool,
    },
    /// Print the per-image tile grid (columns x rows) of a TIFF file.
    TileList { path: PathBuf },
    /// Dump the PTIFF camera / CRS extension domains of a TIFF file as JSON.
    Metadata { path: PathBuf },
    /// Copy every image of a TIFF (metadata + pixel payload) to a new file.
    Copy {
        src: PathBuf,
        dst: PathBuf,
        /// Print each image copied to stderr (off by default).
        #[arg(long)]
        verbose: bool,
    },
    /// Construct an `ImageDescriptor` and show its resolved metadata.
    Make(MakeArgs),
    /// Write a nearest-neighbour downsample of the first image to a new TIFF.
    Thumbnail {
        src: PathBuf,
        dst: PathBuf,
        /// Output width in pixels (default: half the source width).
        #[arg(long = "width", value_name = "PIXELS")]
        width: Option<u32>,
        /// Output height in pixels (default: half the source height).
        #[arg(long = "height", value_name = "PIXELS")]
        height: Option<u32>,
    },
}

#[derive(clap::Args)]
struct MakeArgs {
    /// Image width in pixels.
    width: u32,
    /// Image height in pixels.
    height: u32,
    /// Pixel type: uint8, uint16, uint32, float32, float64 (case-insensitive).
    pixel_type: String,
    /// Number of channels (default: 1).
    #[arg(short = 'c', long = "channels", default_value_t = 1)]
    channel_count: u32,
    /// Ground sample distance in meters (optional).
    #[arg(short = 'g', long = "gsd")]
    gsd: Option<f64>,
    /// Tiling in pixels as WxH (optional).
    #[arg(long, value_name = "WxH")]
    tile: Option<String>,
    /// Compression scheme: none, lzw, zip/deflate, jpeg (optional).
    #[arg(short = 'z', long = "compression")]
    compression: Option<String>,
}

/// Top-level CLI error: a human-readable message printed to stderr.
type CliResult = std::result::Result<(), String>;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Version => run_version(),
        Command::Info { path, json } => run_info(&path, json),
        Command::TileList { path } => run_tile_list(&path),
        Command::Metadata { path } => run_metadata(&path),
        Command::Copy { src, dst, verbose } => run_copy(&src, &dst, verbose),
        Command::Make(args) => run_make(args),
        Command::Thumbnail {
            src,
            dst,
            width,
            height,
        } => run_thumbnail(&src, &dst, width, height),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("ptiff: {msg}");
            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// version
// ---------------------------------------------------------------------------

fn run_version() -> CliResult {
    println!(
        "ptiff {VERSION_STR} (core semver {}.{}.{})",
        APP_VERSION.major, APP_VERSION.minor, APP_VERSION.patch
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// info
// ---------------------------------------------------------------------------

fn run_info(path: &PathBuf, as_json: bool) -> CliResult {
    let tiff = Tiff::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if as_json {
        print_info_json(&tiff);
    } else {
        print_info_text(&tiff);
    }
    Ok(())
}

fn print_info_text(tiff: &Tiff) {
    for (i, image) in tiff.images().enumerate() {
        println!(
            "image[{i}]: {}x{} {}x{}",
            image.width(),
            image.height(),
            image.channel_count(),
            image.pixel_type()
        );
        if let Some(g) = image.ground_sample_distance_meters() {
            println!("  gsd:          {g} m");
        }
        match image.tile_info() {
            Some(t) => println!("  tile:         {}x{}", t.tile_width, t.tile_height),
            None => println!("  tile:         (untiled)"),
        }
        match image.compression() {
            Some(c) => println!("  compression:  {}", compression_name(c)),
            None => println!("  compression:  (none)"),
        }
        if image.camera().is_some() {
            println!("  camera:       present");
        }
        if image.crs().is_some() {
            println!("  crs:          present");
        }
    }
}

fn print_info_json(tiff: &Tiff) {
    use serde_json::{json, Value};

    let images: Vec<Value> = tiff
        .images()
        .enumerate()
        .map(|(i, image)| {
            json!({
                "index": i,
                "width": image.width(),
                "height": image.height(),
                "pixel_type": pixel_name(image.pixel_type()),
                "channel_count": image.channel_count(),
                "gsd_meters": image.ground_sample_distance_meters(),
                "tile": image.tile_info().map(|t| json!({
                    "width": t.tile_width,
                    "height": t.tile_height
                })),
                "compression": image.compression().map(compression_name),
                "camera": image.camera().map(camera_to_json),
                "crs": image.crs().map(crs_to_json),
            })
        })
        .collect();

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({ "images": images })).unwrap_or_default()
    );
}

// ---------------------------------------------------------------------------
// tile-list
// ---------------------------------------------------------------------------

fn run_tile_list(path: &PathBuf) -> CliResult {
    let tiff = Tiff::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let n = tiff.images().count();
    println!("{n} image(s)");
    for (i, image) in tiff.images().enumerate() {
        let layout = tiff
            .tile_layout(i)
            .map_err(|e| format!("tile_layout({i}): {e}"))?;
        let cols = layout.columns(0);
        let rows = layout.rows(0);
        println!(
            "image[{i}]: {}x{}px, {}x{} tiles ({} total)",
            image.width(),
            image.height(),
            cols,
            rows,
            cols as u64 * rows as u64
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// metadata
// ---------------------------------------------------------------------------

fn run_metadata(path: &PathBuf) -> CliResult {
    use serde_json::{json, Value};

    let tiff = Tiff::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let images: Vec<Value> = tiff
        .images()
        .enumerate()
        .map(|(i, image)| {
            json!({
                "index": i,
                "width": image.width(),
                "height": image.height(),
                "camera": image.camera().map(camera_to_json),
                "crs": image.crs().map(crs_to_json),
            })
        })
        .collect();

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({ "images": images })).unwrap_or_default()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// copy
// ---------------------------------------------------------------------------

fn run_copy(src: &PathBuf, dst: &PathBuf, verbose: bool) -> CliResult {
    let tiff = Tiff::open(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let n = tiff.images().count();

    // Rebuild the scene from each image's descriptor so the copy preserves
    // dimensions, pixel type, channels, tiling, compression and the PTIFF
    // camera/CRS extension domains. `Image.tile_info()` is not reconstructed
    // by the scene deserializer for a tiled source, so derive the true tile
    // grid from the on-disk `tile_layout` instead.
    let mut scene = Scene::new();
    let mut rasters: Vec<Vec<u8>> = Vec::new();
    for (i, image) in tiff.images().enumerate() {
        let layout = tiff
            .tile_layout(i)
            .map_err(|e| format!("tile_layout({i}): {e}"))?;
        scene
            .add_image(descriptor_of(image, &layout))
            .map_err(|e| format!("add_image({i}): {e}"))?;
        let raster = tiff
            .read_image_pixels(i)
            .map_err(|e| format!("read_image_pixels({i}): {e}"))?;
        if verbose {
            eprintln!(
                "copied image {i}: {}x{} {}x{} ({} raster bytes)",
                image.width(),
                image.height(),
                image.channel_count(),
                image.pixel_type(),
                raster.len()
            );
        }
        rasters.push(raster);
    }

    // The core requires exactly one raster per image, in file order.
    debug_assert_eq!(rasters.len(), n);
    let raster_refs: Vec<&[u8]> = rasters.iter().map(Vec::as_slice).collect();
    let bytes = Tiff::to_bytes_with_pixels(&scene, &raster_refs)
        .map_err(|e| format!("to_bytes_with_pixels: {e}"))?;

    std::fs::write(dst, &bytes).map_err(|e| format!("{}: {e}", dst.display()))?;
    Ok(())
}

/// Reconstruct an [`ImageDescriptor`] from a read [`Image`] so a copy
/// preserves the full metadata surface (extension domains included).
///
/// `image.tile_info()` is the scene's view and is *not* reconstructed for a
/// tiled source (a known core caveat), so the on-disk `layout` supplies the
/// real tile grid when present.
fn descriptor_of(image: &ptiff::Image, layout: &ptiff::TileLayout) -> ImageDescriptor {
    // A TIFF single-strip (untiled) image reports a "layout" whose tile size
    // equals the whole image (width x rows_per_strip) — that is not real
    // tiling. Only surface a tile grid when the stored tile is strictly
    // smaller than the image in at least one axis.
    let tile_info =
        if layout.tile_size.width < image.width() || layout.tile_size.height < image.height() {
            Some(TileInfo::new(
                layout.tile_size.width,
                layout.tile_size.height,
            ))
        } else {
            image.tile_info()
        };
    ImageDescriptorBuilder::new(image.width(), image.height())
        .pixel_type(image.pixel_type())
        .channel_count(image.channel_count())
        .ground_sample_distance_meters(image.ground_sample_distance_meters())
        .tile_info(tile_info)
        .compression(image.compression())
        .camera(image.camera().cloned())
        .crs(image.crs().cloned())
        .build()
}

// ---------------------------------------------------------------------------
// make
// ---------------------------------------------------------------------------

fn run_make(args: MakeArgs) -> CliResult {
    let pixel = parse_pixel_type(&args.pixel_type)?;
    let compression = match args.compression.as_deref() {
        Some(c) => Some(parse_compression(c)?),
        None => None,
    };
    let tile = match args.tile.as_deref() {
        Some(s) => Some(parse_tile(s)?),
        None => None,
    };

    let desc = ImageDescriptorBuilder::new(args.width, args.height)
        .pixel_type(pixel)
        .channel_count(args.channel_count)
        .ground_sample_distance_meters(args.gsd)
        .tile_info(tile)
        .compression(compression)
        .build();

    println!("width:         {}", desc.width);
    println!("height:        {}", desc.height);
    println!("pixel type:    {}", pixel_name(desc.pixel_type));
    println!("channel count: {}", desc.channel_count);
    match desc.ground_sample_distance_meters {
        Some(g) => println!("GSD (m):       {g}"),
        None => println!("GSD (m):       (unset)"),
    }
    match desc.tile_info {
        Some(t) => println!("tile:          {}x{}", t.tile_width, t.tile_height),
        None => println!("tile:          (untiled)"),
    }
    match desc.compression {
        Some(c) => println!("compression:   {}", compression_name(c)),
        None => println!("compression:   (unset)"),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// thumbnail
// ---------------------------------------------------------------------------

fn run_thumbnail(
    src: &PathBuf,
    dst: &PathBuf,
    width: Option<u32>,
    height: Option<u32>,
) -> CliResult {
    let tiff = Tiff::open(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let image0 = tiff
        .images()
        .next()
        .ok_or_else(|| "thumbnail: source has no images".to_string())?;
    let src_w = image0.width();
    let src_h = image0.height();
    if src_w == 0 || src_h == 0 {
        return Err("thumbnail: invalid source dimensions".to_string());
    }
    let channels = image0.channel_count().max(1);
    let bps = image0.pixel_type().bytes_per_sample();
    let samples = channels as usize * bps;
    if samples == 0 {
        return Err("thumbnail: zero sample size".to_string());
    }

    // Resolve output dimensions (default: half the source), clamped to >= 1.
    let out_w = width.unwrap_or(src_w / 2).max(1);
    let out_h = height.unwrap_or(src_h / 2).max(1);

    let raster = tiff
        .read_image_pixels(0)
        .map_err(|e| format!("read_image_pixels(0): {e}"))?;

    let mut out = vec![0u8; (out_w as usize) * (out_h as usize) * samples];
    downsample_nearest(
        &raster,
        src_w as usize,
        src_h as usize,
        samples,
        &mut out,
        out_w as usize,
        out_h as usize,
    );

    // Write a single-image TIFF carrying the downsampled raster, preserving
    // the pixel type and channel count (nearest-neighbour keeps sample layout).
    let desc = ImageDescriptorBuilder::new(out_w, out_h)
        .pixel_type(image0.pixel_type())
        .channel_count(image0.channel_count())
        .build();
    let mut scene = Scene::new();
    scene
        .add_image(desc)
        .map_err(|e| format!("add_image: {e}"))?;
    let bytes = Tiff::to_bytes_with_pixels(&scene, &[&out])
        .map_err(|e| format!("to_bytes_with_pixels: {e}"))?;
    std::fs::write(dst, &bytes).map_err(|e| format!("{}: {e}", dst.display()))?;
    Ok(())
}

/// Nearest-neighbour downsample over a pixel-interleaved raster.
///
/// Each pixel occupies `samples` bytes; the source grid is `src_w`x`src_h`
/// pixels. Output pixel `(ox, oy)` samples source
/// `(floor(ox*src_w/out_w), floor(oy*src_h/out_h))`.
fn downsample_nearest(
    src: &[u8],
    src_w: usize,
    src_h: usize,
    samples: usize,
    out: &mut [u8],
    out_w: usize,
    out_h: usize,
) {
    debug_assert_eq!(src.len(), src_w * src_h * samples);
    debug_assert_eq!(out.len(), out_w * out_h * samples);
    for oy in 0..out_h {
        let sy = oy * src_h / out_h;
        for ox in 0..out_w {
            let sx = ox * src_w / out_w;
            let src_off = (sy * src_w + sx) * samples;
            let dst_off = (oy * out_w + ox) * samples;
            out[dst_off..dst_off + samples].copy_from_slice(&src[src_off..src_off + samples]);
        }
    }
}

// ---------------------------------------------------------------------------
// serialization helpers
// ---------------------------------------------------------------------------

fn camera_to_json(cam: &ptiff::Camera) -> serde_json::Value {
    use serde_json::json;
    let intr = cam.intrinsics();
    let extr = cam.extrinsics();
    json!({
        "model": cam.model_name(),
        "timestamp": cam.timestamp(),
        "intrinsics": {
            "focal_length_x": intr.focal_length_pixels_x,
            "focal_length_y": intr.focal_length_pixels_y,
            "principal_x": intr.principal_point_x,
            "principal_y": intr.principal_point_y,
        },
        "rotation": [
            extr.rotation.w,
            extr.rotation.x,
            extr.rotation.y,
            extr.rotation.z,
        ],
        "position": [
            extr.translation.x,
            extr.translation.y,
            extr.translation.z,
        ],
        "projection_matrix": cam.projection_matrix(),
    })
}

fn crs_to_json(crs: &CoordinateReferenceSystem) -> serde_json::Value {
    use serde_json::json;
    json!({
        "planet": crs.planet().name(),
        "frame": crs.frame().id(),
        "projection": projection_to_json(crs.projection()),
    })
}

fn projection_to_json(p: &Projection) -> serde_json::Value {
    use serde_json::{json, Map, Value};
    let mut params = Map::new();
    for (k, v) in p.iter() {
        params.insert(k.to_string(), Value::from(v));
    }
    json!({
        "kind": projection_name(p.kind()),
        "parameters": params,
    })
}

fn projection_name(k: ProjectionKind) -> &'static str {
    match k {
        ProjectionKind::Equirectangular => "equirectangular",
        ProjectionKind::Stereographic => "stereographic",
        ProjectionKind::Sinusoidal => "sinusoidal",
        ProjectionKind::Orthographic => "orthographic",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------------------
// parsing helpers
// ---------------------------------------------------------------------------

fn parse_pixel_type(s: &str) -> std::result::Result<PixelType, String> {
    match s.to_ascii_lowercase().as_str() {
        "uint8" | "u8" => Ok(PixelType::UInt8),
        "uint16" | "u16" => Ok(PixelType::UInt16),
        "uint32" | "u32" => Ok(PixelType::UInt32),
        "float32" | "f32" | "float" => Ok(PixelType::Float32),
        "float64" | "f64" | "double" => Ok(PixelType::Float64),
        other => Err(format!("unknown pixel type {other:?}")),
    }
}

fn parse_compression(s: &str) -> std::result::Result<CompressionKind, String> {
    match s.to_ascii_lowercase().as_str() {
        "none" => Ok(CompressionKind::None),
        "lzw" => Ok(CompressionKind::Lzw),
        "zip" | "deflate" => Ok(CompressionKind::Deflate),
        "jpeg" => Ok(CompressionKind::Jpeg),
        other => Err(format!("unknown compression {other:?}")),
    }
}

fn parse_tile(s: &str) -> std::result::Result<TileInfo, String> {
    let (w, h) = s
        .split_once('x')
        .ok_or_else(|| format!("invalid tile spec {s:?}: expected WxH"))?;
    let w = w
        .parse::<u32>()
        .map_err(|_| format!("invalid tile width in {s:?}"))?;
    let h = h
        .parse::<u32>()
        .map_err(|_| format!("invalid tile height in {s:?}"))?;
    Ok(TileInfo::new(w, h))
}

fn pixel_name(p: PixelType) -> &'static str {
    match p {
        PixelType::UInt8 => "uint8",
        PixelType::UInt16 => "uint16",
        PixelType::UInt32 => "uint32",
        PixelType::Float32 => "float32",
        PixelType::Float64 => "float64",
        _ => "unknown",
    }
}

fn compression_name(c: CompressionKind) -> &'static str {
    match c {
        CompressionKind::None => "none",
        CompressionKind::Lzw => "lzw",
        CompressionKind::Deflate => "deflate",
        CompressionKind::Jpeg => "jpeg",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_queryable() {
        // The CLI shares the core's runtime-queryable version constants, so
        // `ptiff --version` / `ptiff version` report the *core* version (the
        // version that defines the on-disk format), not just the CLI crate's.
        assert_eq!(VERSION_STR, env!("CARGO_PKG_VERSION"));
        assert_eq!(APP_VERSION.to_string(), VERSION_STR);
        assert_eq!(APP_VERSION.major, 0);
    }

    #[test]
    fn downsample_nearest_shrinks_grid() {
        // 4x2 grid of 1-byte samples -> 2x1 grid; left/right halves collapse.
        let src = vec![0u8, 1, 2, 3, 100, 101, 102, 103];
        let mut out = vec![0u8; 2];
        downsample_nearest(&src, 4, 2, 1, &mut out, 2, 1);
        assert_eq!(out, vec![0, 2]); // (0,0)->0, (1,0)->2
    }

    #[test]
    fn downsample_nearest_preserves_channels() {
        // 2x1 RGB image -> 1x1; takes top-left pixel verbatim.
        let src = vec![10u8, 20, 30, 40, 50, 60];
        let mut out = vec![0u8; 3];
        downsample_nearest(&src, 2, 1, 3, &mut out, 1, 1);
        assert_eq!(out, vec![10, 20, 30]);
    }

    #[test]
    fn pixel_type_parsing() {
        assert_eq!(parse_pixel_type("uint8").unwrap(), PixelType::UInt8);
        assert_eq!(parse_pixel_type("u16").unwrap(), PixelType::UInt16);
        assert_eq!(parse_pixel_type("FLOAT32").unwrap(), PixelType::Float32);
        assert_eq!(parse_pixel_type("u32").unwrap(), PixelType::UInt32);
        assert_eq!(parse_pixel_type("f64").unwrap(), PixelType::Float64);
        assert!(parse_pixel_type("bogus").is_err());
    }

    #[test]
    fn compression_parsing() {
        assert_eq!(parse_compression("lzw").unwrap(), CompressionKind::Lzw);
        assert_eq!(parse_compression("zip").unwrap(), CompressionKind::Deflate);
        assert_eq!(parse_compression("NONE").unwrap(), CompressionKind::None);
        assert!(parse_compression("brotli").is_err());
    }

    #[test]
    fn tile_spec_parsing() {
        assert_eq!(parse_tile("16x16").unwrap(), TileInfo::new(16, 16));
        assert!(parse_tile("16").is_err());
        assert!(parse_tile("axb").is_err());
    }

    #[test]
    fn projection_names_are_stable() {
        assert_eq!(
            projection_name(ProjectionKind::Equirectangular),
            "equirectangular"
        );
        assert_eq!(
            projection_name(ProjectionKind::Stereographic),
            "stereographic"
        );
        assert_eq!(
            projection_name(ProjectionKind::Orthographic),
            "orthographic"
        );
    }

    #[test]
    fn descriptor_of_round_trips_image_metadata() {
        let desc = ImageDescriptorBuilder::new(40, 30)
            .pixel_type(PixelType::UInt8)
            .channel_count(3)
            .ground_sample_distance_meters(Some(2.5))
            .tile(16, 16)
            .compression(Some(CompressionKind::Lzw))
            .build();
        let img = ptiff::Image::new(desc);
        // Untilited on-disk layout: the scene's tile_info is preserved as-is.
        let rebuilt = descriptor_of(&img, &ptiff::TileLayout::default());
        assert_eq!(rebuilt.width, 40);
        assert_eq!(rebuilt.height, 30);
        assert_eq!(rebuilt.pixel_type, PixelType::UInt8);
        assert_eq!(rebuilt.channel_count, 3);
        assert_eq!(rebuilt.ground_sample_distance_meters, Some(2.5));
        assert_eq!(rebuilt.compression, Some(CompressionKind::Lzw));
    }

    #[test]
    fn descriptor_of_overrides_tile_info_from_layout_for_tiled_sources() {
        // The read image loses its tiling in the scene view, but the on-disk
        // layout still knows it; a tiled layout must win.
        let img = ptiff::Image::new(ImageDescriptorBuilder::new(34, 34).build());
        let layout = ptiff::TileLayout::new(ptiff::TileExtent::new(16, 16), 34, 34, 1);
        let rebuilt = descriptor_of(&img, &layout);
        assert_eq!(rebuilt.tile_info, Some(TileInfo::new(16, 16)));
        assert_eq!(rebuilt.width, 34);
        assert_eq!(rebuilt.height, 34);
    }
}
