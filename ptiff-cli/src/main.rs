//! `ptiff` — command-line interface for the PTIFF planetary image standard.
//!
//! A thin CLI over the idiomatic Rust binding (`ptiff`, see `bindings/rust`),
//! which in turn talks only to the language-agnostic C ABI `libptiff_c`. The
//! CLI never links libptiff's C++ API directly.
//!
//! # Command layout
//!
//! ```text
//! ptiff version                 runtime + compile-time version of libptiff
//! ptiff backends                registered backend names
//! ptiff logger level            current logger level
//! ptiff logger set <LEVEL>      set the process-wide logger level
//! ptiff info <FILE>             read metadata from an on-disk TIFF file
//! ptiff make <W> <H> <PIX> ...  inspect a constructed ImageDescriptor
//! ptiff copy <SRC> <DST>        tile-copy the primary image to a new TIFF
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use ptiff::{
    backend_names, compile_time_version, open_path_camera, read_metadata, runtime_version, Camera,
    FileMetadata, LogLevel, Logger, Sink, Source,
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
    /// Set the process-wide logger level before running the command.
    #[arg(long, global = true, value_name = "LEVEL")]
    log_level: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show the runtime and compile-time version of the linked libptiff.
    Version,
    /// List the backend names registered in the linked libptiff.
    Backends,
    /// Read a TIFF/BigTIFF file's primary image metadata from disk.
    Info {
        path: PathBuf,
        /// Emit the metadata as JSON, including the PTIFF private-tag (65001-65005)
        /// fields grouped by domain under a top-level "ptiff" object.
        #[arg(long)]
        json: bool,
    },
    /// Construct an ImageDescriptor and show its resolved metadata.
    Make(MakeArgs),
    /// Copy the primary image's pixel tiles from one TIFF to another.
    Copy {
        src: PathBuf,
        dst: PathBuf,
        /// Print each tile read/written on stderr (off by default).
        #[arg(long)]
        verbose: bool,
    },
    /// Query or set the process-wide logger level.
    Logger(LoggerArgs),
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
    /// Tile width in pixels (optional, sets tiling).
    #[arg(long = "tile-width")]
    tile_width: Option<u32>,
    /// Tile height in pixels (optional, sets tiling).
    #[arg(long = "tile-height")]
    tile_height: Option<u32>,
    /// Compression: none, lzw, deflate, jpeg (optional).
    #[arg(short = 'x', long = "compression")]
    compression: Option<String>,
}

#[derive(clap::Args)]
struct LoggerArgs {
    #[command(subcommand)]
    command: LoggerCommand,
}

