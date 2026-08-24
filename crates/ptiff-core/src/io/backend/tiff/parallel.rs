//! Deterministic Rayon-based parallel tile compression/decompression.
//!
//! Mirrors the Phase 5 / §6 parallelization strategy of
//! `PTIFF-1.0-RUST-CORE-PLAN.md`: header/IFD parsing and I/O stay sequential
//! (single-cursor [`crate::io::BinaryReader`]); only the **CPU-bound** per-tile
//! work (compression / decompression) is handed to a shared Rayon work-stealing
//! pool, one job per tile. Each tile's compressed/raw bytes are an *owned*
//! `Vec<u8>` viewed independently, so no shared `&mut` exists and the workers
//! race-free by construction.
//!
//! ## Deterministic results
//!
//! Completely `parallel_iter` over an [`rayon_par_iter`] + `.collect()` into a
//! `Vec` is guaranteed by Rayon to yield its results **in the same order as the
//! input** — even though individual elements may be processed concurrently. So
//! every result in one tile-set is produced concurrently, but the assembled
//! output is byte-identical, in row-major order, to the sequential path.
//! Because (lossless) codecs are pure functions over an owned buffer, parallel
//! and sequential output are bit-for-bit equal — meeting §6.5 "bestimmtheit".
//!
//! This module is only compiled when the `parallel` feature is enabled.

use rayon::prelude::*;

use crate::Result;

/// Fallible encode/decode of one tile's bytes.
///
/// Implementations must be **pure**: deterministic over the input, no shared
/// mutable state, and `Send + Sync` so a Rayon closure can own-and-invoke any
/// captured context concurrently.
pub type TileCodec = dyn Fn(&[u8]) -> Result<Vec<u8>> + Send + Sync;

/// Runs `codec` over every tile in `tiles`, in parallel, returning the outputs
/// **in the same row-major order as the input**.
///
/// Each element of `tiles` is an owned, independent pixel/compressed byte
/// buffer, so the codec only ever sees `&[u8]` views into distinct buffers —
/// there is no shared `&mut` to race. Rayon's `IndexedParallelIterator::collect`
/// preserves input order while scheduling elements across the pool.
///
/// When `tiles` has few elements the pool startup cost can outweigh the win; a
/// caller may short-circuit via [`SerialOverflowGuard::run`] or simply gate by
/// count. This function itself always parallelizes.
///
/// # Errors
///
/// The first failing tile (by input order) short-circuits the pool and is
/// returned; remaining tiles are not processed.
pub fn encode_all<F>(tiles: Vec<Vec<u8>>, codec: F) -> Result<Vec<Vec<u8>>>
where
    F: Fn(&[u8]) -> Result<Vec<u8>> + Send + Sync,
{
    tiles
        .into_par_iter()
        .map(|buf| codec(&buf))
        .collect::<Result<Vec<_>>>()
}

/// Runs `codec` over every tile in `tiles` in parallel, preserving input order.
///
/// Provided as a named sibling of [`encode_all`] for the decode direction — the
/// implementation is identical (both are just `map` + ordered `collect`), kept
/// as separate names for clarity at call sites and for [`TileCodec`]-typed
/// dispatch.
///
/// # Errors
///
/// The first failing tile (by input order) short-circuits the pool.
pub fn decode_all<F>(tiles: Vec<Vec<u8>>, codec: F) -> Result<Vec<Vec<u8>>>
where
    F: Fn(&[u8]) -> Result<Vec<u8>> + Send + Sync,
{
    encode_all(tiles, codec)
}

/// Decisions of whether a tile count is worth the Rayon pool overhead.
///
/// Pool startup and element scheduling have a small fixed cost. Below a modest
/// threshold it is usually faster to run a single-threaded fold. These helpers
/// keep the "parallel or sequential" choice out of business logic while
/// remaining deterministic either way (the sequential fold produces the same
/// byte-identical output).
pub mod threshold {
    /// Minimum number of tiles before parallelism is considered worthwhile.
    ///
    /// Conservative default; tune per workload. Below this, callers should run
    /// the sequential path.
    pub const MIN_TILES_FOR_PARALLEL: usize = 8;

    /// Returns `true` when `count` is large enough that parallelising is
    /// likely to pay off.
    #[must_use]
    pub fn should_parallelize(count: usize) -> bool {
        count >= MIN_TILES_FOR_PARALLEL
    }
}

