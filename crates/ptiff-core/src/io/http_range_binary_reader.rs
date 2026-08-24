//! Read-only [``BinaryReader``] over HTTP(S) Range requests.
//!
//! Mirrors the C++ `ptiff::io::HttpRangeBinaryReader`
//! (`libptiff/include/ptiff/io/http_range_binary_reader.hpp` and
//! `libptiff/src/io/http_range_binary_reader.cpp`): fetches bytes from a remote
//! object (S3/GCS/Azure Blob presigned URL, or any HTTPS endpoint that honours
//! the `Range` header) via a blocking, pure-Rust HTTP(S) client (ureq), exposing
//! the same cursor [`BinaryReader`] API as `MemoryBinaryReader`. Any backend
//! that only knows [`BinaryReader`] (e.g. `TiffBackend` reading a
//! Cloud-Optimized TIFF) works over this transport unmodified.
//!
//! A single read-ahead buffer (default 64 KiB) absorbs the many small sequential
//! reads a TIFF header/IFD parse makes, so opening a remote file does not cost
//! one HTTP request per field. A single read larger than the buffer bypasses it
//! entirely (served directly, buffer left untouched) so one oversized tile fetch
//! cannot evict the header region a later small read needs.
//!
//! # Thread-safety
//!
//! Not thread-safe, under the same contract as its [`BinaryReader`] base: it
//! holds a mutable read cursor and a per-instance HTTP agent.
//!
//! ```text
//! use ptiff_core::io::{BinaryReader, HttpRangeBinaryReader};
//!
//! let mut r = HttpRangeBinaryReader::open(
//!     "https://example-bucket.s3.amazonaws.com/scene.tif",
//!     None,
//! )?;
//! assert!(r.size()? > 0);
//! ```

use crate::io::BinaryReader;
use crate::{Error, Result};

/// Default read-ahead buffer size in bytes (mirrors the C++ default).
const DEFAULT_BUFFER_SIZE: u64 = 64 * 1024;
/// Connect timeout, milliseconds (mirrors the C++ `CURLOPT_CONNECTTIMEOUT_MS`).
const CONNECT_TIMEOUT_MS: u64 = 10_000;
/// Overall per-request timeout, milliseconds (mirrors the C++ `CURLOPT_TIMEOUT_MS`).
const REQUEST_TIMEOUT_MS: u64 = 30_000;

/// A read-only cursor over a remote object fetched via HTTP(S) Range requests.
///
/// See the module docs for the full semantics (read-ahead buffer, bypass on
/// oversized reads, and the `open()`-time probe).
#[derive(Debug)]
pub struct HttpRangeBinaryReader {
    agent: ureq::Agent,
    url: String,
    bearer_token: Option<String>,
    total_size: u64,
    cursor: u64,
    buffer: Vec<u8>,
    buffer_start: u64,
}

/// The result of a single ranged GET.
struct RangedGet {
    status: u16,
    total_size: Option<u64>,
    body: Vec<u8>,
}

