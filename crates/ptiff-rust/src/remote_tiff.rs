//! Remote, cloud-backed reading of a PTIFF over HTTP range requests.
//!
//! [`RemoteTiff`] is the network counterpart of [`Tiff`]: instead of loading
//! the whole file into memory with [`Tiff::open`], it streams the container
//! through the core's [`HttpRangeBinaryReader`](ptiff_core::io::HttpRangeBinaryReader),
//! which fetches only the byte ranges a TIFF/BigTIFF parser actually touches
//! (64 KiB read-ahead, oversized reads bypass straight to the socket). This is
//! what lets a multi-gigabyte PTIFF on object storage or a web server be
//! inspected — header, IFD chain, and individual tiles — without downloading
//! the whole object.
//!
//! The API mirrors [`Tiff`]: parse the scene on open, then read metadata and
//! individual tiles on demand. Each tile read performs one (buffered) range
//! request, so walking an entire raster is naturally bandwidth-bounded.
//!
//! # Examples
//!
//! ```no_run
//! use ptiff::RemoteTiff;
//!
//! let remote = RemoteTiff::open_remote("https://example.com/image.ptiff", None)?;
//! println!("{} image(s)", remote.scene().image_count());
//! let tile = remote.read_tile(0, 0, 0)?;
//! # Ok::<_, ptiff::Error>(())
//! ```

use ptiff_core::io::backend::tiff::TiffBackend;
use ptiff_core::io::{BinaryReader, HttpRangeBinaryReader, SceneDeserializer};
use ptiff_core::{Deserializer, Result, Scene, StorageBackend as _};

/// A PTIFF/TIFF/BigTIFF file read over HTTP range requests.
///
/// `RemoteTiff` parses the on-disk container's metadata over the network on
/// [`open_remote`](RemoteTiff::open_remote)(the header + IFD chain, typically
/// the first few kilobytes), then lazily fetches individual tiles when the
/// pixel tier is used ([`RemoteTiff::read_tile`], [`RemoteTiff::tile_layout`]).
///
/// Unlike [`Tiff`], it does **not** retain the full byte stream in memory: the
/// underlying [`HttpRangeBinaryReader`](ptiff_core::io::HttpRangeBinaryReader)
/// only keeps a bounded read-ahead buffer. This is the cloud-friendly read path.
///
/// [`Tiff`]: crate::Tiff
#[derive(Debug)]
pub struct RemoteTiff {
    /// The transport; cursor is rewound to the container start before each
    /// pixel/tile operation so `TiffBackend` can re-parse the IFD chain.
    reader: HttpRangeBinaryReader,
    /// The parsed scene (image metadata), captured at open time.
    scene: Scene,
}

impl RemoteTiff {
    /// Opens a PTIFF/TIFF/BigTIFF file at `url` over HTTP(S) range requests.
    ///
    /// The first network request probes the object's size (`Range: bytes=0-...`)
    /// and, on success, parses the container metadata (header + IFD chain).
    /// `bearer_token`, when given, is sent as `Authorization: Bearer <token>`
    /// on every range request.
    ///
    /// Only `http://` and `https://` URLs are accepted.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::InvalidArgument`](crate::ErrorCode::InvalidArgument) for non-HTTP(S)
    /// URLs, [`ErrorCode::NotFound`](crate::ErrorCode::NotFound) for HTTP 404,
    /// [`ErrorCode::OutOfRange`](crate::ErrorCode::OutOfRange) for an
    /// unsatisfiable range, [`ErrorCode::Unknown`](crate::ErrorCode::Unknown)
    /// for other transport/status failures, or
    /// [`ErrorCode::InvalidArgument`](crate::ErrorCode::InvalidArgument) if the
    /// object is not a valid TIFF/BigTIFF container.
    pub fn open_remote(url: &str, bearer_token: Option<&str>) -> Result<RemoteTiff> {
        let mut reader = HttpRangeBinaryReader::open(url, bearer_token)?;
        let backend = TiffBackend;
        let model = backend.deserialize_model(&mut reader)?;
        let scene = SceneDeserializer.deserialize(&model)?;
        Ok(RemoteTiff { reader, scene })
    }