#[derive(Subcommand)]
enum LoggerCommand {
    /// Print the current logger level.
    Level,
    /// Set the process-wide logger level.
    Set { level: String },
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    let cli = Cli::parse();
    // Apply a global --log-level first so any libptiff logging emitted while
    // executing the command respects the requested filter (same process).
    if let Some(level) = &cli.log_level {
        match parse_log_level(level) {
            Ok(lv) => Logger::instance().set_level(lv),
            Err(msg) => {
                eprintln!("ptiff: error: {msg}");
                return ExitCode::FAILURE;
            }
        }
    }
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("ptiff: error: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn run(cmd: Command) -> Result<(), String> {
    match cmd {
        Command::Version => {
            println!("runtime:      {}", runtime_version());
            println!("compile-time: {}", compile_time_version());
            Ok(())
        }
        Command::Backends => {
            let names = backend_names();
            if names.is_empty() {
                println!("(no backends registered)");
            } else {
                for n in &names {
                    println!("{n}");
                }
            }
            Ok(())
        }
        Command::Logger(c) => run_logger(c),
        Command::Info { path, json } => run_info(path, json),
        Command::Make(args) => run_make(args),
        Command::Copy { src, dst, verbose } => run_copy(src, dst, verbose),
    }
}

fn run_logger(args: LoggerArgs) -> Result<(), String> {
    let logger = Logger::instance();
    match args.command {
        LoggerCommand::Level => {
            println!("{}", log_level_name(logger.level()));
            Ok(())
        }
        LoggerCommand::Set { level } => {
            let lv = parse_log_level(&level)?;
            logger.set_level(lv);
            Ok(())
        }
    }
}

/// Read an on-disk TIFF file's metadata and print it.
///
/// Uses the unified [`read_metadata`] entry point so all three facets (primary
/// image descriptor, PTIFF extension fields, structured camera) are shown
/// together. In text mode the camera and field blocks appear only when present.
fn run_info(path: PathBuf, json: bool) -> Result<(), String> {
    let md = read_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if json {
        print_file_metadata_json(&md);
        return Ok(());
    }
    print_file_metadata(&md);
    Ok(())
}

/// Print metadata read from a real file as a JSON document.
///
/// The flattened `ptiff.<domain>.<name>` field list is grouped by domain under
/// a top-level `"ptiff"` object (e.g. `{"spice": {"frame": "IAU_MOON"}}`), and
/// the structured camera calibration is emitted under a `"camera"` object when
/// present (K / [R|t] / P matrices row-major). Values of the extension fields
/// are kept as strings exactly as stored; domains absent from the file simply
/// contribute no keys.
fn print_file_metadata_json(md: &ptiff::Metadata) {
    use serde_json::{json, Map, Value};

    let mut ptiff_domains: Map<String, Value> = Map::new();
    for (key, value) in &md.fields {
        // Split "ptiff.<domain>.<name>" -> ("<domain>", "<name>").
        let rest = key.strip_prefix("ptiff.").unwrap_or(key);
        let (domain, name) = match rest.split_once('.') {
            Some((d, n)) => (d, n),
            None => (rest, ""),
        };
        let entry = ptiff_domains
            .entry(domain.to_string())
            .or_insert_with(|| json!({}));
        if let Value::Object(obj) = entry {
            obj.insert(name.to_string(), Value::String(value.clone()));
        }
    }

    let doc = json!({
        "file": meta_file_fields(&md.file),
        "ptiff": ptiff_domains,
        "camera": md.camera.as_ref().map(camera_to_json),
    });
    // Pretty-print; serde_json always escapes correctly.
    println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
}

/// Build a JSON object describing a structured [`Camera`] calibration.
fn camera_to_json(cam: &Camera) -> serde_json::Value {
    use serde_json::json;
    json!({
        "model": cam.model,
        "has_intrinsics": cam.has_intrinsics,
        "focal_length_x": cam.focal_length_x,
        "focal_length_y": cam.focal_length_y,
        "principal_x": cam.principal_x,
        "principal_y": cam.principal_y,
        "intrinsics": cam.intrinsics,
        "has_extrinsics": cam.has_extrinsics,
        "rotation_w": cam.rotation_w,
        "rotation_x": cam.rotation_x,
        "rotation_y": cam.rotation_y,
        "rotation_z": cam.rotation_z,
        "position_x": cam.position_x,
        "position_y": cam.position_y,
        "position_z": cam.position_z,
        "extrinsics": cam.extrinsics,
        "projection": cam.projection,
        "timestamp": cam.timestamp,
    })
}

/// Build the image-metadata object shared by JSON output.
fn meta_file_fields(meta: &FileMetadata) -> serde_json::Value {
    use serde_json::json;
    json!({
        "width": meta.width,
        "height": meta.height,
        "pixel_type": pixel_name(meta.pixel_type),
        "channel_count": meta.channel_count,
        "tile": match meta.tile_info {
            Some(t) => serde_json::json!({"width": t.tile_width, "height": t.tile_height}),
            None => serde_json::Value::Null,
        },
    })
}

/// Construct an ImageDescriptor and print its resolved metadata.
fn run_make(args: MakeArgs) -> Result<(), String> {
    let pixel = parse_pixel_type(&args.pixel_type)?;
    let compression = match args.compression.as_deref() {
        Some(c) => Some(parse_compression(c)?),
        None => None,
    };

    let mut b = ptiff::ImageDescriptorBuilder::new(args.width, args.height, pixel)
        .channel_count(args.channel_count);

    if let Some(gsd) = args.gsd {
        b = b.gsd(gsd);
    }
    if let (Some(tw), Some(th)) = (args.tile_width, args.tile_height) {
        b = b.tile(tw, th);
    }
    if let Some(c) = compression {
        b = b.compression(c);
    }

    let img = b.build();

    println!("width:         {}", img.width());
    println!("height:        {}", img.height());
    println!("pixel type:    {}", pixel_name(img.pixel_type()));
    println!("channel count: {}", img.channel_count());
    match img.gsd() {
        Some(g) => println!("GSD (m):       {g}"),
        None => println!("GSD (m):       (unset)"),
    }
    match img.tile_info() {
        Some(t) => println!("tile:          {}x{}", t.tile_width, t.tile_height),
        None => println!("tile:          (untiled)"),
    }
    match img.compression() {
        Some(c) => println!("compression:   {}", compression_name(c)),
        None => println!("compression:   (unset)"),
    }
    Ok(())
}

/// Copy the primary image's pixel tiles from `src` to a new TIFF at `dst`.
///
/// Opens a pixel-read [`Source`] on `src`, mirrors its tile layout into a
/// write-side [`Sink`] on `dst`, then reads every tile and writes it straight
/// through. This is a genuine end-to-end pixel I/O path (used to benchmark
/// tile-reads through the CLI, e.g. the large real NASA LRO-NAC DTM) as well as
/// a general tile-copy utility.
fn run_copy(src: PathBuf, dst: PathBuf, verbose: bool) -> Result<(), String> {
    let source = Source::open(&src).map_err(|e| format!("{}: {e}", src.display()))?;
    let cols = source.tile_columns();
    let rows = source.tile_rows();
    let tile_size = source.tile_byte_size();

    // Mirror the source layout into a sink. The C sink requires a tiled image;
    // non-tiled (strip) sources are rejected here with a clear message.
    let builder = source
        .descriptor_builder()
        .map_err(|e| format!("{}: {e}", src.display()))?;
    if builder.tile_info.is_none() {
        return Err(format!(
            "{}: cannot copy a non-tiled (strip) image -- source has no tile layout",
            src.display()
        ));
    }

    // Preserve the source's structured camera calibration (if any) on the
    // copy, so the destination keeps its `ptiff.camera.*` metadata.
    let source_camera = open_path_camera(&src)
        .ok()
        .filter(|c| c.has_intrinsics || c.has_extrinsics);
    let sink = match &source_camera {
        Some(cam) => Sink::create_with_camera(&dst, builder, cam)
            .map_err(|e| format!("{}: {e}", dst.display()))?,
        None => Sink::create(&dst, builder).map_err(|e| format!("{}: {e}", dst.display()))?,
    };

    let mut buffer = vec![0u8; tile_size];
    let mut tiles = 0u64;
    let mut bytes: u64 = 0;
    for r in 0..rows {
        for c in 0..cols {
            let n = source
                .read_tile(c, r, &mut buffer)
                .map_err(|e| format!("{}: read tile ({c},{r}): {e}", src.display()))?;
            sink.write_tile(c, r, &buffer)
                .map_err(|e| format!("{}: write tile ({c},{r}): {e}", dst.display()))?;
            tiles += 1;
            bytes += n as u64;
            if verbose {
                eprintln!("tile ({c},{r}): {n} bytes");
            }
        }
    }
    // Drop the sink (flush) before reporting so the destination is complete
    // and readable when we print the summary.
    drop(sink);

    println!("src:           {}", src.display());
    println!("dst:           {}", dst.display());
    println!("grid:         {}x{} ({} tiles)", cols, rows, tiles);
    println!("tile size:     {tile_size} bytes");
    println!("bytes copied:  {bytes}");
    if source_camera.is_some() {
        println!("camera:        preserved");
    }
    Ok(())
}

/// Print metadata read from a real file (no GSD/compression over this ABI yet).
fn print_file_metadata(md: &ptiff::Metadata) {
    // Primary image descriptor.
    println!("width:         {}", md.file.width);
    println!("height:        {}", md.file.height);
    println!("pixel type:    {}", pixel_name(md.file.pixel_type));
    println!("channel count: {}", md.file.channel_count);
    match md.file.tile_info {
        Some(t) => println!("tile:          {}x{}", t.tile_width, t.tile_height),
        None => println!("tile:          (untiled)"),
    }
    // Structured camera calibration, when present.
    if let Some(cam) = &md.camera {
        println!(
            "camera:        {} (intrinsics={}, extrinsics={})",
            cam.model, cam.has_intrinsics, cam.has_extrinsics
        );
        if cam.has_intrinsics {
            println!(
                "  fx/fy:       {}/{}",
                cam.focal_length_x, cam.focal_length_y
            );
            println!("  cx/cy:       {}/{}", cam.principal_x, cam.principal_y);
        }
        if cam.has_extrinsics {
            println!(
                "  pos (x,y,z): ({}, {}, {})",
                cam.position_x, cam.position_y, cam.position_z
            );
        }
        if !cam.timestamp.is_empty() {
            println!("  timestamp:   {}", cam.timestamp);
        }
    }
    // Flattened PTIFF extension fields, when any.
    if !md.fields.is_empty() {
        println!("fields:        {}", md.fields.len());
        for (key, value) in &md.fields {
            println!("  {key} = {value}");
        }
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

fn parse_pixel_type(s: &str) -> Result<ptiff::PixelType, String> {
    match s.to_ascii_lowercase().as_str() {
        "uint8" | "u8" => Ok(ptiff::PixelType::UInt8),
        "uint16" | "u16" => Ok(ptiff::PixelType::UInt16),
        "uint32" | "u32" => Ok(ptiff::PixelType::UInt32),
        "float32" | "f32" => Ok(ptiff::PixelType::Float32),
        "float64" | "f64" => Ok(ptiff::PixelType::Float64),
        other => Err(format!(
            "unknown pixel type '{other}' (expected uint8|uint16|uint32|float32|float64)"
        )),
    }
}

fn parse_compression(s: &str) -> Result<ptiff::CompressionKind, String> {
    match s.to_ascii_lowercase().as_str() {
        "none" => Ok(ptiff::CompressionKind::None),
        "lzw" => Ok(ptiff::CompressionKind::Lzw),
        "deflate" | "zip" => Ok(ptiff::CompressionKind::Deflate),
        "jpeg" => Ok(ptiff::CompressionKind::Jpeg),
        other => Err(format!(
            "unknown compression '{other}' (expected none|lzw|deflate|jpeg)"
        )),
    }
}

fn parse_log_level(s: &str) -> Result<LogLevel, String> {
    match s.to_ascii_lowercase().as_str() {
        "trace" => Ok(LogLevel::Trace),
        "debug" => Ok(LogLevel::Debug),
        "info" => Ok(LogLevel::Info),
        "warn" => Ok(LogLevel::Warn),
        "error" => Ok(LogLevel::Error),
        "critical" => Ok(LogLevel::Critical),
        "off" => Ok(LogLevel::Off),
        other => Err(format!(
            "unknown log level '{other}' (expected trace|debug|info|warn|error|critical|off)"
        )),
    }
}

fn pixel_name(p: ptiff::PixelType) -> &'static str {
    match p {
        ptiff::PixelType::UInt8 => "uint8",
        ptiff::PixelType::UInt16 => "uint16",
        ptiff::PixelType::UInt32 => "uint32",
        ptiff::PixelType::Float32 => "float32",
        ptiff::PixelType::Float64 => "float64",
    }
}

fn compression_name(c: ptiff::CompressionKind) -> &'static str {
    match c {
        ptiff::CompressionKind::None => "none",
        ptiff::CompressionKind::Lzw => "lzw",
        ptiff::CompressionKind::Deflate => "deflate",
        ptiff::CompressionKind::Jpeg => "jpeg",
    }
}

fn log_level_name(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
        LogLevel::Critical => "critical",
        LogLevel::Off => "off",
    }
}