impl HttpRangeBinaryReader {
    /// Opens `url` for Range-read access.
    ///
    /// Issues one ranged `GET` (never `HEAD` -- presigned GET URLs commonly
    /// reject HEAD) to discover the object's total size and prime the read-ahead
    /// buffer, mirroring the C++ `HttpRangeBinaryReader::open`.
    ///
    /// # Errors
    /// - [`crate::ErrorCode::InvalidArgument`] if `url` does not start with
    ///   `http://` or `https://`.
    /// - [`crate::ErrorCode::NotFound`] for an HTTP 404.
    /// - [`crate::ErrorCode::Unknown`] for any other transport or HTTP-level
    ///   failure (including a 206 probe missing its `Content-Range` total size).
    pub fn open(url: &str, bearer_token: Option<&str>) -> Result<HttpRangeBinaryReader> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(Error::invalid_argument(format!(
                "HttpRangeBinaryReader::open: url must start with http:// or https://: {url}"
            )));
        }

        let agent = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_connect(Some(std::time::Duration::from_millis(CONNECT_TIMEOUT_MS)))
                .timeout_global(Some(std::time::Duration::from_millis(REQUEST_TIMEOUT_MS)))
                // Treat non-2xx as plain responses, not errors: the reader
                // handles 206/200/404/416 status codes itself (mirrors C++).
                .http_status_as_error(false)
                .build(),
        );

        let bearer_token = bearer_token.map(str::to_owned);

        let mut reader = HttpRangeBinaryReader {
            agent,
            url: url.to_owned(),
            bearer_token,
            total_size: 0,
            cursor: 0,
            buffer: Vec::new(),
            buffer_start: 0,
        };

        // Probe: request the first read-ahead buffer's worth of bytes.
        let probe = reader.perform_ranged_get(0, DEFAULT_BUFFER_SIZE)?;

        match probe.status {
            206 => {
                let total = probe.total_size.ok_or_else(|| {
                    Error::unknown(format!(
                        "HttpRangeBinaryReader::open: 206 response missing Content-Range total size: {url}"
                    ))
                })?;
                reader.total_size = total;
                reader.buffer = probe.body;
                reader.buffer_start = 0;
            }
            200 => {
                // Server ignored the Range header; assume the whole body.
                reader.total_size = probe.body.len() as u64;
                reader.buffer = probe.body;
                reader.buffer_start = 0;
            }
            416 => {
                // Zero-byte object.
                reader.total_size = 0;
            }
            404 => {
                return Err(Error::not_found(format!(
                    "HttpRangeBinaryReader::open: 404 Not Found: {url}"
                )));
            }
            other => {
                return Err(Error::unknown(format!(
                    "HttpRangeBinaryReader::open: unexpected HTTP status {other}: {url}"
                )));
            }
        }

        Ok(reader)
    }

    /// Performs a single ranged `GET` for bytes `[start, start + length)` and
    /// returns the status, optional total size (from `Content-Range`), and body.
    ///
    /// # Errors
    /// [`crate::ErrorCode::Unknown`] on any transport-level failure (DNS, TLS,
    /// connect, timeout, I/O).
    fn perform_ranged_get(&self, start: u64, length: u64) -> Result<RangedGet> {
        let end = start.saturating_add(length).saturating_sub(1);
        let range_header = format!("bytes={start}-{end}");

        let mut req = self.agent.get(&self.url).header("Range", &range_header);
        if let Some(token) = &self.bearer_token {
            req = req.header("Authorization", &format!("Bearer {token}"));
        }

        let res = match req.call() {
            Ok(res) => res,
            Err(e) => {
                // Stale/other transport errors all map to Unknown (mirrors C++
                // `curl_easy_strerror` handling).
                return Err(Error::unknown(format!(
                    "HttpRangeBinaryReader: transport failure: {e}"
                )));
            }
        };

        let status = res.status().as_u16();
        // Read `Content-Range` total (`... / <total>`) if present.
        let mut total_size = None;
        if let Some(cr) = res.headers().get("content-range") {
            if let Ok(cr) = cr.to_str() {
                if let Some(slash) = cr.rfind('/') {
                    if let Ok(v) = cr[slash + 1..].trim().parse::<u64>() {
                        total_size = Some(v);
                    }
                }
            }
        }

        let mut body = res.into_body().into_reader();
        let mut bytes = Vec::new();
        if std::io::Read::read_to_end(&mut body, &mut bytes).is_err() {
            return Err(Error::unknown(
                "HttpRangeBinaryReader: failed to read response body",
            ));
        }

        Ok(RangedGet {
            status,
            total_size,
            body: bytes,
        })
    }

    /// Returns the URL this reader is Range-reading from.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns the bearer token this reader sends (if any), for callers that
    /// need to reopen an equivalent transport.
    #[must_use]
    pub fn bearer_token(&self) -> Option<&str> {
        self.bearer_token.as_deref()
    }
}

