//! TIFF byte-order helpers.
//!
//! Mirrors `ptiff::io::backend::tiff::tiff_endian.hpp`.

/// Byte order a TIFF/BigTIFF file declares in its header ("II" = little-endian,
/// "MM" = big-endian). Every multi-byte field in the file, including IFD
/// entries, is encoded in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    /// Least-significant byte first.
    Little,
    /// Most-significant byte first.
    Big,
}

/// Reads a 2-byte unsigned integer from the first 2 bytes of `bytes`, honoring
/// `endian`.
///
/// # Panics
///
/// Panics if `bytes.len() < 2` (mirrors the C++ precondition).
pub fn read_u16(bytes: &[u8], endian: Endian) -> u16 {
    assert!(bytes.len() >= 2, "read_u16 requires at least 2 bytes");
    let b0 = u16::from(bytes[0]);
    let b1 = u16::from(bytes[1]);
    match endian {
        Endian::Little => b0 | (b1 << 8),
        Endian::Big => (b0 << 8) | b1,
    }
}

/// Reads a 4-byte unsigned integer from the first 4 bytes of `bytes`, honoring
/// `endian`.
///
/// # Panics
///
/// Panics if `bytes.len() < 4` (mirrors the C++ precondition).
pub fn read_u32(bytes: &[u8], endian: Endian) -> u32 {
    assert!(bytes.len() >= 4, "read_u32 requires at least 4 bytes");
    let b0 = u32::from(bytes[0]);
    let b1 = u32::from(bytes[1]);
    let b2 = u32::from(bytes[2]);
    let b3 = u32::from(bytes[3]);
    match endian {
        Endian::Little => b0 | (b1 << 8) | (b2 << 16) | (b3 << 24),
        Endian::Big => (b0 << 24) | (b1 << 16) | (b2 << 8) | b3,
    }
}

/// Reads an 8-byte unsigned integer from the first 8 bytes of `bytes`, honoring
/// `endian`.
///
/// # Panics
///
/// Panics if `bytes.len() < 8` (mirrors the C++ precondition).
pub fn read_u64(bytes: &[u8], endian: Endian) -> u64 {
    assert!(bytes.len() >= 8, "read_u64 requires at least 8 bytes");
    let mut value: u64 = 0;
    for (i, &byte) in bytes.iter().take(8).enumerate() {
        let byte = u64::from(byte);
        let shift = match endian {
            Endian::Little => i,
            Endian::Big => 7 - i,
        } as u32
            * 8;
        value |= byte << shift;
    }
    value
}

/// Writes `value` into the first 2 bytes of `out`, honoring `endian`.
///
/// # Panics
///
/// Panics if `out.len() < 2` (mirrors the C++ precondition).
pub fn write_u16(out: &mut [u8], value: u16, endian: Endian) {
    assert!(
        out.len() >= 2,
        "write_u16 requires at least 2 bytes of output"
    );
    let b0 = (value & 0xFF) as u8;
    let b1 = ((value >> 8) & 0xFF) as u8;
    match endian {
        Endian::Little => {
            out[0] = b0;
            out[1] = b1;
        }
        Endian::Big => {
            out[0] = b1;
            out[1] = b0;
        }
    }
}

/// Writes `value` into the first 4 bytes of `out`, honoring `endian`.
///
/// # Panics
///
/// Panics if `out.len() < 4` (mirrors the C++ precondition).
pub fn write_u32(out: &mut [u8], value: u32, endian: Endian) {
    assert!(
        out.len() >= 4,
        "write_u32 requires at least 4 bytes of output"
    );
    for (i, slot) in out.iter_mut().take(4).enumerate() {
        let shift = match endian {
            Endian::Little => i,
            Endian::Big => 3 - i,
        } as u32
            * 8;
        *slot = ((value >> shift) & 0xFF) as u8;
    }
}

/// Writes `value` into the first 8 bytes of `out`, honoring `endian`.
///
/// # Panics
///
/// Panics if `out.len() < 8` (mirrors the C++ precondition).
pub fn write_u64(out: &mut [u8], value: u64, endian: Endian) {
    assert!(
        out.len() >= 8,
        "write_u64 requires at least 8 bytes of output"
    );
    for (i, slot) in out.iter_mut().take(8).enumerate() {
        let shift = match endian {
            Endian::Little => i,
            Endian::Big => 7 - i,
        } as u32
            * 8;
        *slot = ((value >> shift) & 0xFF) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_u16_honors_byte_order() {
        assert_eq!(read_u16(&[0x34, 0x12], Endian::Little), 0x1234);
        assert_eq!(read_u16(&[0x12, 0x34], Endian::Big), 0x1234);
    }

    #[test]
    fn read_u32_honors_byte_order() {
        assert_eq!(
            read_u32(&[0x78, 0x56, 0x34, 0x12], Endian::Little),
            0x12345678
        );
        assert_eq!(read_u32(&[0x12, 0x34, 0x56, 0x78], Endian::Big), 0x12345678);
    }

    #[test]
    fn read_u64_honors_byte_order() {
        assert_eq!(
            read_u64(
                &[0xF0, 0xDE, 0xBC, 0x9A, 0x78, 0x56, 0x34, 0x12],
                Endian::Little
            ),
            0x123456789ABCDEF0
        );
        assert_eq!(
            read_u64(
                &[0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0],
                Endian::Big
            ),
            0x123456789ABCDEF0
        );
    }

    #[test]
    fn readers_only_consume_leading_bytes() {
        // Leading 0x0001 little-endian == 1; trailing bytes ignored.
        assert_eq!(read_u16(&[0x01, 0x00, 0xFF, 0xFF], Endian::Little), 1);
    }

    #[test]
    fn write_u16_honors_byte_order() {
        let mut little = [0u8; 2];
        write_u16(&mut little, 0x1234, Endian::Little);
        assert_eq!(little, [0x34, 0x12]);

        let mut big = [0u8; 2];
        write_u16(&mut big, 0x1234, Endian::Big);
        assert_eq!(big, [0x12, 0x34]);
    }

    #[test]
    fn write_u32_honors_byte_order() {
        let mut little = [0u8; 4];
        write_u32(&mut little, 0x12345678, Endian::Little);
        assert_eq!(little, [0x78, 0x56, 0x34, 0x12]);

        let mut big = [0u8; 4];
        write_u32(&mut big, 0x12345678, Endian::Big);
        assert_eq!(big, [0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn write_u64_honors_byte_order() {
        let mut little = [0u8; 8];
        write_u64(&mut little, 0x123456789ABCDEF0, Endian::Little);
        assert_eq!(little, [0xF0, 0xDE, 0xBC, 0x9A, 0x78, 0x56, 0x34, 0x12]);

        let mut big = [0u8; 8];
        write_u64(&mut big, 0x123456789ABCDEF0, Endian::Big);
        assert_eq!(big, [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0]);
    }

    #[test]
    fn write_then_read_round_trips() {
        let mut a = [0u8; 2];
        write_u16(&mut a, 0xBEEF, Endian::Big);
        assert_eq!(read_u16(&a, Endian::Big), 0xBEEF);

        let mut b = [0u8; 4];
        write_u32(&mut b, 0xDEADBEEF, Endian::Little);
        assert_eq!(read_u32(&b, Endian::Little), 0xDEADBEEF);

        let mut c = [0u8; 8];
        write_u64(&mut c, 0x0011223344556677, Endian::Little);
        assert_eq!(read_u64(&c, Endian::Little), 0x0011223344556677);
    }
}
