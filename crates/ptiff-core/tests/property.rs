//! Property tests (§11.4).
//!
//! Generated-input checks over the dependency-free core maths (tile
//! arithmetic) and the lossless immutable codecs, so the algorithms are not
//! only exercised on hand-picked constants but across wide input spaces.
//! These run via `proptest`, which is a **test-only** dependency (never in the
//! library build, preserving the default dependency-free core).
//!
//! Mirrors the C++ policy layer's hand-written unit cases but generalises them:
//! the same rounding (`div_ceil`), overflow-freedom, and grid-consistency
//! properties are asserted for arbitrary generated layouts/bytes.

use proptest::prelude::*;
use ptiff_core::io::backend::tiff::{decode_lzw, decode_pack_bits, encode_lzw, encode_pack_bits};
use ptiff_core::tile::{TileExtent, TileIndex, TileLayout};

/// A generated single-level tiled layout with dimensions that cannot overflow.
fn any_layout() -> impl Strategy<Value = TileLayout> {
    // Keep dimensions modest so `width*height` (byte space) and `>> level`
    // computations never overflow, and so `div_ceil` is exact in u32.
    (
        1u32..=2000u32,
        1u32..=2000u32,
        1u32..=2000u32,
        1u32..=2000u32,
    )
        .prop_map(|(tw, th, w, h)| TileLayout::new(TileExtent::new(tw, th), w, h, 1))
}

/// Any payload bytes for the (immutable) codecs.
fn any_bytes(max_len: usize) -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 0..=max_len)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// `columns(level)` is the smallest non-negative integer `c` with
    /// `c * tile_width >= level_width` (i.e. `div_ceil`), and never overflows.
    #[test]
    fn columns_match_div_ceil_reference(layout in any_layout()) {
        for level in 0..layout.level_count {
            let level_w = layout.image_width >> level;
            let c = layout.columns(level);
            let expect = level_w.div_ceil(layout.tile_size.width);
            assert_eq!(c, expect);
            // The last column starts at (c-1)*w and must be inside/at edge
            // of the level image: (c-1)*w < level_w.
            if c > 0 {
                assert!((c - 1) * layout.tile_size.width < level_w);
            }
        }
    }

    /// `rows(level)` is `div_ceil(level_height, tile_height)` and never
    /// overflows.
    #[test]
    fn rows_match_div_ceil_reference(layout in any_layout()) {
        for level in 0..layout.level_count {
            let level_h = layout.image_height >> level;
            let r = layout.rows(level);
            let expect = level_h.div_ceil(layout.tile_size.height);
            assert_eq!(r, expect);
            if r > 0 {
                assert!((r - 1) * layout.tile_size.height < level_h);
            }
        }
    }

    /// For any in-bounds pixel `(px, py)`, `index_for` returns the index of the
    /// tile whose region *starts* at or before that pixel (i.e. the containing
    /// tile's top-left corner is (px - px%tw, py - py%th)).
    #[test]
    fn index_for_region_starts_at_tile_aligned_origin(
        layout in any_layout(),
        px in 0u32..=u32::MAX,
        py in 0u32..=u32::MAX,
    ) {
        let w = layout.image_width;
        let h = layout.image_height;
        let px = px % w;
        let py = py % h;
        let idx = layout.index_for(px, py, 0).expect("in-bounds pixel");
        let region = layout.region_for(idx).expect("index in grid");
        let tw = layout.tile_size.width;
        let th = layout.tile_size.height;
        // The containing tile's origin is the largest tile-aligned origin ≤ the
        // pixel.
        assert_eq!(region.x, px - (px % tw));
        assert_eq!(region.y, py - (py % th));
        // The pixel lies within the tile extent (for the base grid).
        assert!(px < region.x + tw);
        assert!(py < region.y + th);
    }

    /// Enumerating tile (0,0)..(columns-1, rows-1) tiles the level image
    /// without gaps or overlaps in origin space: consecutive columns advance
    /// horizontally by exactly one tile width.
    #[test]
    fn grid_origins_are_contiguous(layout in any_layout()) {
        let cols = layout.columns(0);
        let rows = layout.rows(0);
        // Origin of the last tile (col-1,row-1) must be within the image.
        for col in 0..cols {
            if col > 0 {
                let a = layout.region_for(TileIndex::new(col - 1, 0, 0)).unwrap();
                let b = layout.region_for(TileIndex::new(col, 0, 0)).unwrap();
                assert_eq!(b.x, a.x + layout.tile_size.width);
            }
        }
        for row in 0..rows {
            if row > 0 {
                let a = layout.region_for(TileIndex::new(0, row - 1, 0)).unwrap();
                let b = layout.region_for(TileIndex::new(0, row, 0)).unwrap();
                assert_eq!(b.y, a.y + layout.tile_size.height);
            }
        }
        // Tile (0,0) is at the origin.
        let origin = layout.region_for(TileIndex::new(0, 0, 0)).unwrap();
        assert_eq!((origin.x, origin.y), (0, 0));
    }

    /// LZW is a fixed-point: encode→decode returns the exact original bytes
    /// for arbitrary small inputs (within the size-bomb guard limits).
    #[test]
    fn lzw_round_trip_bytes(data in any_bytes(512)) {
        let encoded = encode_lzw(&data).expect("LZW encode");
        let decoded = decode_lzw(&encoded, data.len()).expect("LZW decode");
        prop_assert_eq!(decoded, data);
    }

    /// PackBits is a fixed-point for arbitrary bytes.
    #[test]
    fn pack_bits_round_trip_bytes(data in any_bytes(512)) {
        let encoded = encode_pack_bits(&data).expect("PackBits encode");
        let decoded = decode_pack_bits(&encoded, data.len()).expect("PackBits decode");
        prop_assert_eq!(decoded, data);
    }
}

/// A hand-written focused check that complements the proptest grid coverage:
/// for several concrete layouts the number of tiles matches width*height
/// coverage (each tile is at least one pixel, total ≥ image pixel area).
#[test]
fn tiled_layout_covers_at_least_the_image_area() {
    for (tw, th, w, h) in [(16, 16, 64, 32), (7, 9, 100, 31), (256, 256, 1000, 1000)] {
        let layout = TileLayout::new(TileExtent::new(tw, th), w, h, 1);
        let cols = layout.columns(0) as u64;
        let rows = layout.rows(0) as u64;
        let tile_area = (tw as u64) * (th as u64);
        assert!(cols * rows * tile_area >= (w as u64) * (h as u64));
    }
}