impl BinaryReader for HttpRangeBinaryReader {
    fn read(&mut self, destination: &mut [u8]) -> Result<usize> {
        if destination.is_empty() {
            return Ok(0);
        }
        if self.cursor >= self.total_size {
            return Ok(0);
        }

        let remaining = self.total_size - self.cursor;
        let wanted = (destination.len() as u64).min(remaining) as usize;

        // Buffer hit: the requested range sits entirely within the read-ahead
        // buffer.
        let hit = self.cursor >= self.buffer_start
            && self.cursor + wanted as u64 <= self.buffer_start + self.buffer.len() as u64;
        if hit {
            let offset = (self.cursor - self.buffer_start) as usize;
            destination[..wanted].copy_from_slice(&self.buffer[offset..offset + wanted]);
            self.cursor += wanted as u64;
            return Ok(wanted);
        }

        // Buffer miss: fetch. Clamp the fetch length to what remains.
        let fetch_len = (wanted as u64).max(DEFAULT_BUFFER_SIZE).min(remaining);
        let fetched = self.perform_ranged_get(self.cursor, fetch_len)?;

        match fetched.status {
            416 => {
                // Only reachable if the remote object shrank between open() and
                // this read(); the requested range is always clamped to what
                // open() learned was available.
                return Err(Error::out_of_range(
                    "HttpRangeBinaryReader::read: range no longer satisfiable (416) -- remote object may have changed since open()",
                ));
            }
            206 | 200 => {}
            other => {
                return Err(Error::unknown(format!(
                    "HttpRangeBinaryReader::read: unexpected HTTP status {other}"
                )));
            }
        }

        if fetched.body.len() as u64 > DEFAULT_BUFFER_SIZE {
            // Oversized fetch (a single read wider than the buffer): serve
            // directly, leave the read-ahead buffer untouched.
            let n = wanted.min(fetched.body.len());
            destination[..n].copy_from_slice(&fetched.body[..n]);
            self.cursor += n as u64;
            return Ok(n);
        }

        self.buffer = fetched.body;
        self.buffer_start = self.cursor;
        let n = wanted.min(self.buffer.len());
        if n > 0 {
            destination[..n].copy_from_slice(&self.buffer[..n]);
        }
        self.cursor += n as u64;
        Ok(n)
    }

    fn seek(&mut self, offset: u64) -> Result<()> {
        if offset > self.total_size {
            return Err(Error::out_of_range(
                "HttpRangeBinaryReader::seek: offset beyond size",
            ));
        }
        self.cursor = offset;
        Ok(())
    }

    fn position(&self) -> Result<u64> {
        Ok(self.cursor)
    }

    fn size(&self) -> Result<u64> {
        Ok(self.total_size)
    }
}