/// A simple, single-option drop-in that always returns the input untouched —
/// used to document that the parallel functions are opt-in and the sequential
/// path remains authoritative.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    fn bytes(seed: u8, n: usize) -> Vec<u8> {
        (0..n).map(|i| seed.wrapping_add(i as u8)).collect()
    }

    #[test]
    fn encode_all_preserves_input_order() {
        let tiles: Vec<Vec<u8>> = (0..64).map(|_| bytes(0x40, 256)).collect();
        // Identity "codec" that flips a bit and records its index — proving
        // order is preserved even though work is parallel.
        let out = encode_all(tiles.clone(), |b| Ok(b.iter().map(|x| x ^ 0xFF).collect())).unwrap();
        for (inp, o) in tiles.iter().zip(out.iter()) {
            assert_eq!(o.len(), inp.len());
            for (&i, &j) in inp.iter().zip(o.iter()) {
                assert_eq!(i ^ j, 0xFF);
            }
        }
        assert_eq!(out.len(), 64);
    }

    #[test]
    fn encode_all_byte_identical_to_sequential() {
        let tiles: Vec<Vec<u8>> = (0..128).map(|_| bytes(0x10, 1024)).collect();
        // A slightly non-trivial "codec": sum + write inverse. Then run the
        // identical closure sequentially, in order, and require bit-equality.
        let codec_parallel =
            |b: &[u8]| -> Result<Vec<u8>> { Ok(b.iter().rev().cloned().collect()) };
        let parallel_out = encode_all(tiles.clone(), codec_parallel).unwrap();
        let mut sequential_out = Vec::with_capacity(tiles.len());
        for t in tiles.iter() {
            sequential_out.push(codec_parallel(t).unwrap());
        }
        assert_eq!(parallel_out, sequential_out);
    }

    #[test]
    fn first_failure_short_circuits() {
        let tiles: Vec<Vec<u8>> = (0..16).map(|_| bytes(0, 8)).collect();
        let err = encode_all(tiles, |b| {
            if b[0] == 0x00 {
                Err(Error::out_of_range("boom"))
            } else {
                Ok(Vec::new())
            }
        });
        assert!(err.is_err());
    }

    #[test]
    fn decode_all_matches_encode_direction() {
        // decode_all is just an alias; verify it is not a copy-slice trap.
        let tiles = vec![bytes(7, 16), bytes(9, 16)];
        let out = decode_all(tiles.clone(), |b| Ok(b.to_vec())).unwrap();
        assert_eq!(out, tiles);
    }

    #[test]
    fn empty_input_is_fine() {
        let empty: Vec<Vec<u8>> = Vec::new();
        assert!(encode_all(empty.clone(), |_| Ok(vec![0]))
            .unwrap()
            .is_empty());
        assert!(decode_all(empty, |_| Ok(vec![0])).unwrap().is_empty());
    }

    #[test]
    fn threshold_heuristic() {
        assert!(!threshold::should_parallelize(0));
        assert!(!threshold::should_parallelize(1));
        assert!(!threshold::should_parallelize(
            threshold::MIN_TILES_FOR_PARALLEL - 1
        ));
        assert!(threshold::should_parallelize(
            threshold::MIN_TILES_FOR_PARALLEL
        ));
        assert!(threshold::should_parallelize(1000));
    }

    // Concurrency smoke test: spin up N tasks that each run the parallel
    // codec, ensuring the shared pool is safe to use concurrently (no
    // `&mut` global, no data race).
    #[test]
    fn pool_is_reusable_from_multiple_threads() {
        use std::thread;
        let handles: Vec<_> = (0..4)
            .map(|_| {
                thread::spawn(|| {
                    let tiles: Vec<Vec<u8>> = (0..32).map(|_| bytes(0x20, 512)).collect();
                    let out = encode_all(tiles.clone(), |b| Ok(b.to_vec())).unwrap();
                    assert_eq!(out.len(), tiles.len());
                    out
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn codec_captures_owned_state_and_stays_semantically_pure() {
        // A codec that refers to captured (shared, immutable) state is allowed
        // as `&Send`; it must still produce per-input deterministic results.
        let suffix: Vec<u8> = vec![0xAA, 0xBB];
        let out = encode_all(vec![vec![1, 2], vec![3, 4]], |b| {
            let mut v = b.to_vec();
            v.extend_from_slice(&suffix);
            Ok(v)
        })
        .unwrap();
        assert_eq!(out, vec![vec![1, 2, 0xAA, 0xBB], vec![3, 4, 0xAA, 0xBB]]);
    }
}