    /// Returns the parsed scene (the composition of the file's images).
    #[must_use]
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Returns the `index`-th image of the scene.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::OutOfRange`](crate::ErrorCode::OutOfRange) if `index` is not in
    /// `[0, image_count())`.
    pub fn image(&self, index: usize) -> Result<&ptiff_core::Image> {
        self.scene.image_at(index)
    }

    /// Returns an iterator over the scene's images in file order.
    pub fn images(&self) -> impl Iterator<Item = &ptiff_core::Image> {
        let scene = &self.scene;
        (0..scene.image_count()).filter_map(|i| scene.image_at(i).ok())
    }

    /// Decodes the `image_index`-th image's raw pixel bytes.
    ///
    /// Equivalent to [`Tiff::read_image_pixels`](crate::Tiff::read_image_pixels)
    /// but served over the network: the grid is walked one tile per range
    /// request. Rewinds the transport to the container start first, so each
    /// call is independent.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::OutOfRange`](crate::ErrorCode::OutOfRange) if `image_index` is not in
    /// `[0, image_count())`, or a backend/transport error while decoding.
    pub fn read_image_pixels(&self, image_index: usize) -> Result<Vec<u8>> {
        let mut reader = self.reopen_reader()?;
        let mut source = TiffBackend.open_image_source_at(&mut reader, image_index)?;
        let layout = *source.layout();
        let columns = layout.columns(0);
        let rows = layout.rows(0);

        let mut out = Vec::new();
        for row in 0..rows {
            for column in 0..columns {
                let tile = source.read_tile(ptiff_core::tile::TileIndex::new(column, row, 0))?;
                out.extend_from_slice(tile.data());
            }
        }
        Ok(out)
    }

    /// Decodes a single tile (or strip) of the `image_index`-th image.
    ///
    /// One buffered range request is performed per call. See
    /// [`Tiff::read_tile`](crate::Tiff::read_tile) for the addressing scheme.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::OutOfRange`](crate::ErrorCode::OutOfRange) if `image_index` is out of
    /// range or `column`/`row` fall outside the tile grid, or a backend/transport
    /// error while decoding.
    pub fn read_tile(&self, image_index: usize, column: u32, row: u32) -> Result<Vec<u8>> {
        let mut reader = self.reopen_reader()?;
        let mut source = TiffBackend.open_image_source_at(&mut reader, image_index)?;
        let tile = source.read_tile(ptiff_core::tile::TileIndex::new(column, row, 0))?;
        Ok(tile.data().to_vec())
    }

    /// Returns the tile grid layout of the `image_index`-th image, as stored.
    ///
    /// See [`Tiff::tile_layout`](crate::Tiff::tile_layout).
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::OutOfRange`](crate::ErrorCode::OutOfRange) if `image_index` is not in
    /// `[0, image_count())`.
    pub fn tile_layout(&self, image_index: usize) -> Result<ptiff_core::tile::TileLayout> {
        let mut reader = self.reopen_reader()?;
        let source = TiffBackend.open_image_source_at(&mut reader, image_index)?;
        Ok(*source.layout())
    }

    /// URL of the remote object being read.
    #[must_use]
    pub fn url(&self) -> &str {
        self.reader.url()
    }

    /// Returns the remote object's total size in bytes, as discovered on open.
    pub fn size_bytes(&self) -> Result<u64> {
        self.reader.size()
    }

    /// Opens a fresh transport over the same URL, positioned at the container
    /// start (offset 0), so `TiffBackend` can parse the IFD chain / open an
    /// image source for an independent, re-entrant pixel/tile call.
    ///
    /// The stored reader is not `Clone`, and its cursor is consumed while
    /// parsing the scene at open time — a fresh transport per call sidesteps
    /// mutable-borrow bookkeeping and lets `&self` methods stay independent.
    fn reopen_reader(&self) -> Result<HttpRangeBinaryReader> {
        let mut reader =
            HttpRangeBinaryReader::open(self.reader.url(), self.reader.bearer_token())?;
        // A freshly opened reader already sits at offset 0; seek defensively
        // anyway (cheap, and future-proof).
        reader.seek(0)?;
        Ok(reader)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ptiff_core::{CompressionKind, ImageDescriptor, PixelType, Scene};

    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    use crate::Tiff;

    /// Spins up a tiny HTTP server that serves `body` honoring `Range`
    /// requests (206 + Content-Range; 200 fallback), and returns its base URL.
    fn serve(body: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let total = body.len();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let _ = handle(&mut stream, &body, total);
            }
        });
        format!("http://{addr}")
    }

    fn handle(stream: &mut TcpStream, body: &[u8], total: usize) -> std::io::Result<()> {
        let mut buf = vec![0u8; 4096];
        let n = stream.read(&mut buf)?;
        let req = String::from_utf8_lossy(&buf[..n]);

        if !req.starts_with("GET ") {
            return Ok(());
        }
        // Extract the Range header if present.
        let range = req
            .lines()
            .find_map(|l| l.strip_prefix("Range: "))
            .map(|v| v.trim().to_string());

        let (start, end) = match &range {
            Some(r) if r.starts_with("bytes=") => {
                let spec = r.trim_start_matches("bytes=");
                let (s, e) = spec.split_once('-').unwrap_or((spec, ""));
                let s = s.parse::<usize>().ok();
                let e = if e.is_empty() {
                    None
                } else {
                    e.parse::<usize>().ok()
                };
                (s, e)
            }
            _ => (None, None),
        };

        let start = start.unwrap_or(0);
        if start >= total {
            let resp = "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\n\r\n";
            stream.write_all(resp.as_bytes())?;
            return Ok(());
        }
        let end = end.unwrap_or(total).min(total - 1);
        let slice = &body[start..=end];
        let clen = slice.len();
        let head = format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{total}\r\nContent-Length: {clen}\r\n\r\n"
        );
        stream.write_all(head.as_bytes())?;
        stream.write_all(slice)?;
        Ok(())
    }

    /// Builds a single-image UInt8 grayscale TIFF with real pixels.
    fn single_image_tiff(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        let mut scene = Scene::new();
        let mut desc = ImageDescriptor::new(width, height);
        desc.channel_count = 1;
        scene.add_image(desc).expect("add");
        Tiff::to_bytes_with_pixels(&scene, &[pixels]).expect("write")
    }

    /// A two-image scene (metadata only) to verify multi-image remote reads.
    fn multi_image_tiff() -> Vec<u8> {
        let mut scene = Scene::new();
        let mut first = ImageDescriptor::new(8, 8);
        first.channel_count = 1;
        scene.add_image(first).expect("first");
        let mut second = ImageDescriptor::new(16, 16);
        second.channel_count = 3;
        second.pixel_type = PixelType::UInt16;
        second.compression = Some(CompressionKind::Lzw);
        scene.add_image(second).expect("second");
        Tiff::to_bytes(&scene).expect("write")
    }

    #[test]
    fn open_remote_parses_scene_over_range() {
        let bytes = single_image_tiff(
            16,
            16,
            &(0..256u32).map(|i| (i % 251) as u8).collect::<Vec<_>>(),
        );
        let url = serve(bytes.clone());
        let remote = RemoteTiff::open_remote(&url, None).expect("open remote");
        assert_eq!(remote.scene().image_count(), 1);
        let img = remote.image(0).expect("image");
        assert_eq!((img.width(), img.height()), (16, 16));
        assert_eq!(img.channel_count(), 1);
        assert_eq!(remote.size_bytes().expect("size"), bytes.len() as u64);
    }

    #[test]
    fn open_remote_reads_pixels_over_range() {
        let width = 32u32;
        let height = 16u32;
        let pixels: Vec<u8> = (0..(width * height)).map(|i| (i * 7 + 3) as u8).collect();
        let bytes = single_image_tiff(width, height, &pixels);
        let url = serve(bytes);
        let remote = RemoteTiff::open_remote(&url, None).expect("open remote");
        let read = remote.read_image_pixels(0).expect("read pixels");
        assert_eq!(read, pixels, "remote raster must match served pixels");
    }

    #[test]
    fn open_remote_reads_individual_tile_and_layout() {
        let bytes = single_image_tiff(16, 16, &(0..256u32).map(|i| i as u8).collect::<Vec<_>>());
        let url = serve(bytes);
        let remote = RemoteTiff::open_remote(&url, None).expect("open remote");
        let layout = remote.tile_layout(0).expect("layout");
        assert_eq!((layout.columns(0), layout.rows(0)), (1, 1));
        let tile = remote.read_tile(0, 0, 0).expect("read tile");
        assert_eq!(tile.len(), 256);
    }

    #[test]
    fn open_remote_supports_multi_image_scene() {
        let bytes = multi_image_tiff();
        let url = serve(bytes);
        let remote = RemoteTiff::open_remote(&url, None).expect("open remote");
        assert_eq!(remote.scene().image_count(), 2);
        let dims: Vec<(u32, u32)> = remote.images().map(|i| (i.width(), i.height())).collect();
        assert_eq!(dims, vec![(8, 8), (16, 16)]);
    }

    #[test]
    fn open_remote_sends_bearer_token() {
        use std::sync::{Arc, Mutex};
        use std::time::{Duration, Instant};
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(None::<String>));
        let seen_arc = seen.clone();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = vec![0u8; 4096];
                let _ = stream.read(&mut buf);
                let req = String::from_utf8_lossy(&buf);
                // ureq sends header names lowercased ("authorization:");
                // match case-insensitively.
                let auth = req
                    .lines()
                    .find(|l| l.to_ascii_lowercase().starts_with("authorization:"))
                    .and_then(|l| l.split_once(':'))
                    .map(|(_, v)| v.trim().to_string());
                *seen_arc.lock().unwrap() = auth;
                // Reply with a valid 206 so the probe (and a minimal header
                // parse) can complete on the client side.
                let body = single_image_tiff(4, 4, &[0u8; 16]);
                let resp = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\n\r\n",
                    0,
                    body.len() - 1,
                    body.len(),
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.write_all(&body);
                let _ = stream.flush();
            }
        });
        let url = format!("http://{addr}");
        // Open with a bearer token; parse may or may not fully succeed (the
        // object is a real TIFF, so it should), but we only assert the header.
        let _ = RemoteTiff::open_remote(&url, Some("sekrit"));

        // Wait (bounded) for the server thread to capture the Authorization
        // header — the open() round-trip has already unblocked by now, but the
        // server thread may not have scheduled its write yet.
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            {
                let guard = seen.lock().unwrap();
                if guard.is_some() {
                    assert_eq!(guard.as_deref(), Some("Bearer sekrit"));
                    break;
                }
            }
            assert!(
                Instant::now() < deadline,
                "server never captured the Authorization header"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn open_remote_rejects_non_http_url() {
        let err =
            RemoteTiff::open_remote("ftp://example.com/x.tif", None).expect_err("must reject ftp");
        assert_eq!(err.code(), ptiff_core::ErrorCode::InvalidArgument);
    }
}