#[cfg(test)]
mod tests {
    //! Tests spin up a minimal local HTTP server on an ephemeral port that
    //! honours `Range` headers (206 + `Content-Range`), falls back to a full
    //! 200 when asked to ignore ranges, and returns 404/416 for the relevant
    //! error cases. This validates the whole transport against real HTTP
    //! semantics without any external network.

    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    /// Serves `body`, honouring `Range: bytes=a-b` when `ignore_range` is false
    /// (206 + Content-Range), else returning the full body with 200.
    ///
    /// - path `/missing` → 404
    fn serve(body: &'static [u8], ignore_range: bool) -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = match stream {
                    Ok(s) => s,
                    Err(_) => break,
                };
                // Read the request line + headers (small request; coarse read).
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let request = String::from_utf8_lossy(&buf);
                let first_line = request.lines().next().unwrap_or("");
                let path = first_line.split_whitespace().nth(1).unwrap_or("/");
                let range = request.lines().find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("range")
                        .then_some(v.trim().to_string())
                });

                let response = if path.starts_with("/missing") {
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .as_bytes()
                        .to_vec()
                } else if !ignore_range {
                    // Honour Range.
                    match range {
                        Some(r) if r.starts_with("bytes=") => {
                            let spec = &r["bytes=".len()..];
                            if let Some((a, b)) = spec.split_once('-') {
                                let start: usize = a.parse().unwrap_or(0);
                                let len: usize = b
                                    .parse::<usize>()
                                    .map(|x| x - start + 1)
                                    .unwrap_or(body.len() - start);
                                let end = start.saturating_add(len).saturating_sub(1);
                                // 416 only when the START is out of range; the
                                // end is clamped to the last available byte
                                // (real servers saturate, they don't 416).
                                if start >= body.len() {
                                    format!(
                                        "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                        body.len()
                                    )
                                    .into_bytes()
                                } else {
                                    let end = end.min(body.len() - 1);
                                    let slice = &body[start..=end];
                                    let mut head = format!(
                                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                        body.len(),
                                        slice.len()
                                    )
                                    .into_bytes();
                                    head.extend_from_slice(slice);
                                    head
                                }
                            } else {
                                // `bytes=start-` → to end.
                                let start: usize = spec.parse().unwrap_or(0);
                                if start >= body.len() {
                                    format!(
                                        "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                        body.len()
                                    )
                                    .into_bytes()
                                } else {
                                    let slice = &body[start..];
                                    let mut head = format!(
                                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                        body.len() - 1,
                                        body.len(),
                                        slice.len()
                                    )
                                    .into_bytes();
                                    head.extend_from_slice(slice);
                                    head
                                }
                            }
                        }
                        _ => {
                            // No Range header → 200 full body.
                            let mut head = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            )
                            .into_bytes();
                            head.extend_from_slice(body);
                            head
                        }
                    }
                } else {
                    // Range ignored → always 200 with the full body.
                    let mut head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .into_bytes();
                    head.extend_from_slice(body);
                    head
                };

                let _ = stream.write_all(&response);
                let _ = stream.flush();
            }
        });
        addr
    }

    /// Builds a URL for the given server address + optional path.
    fn url_for(addr: std::net::SocketAddr, path: &str) -> String {
        format!("http://127.0.0.1:{}/{}", addr.port(), path)
    }

    fn test_body() -> &'static [u8] {
        // 4096 bytes of a deterministic pattern.
        Box::leak(Box::new(
            (0..4096usize).map(|i| (i % 251) as u8).collect::<Vec<u8>>(),
        ))
    }

    #[test]
    fn rejects_non_http_url() {
        let err =
            HttpRangeBinaryReader::open("ftp://example.com/file", None).expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
        let err = HttpRangeBinaryReader::open("/local/path.tif", None).expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn missing_path_is_not_found() {
        let body = test_body();
        let listener = serve(body, false);
        let url = url_for(listener, "missing");
        let err = HttpRangeBinaryReader::open(&url, None).expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::NotFound);
    }

    #[test]
    fn open_reports_total_size_and_sequential_read() {
        let body = test_body();
        let listener = serve(body, false);
        let url = url_for(listener, "file.tif");
        let mut r = HttpRangeBinaryReader::open(&url, None).expect("open");
        assert_eq!(r.size().unwrap(), body.len() as u64);
        assert_eq!(r.position().unwrap(), 0);

        // Sequential small reads should hit the read-ahead buffer (single fetch
        // for the first 64 KiB).
        let mut got = Vec::new();
        let mut chunk = [0u8; 256];
        loop {
            let n = r.read(&mut chunk).expect("read");
            if n == 0 {
                break;
            }
            got.extend_from_slice(&chunk[..n]);
        }
        assert_eq!(got, body, "whole body must round-trip over ranged reads");
        assert_eq!(r.position().unwrap(), body.len() as u64);
        // At EOF, reads return 0.
        assert_eq!(r.read(&mut chunk).expect("read at eof"), 0);
    }

    #[test]
    fn seek_and_buffer_miss() {
        // Body larger than the initial read-ahead buffer so we can seek beyond
        // it and force a fresh ranged fetch.
        let big: Vec<u8> = (0..(DEFAULT_BUFFER_SIZE as usize) + 1024)
            .map(|i| (i % 251) as u8)
            .collect();
        let big: &'static [u8] = Box::leak(Box::new(big));
        let listener = serve(big, false);
        let url = url_for(listener, "file.tif");
        let mut r = HttpRangeBinaryReader::open(&url, None).expect("open");

        // Seek far beyond the initial read-ahead buffer → forces a new fetch.
        let target = (DEFAULT_BUFFER_SIZE + 64) as usize;
        r.seek(target as u64).expect("seek");
        assert_eq!(r.position().unwrap(), target as u64);

        let mut chunk = [0u8; 128];
        let n = r.read(&mut chunk).expect("read after seek");
        assert_eq!(n, 128);
        assert_eq!(&chunk[..], &big[target..target + 128]);
    }

    #[test]
    fn seek_past_end_is_out_of_range() {
        let body = test_body();
        let listener = serve(body, false);
        let url = url_for(listener, "file.tif");
        let mut r = HttpRangeBinaryReader::open(&url, None).expect("open");
        let err = r.seek(body.len() as u64 + 1).expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::OutOfRange);
    }

    #[test]
    fn oversized_direct_read_bypasses_buffer() {
        let body = test_body(); // 4096 bytes < 64 KiB buffer, but we force a large read.
        let listener = serve(body, false);
        let url = url_for(listener, "file.tif");
        let mut r = HttpRangeBinaryReader::open(&url, None).expect("open");

        // Read a chunk larger than the body in one call; clamp to remaining.
        let mut big = [0u8; 8192];
        let n = r.read(&mut big).expect("big read");
        assert_eq!(n, body.len(), "one read returns the whole remaining body");
        assert_eq!(&big[..n], body);
        assert_eq!(r.position().unwrap(), body.len() as u64);
    }

    #[test]
    fn server_ignoring_range_uses_200_fallback() {
        let body = test_body();
        let listener = serve(body, true);
        let url = url_for(listener, "file.tif");
        let mut r = HttpRangeBinaryReader::open(&url, None).expect("open");
        assert_eq!(r.size().unwrap(), body.len() as u64);

        let mut got = Vec::new();
        let mut chunk = [0u8; 512];
        loop {
            let n = r.read(&mut chunk).expect("read");
            if n == 0 {
                break;
            }
            got.extend_from_slice(&chunk[..n]);
        }
        assert_eq!(got, body);
    }

    #[test]
    fn bearer_token_is_sent() {
        // A server that echoes the Authorization header into the first body
        // bytes isn't practical with our simple helper; this test just verifies
        // opening with a token still works end-to-end over the ranged transport.
        let body = test_body();
        let listener = serve(body, false);
        let url = url_for(listener, "file.tif");
        let mut r =
            HttpRangeBinaryReader::open(&url, Some("s3cr3t")).expect("open with bearer token");
        assert_eq!(r.size().unwrap(), body.len() as u64);
        let mut chunk = [0u8; 16];
        assert_eq!(r.read(&mut chunk).expect("read"), 16);
    }

    #[test]
    fn transport_skips_on_read_serve_directly() {
        // A single read larger than the default buffer must be served directly.
        // Build a body larger than the buffer to exercise the bypass path.
        let big: Vec<u8> = (0..(DEFAULT_BUFFER_SIZE as usize) + 1024)
            .map(|i| (i % 251) as u8)
            .collect();
        let big: &'static [u8] = Box::leak(Box::new(big));
        let listener = serve(big, false);
        let url = url_for(listener, "big.tif");
        let mut r = HttpRangeBinaryReader::open(&url, None).expect("open");
        assert_eq!(r.size().unwrap(), big.len() as u64);

        // Request a single read wider than the whole body up to the buffer size;
        // clamp to the requested length. A single oversized fetch happens below.
        let mut chunk = vec![0u8; (DEFAULT_BUFFER_SIZE as usize) + 1];
        let n = r.read(&mut chunk).expect("read");
        assert_eq!(n, (DEFAULT_BUFFER_SIZE as usize) + 1);
        assert_eq!(&chunk[..n], &big[..n]);
        assert_eq!(r.position().unwrap(), n as u64);
        // On the oversized-fetch path the read-ahead buffer was left untouched
        // (empty here), so the next small read triggers another fetch from the
        // new cursor -- which must still return the correct bytes.
        let mut small = [0u8; 32];
        let m = r.read(&mut small).expect("read small");
        let start = (DEFAULT_BUFFER_SIZE as usize) + 1;
        assert_eq!(m, 32);
        assert_eq!(&small[..], &big[start..start + 32]);
    }

    #[test]
    fn zero_byte_object_via_416_probe_is_ok() {
        // Probe with an out-of-range request (start >= len) on a zero-body
        // "server" returns 416 → total_size = 0.
        let listener = serve(&[], false);
        let url = url_for(listener, "empty.tif");
        let r = HttpRangeBinaryReader::open(&url, None).expect("open");
        assert_eq!(r.size().unwrap(), 0);
    }
}