// ---------------------------------------------------------------------------
// Tests (pure logic, no linked library required)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_type_parsing() {
        assert_eq!(parse_pixel_type("uint8").unwrap(), ptiff::PixelType::UInt8);
        assert_eq!(parse_pixel_type("u16").unwrap(), ptiff::PixelType::UInt16);
        assert_eq!(
            parse_pixel_type("FLOAT32").unwrap(),
            ptiff::PixelType::Float32
        );
        assert!(parse_pixel_type("bogus").is_err());
    }

    #[test]
    fn compression_parsing() {
        assert_eq!(
            parse_compression("lzw").unwrap(),
            ptiff::CompressionKind::Lzw
        );
        assert_eq!(
            parse_compression("zip").unwrap(),
            ptiff::CompressionKind::Deflate
        );
        assert_eq!(
            parse_compression("NONE").unwrap(),
            ptiff::CompressionKind::None
        );
        assert!(parse_compression("brotli").is_err());
    }

    #[test]
    fn log_level_names() {
        assert_eq!(log_level_name(LogLevel::Info), "info");
        assert_eq!(log_level_name(LogLevel::Critical), "critical");
    }

    #[test]
    fn log_level_parsing() {
        assert_eq!(parse_log_level("trace").unwrap(), LogLevel::Trace);
        assert_eq!(parse_log_level("OFF").unwrap(), LogLevel::Off);
        assert!(parse_log_level("verbose").is_err());
    }

    #[test]
    fn camera_json_serializes_model_and_matrices() {
        let cam = Camera::pinhole(
            700.0,
            715.0,
            32.0,
            24.0,
            1.0,
            0.0,
            0.0,
            0.0,
            1.0,
            2.0,
            3.0,
            Some("2026-08-21T12:34:56.000Z".to_string()),
        );
        let v = camera_to_json(&cam);
        // The camera is emitted with its model, intrinsics and timestamp.
        assert_eq!(v["model"], "pinhole");
        assert_eq!(v["focal_length_x"], 700.0);
        assert_eq!(v["has_intrinsics"], true);
        assert_eq!(v["timestamp"], "2026-08-21T12:34:56.000Z");
        // intrinsics / extrinsics / projection are JSON arrays of the right size.
        assert_eq!(v["intrinsics"].as_array().unwrap().len(), 9);
        assert_eq!(v["extrinsics"].as_array().unwrap().len(), 12);
        assert_eq!(v["projection"].as_array().unwrap().len(), 12);
    }
}
