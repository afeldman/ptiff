//! TIFF-variant LZW compression (Compression = 5).
//!
//! Mirrors `ptiff::compression::lzw.{hpp,cpp}`.

use std::collections::BTreeMap;

use crate::{Error, Result};

const CLEAR_CODE: u16 = 256;
const EOI_CODE: u16 = 257;
const FIRST_DICTIONARY_CODE: u16 = 258;
const MAX_DICTIONARY_CODE: u16 = 4094;

/// TIFF's "early change" code-width schedule: width grows one code sooner than
/// a naive scheme (at 511/1023/2047 instead of 512/1024/2048).
#[must_use]
const fn code_width_for(next_code: u16) -> u8 {
    if next_code >= 2047 {
        12
    } else if next_code >= 1023 {
        11
    } else if next_code >= 511 {
        10
    } else {
        9
    }
}

type Table = BTreeMap<(Vec<u8>, u8), u16>;

/// Returns the LZW code for a phrase: its own byte for a 1-byte phrase (codes
/// 0..255), or the table code assigned by the encoder for longer phrases
/// (258..4094).
fn code_for_byte_sequence(seq: &[u8], table: &Table) -> u16 {
    if seq.len() == 1 {
        return u16::from(seq[0]);
    }
    let prefix = seq[..seq.len() - 1].to_vec();
    table[&(prefix, seq[seq.len() - 1])]
}

/// Packs `codes` MSB-first, honoring the decoder's width transitions.
fn write_codes_to_bits(codes: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut bits_acc: u32 = 0;
    let mut bits_pending: u32 = 0;
    let mut table_size: u16 = FIRST_DICTIONARY_CODE;
    let mut have_previous = false;

    let read_width = |table_size: u16| code_width_for(table_size);

    for &code in codes {
        let w = read_width(table_size) as u32;
        let mask = (1u32 << w) - 1;
        bits_acc = (bits_acc << w) | (code & mask);
        bits_pending += w;
        while bits_pending >= 8 {
            let byte = ((bits_acc >> (bits_pending - 8)) & 0xFF) as u8;
            out.push(byte);
            bits_pending -= 8;
        }

        if code == u32::from(CLEAR_CODE) {
            table_size = FIRST_DICTIONARY_CODE;
            have_previous = false;
        } else if code != u32::from(EOI_CODE) {
            // Exactly mirrors the C++ oracle's bit-writer: the table grows only
            // once we already hold a previous normal code (mirrors the decoder's
            // `have_old_entry` gating), but `have_previous` is set unconditionally
            // for any non-clear/non-EOI code. Without the unconditional set the
            // table never grows, producing a stream the standard LZW decoder
            // (e.g. weezl/image-tiff) rejects.
            if have_previous && table_size < MAX_DICTIONARY_CODE {
                table_size += 1;
            }
            have_previous = true;
        }
    }
    // Emit any trailing zero-padded partial byte.
    if bits_pending > 0 {
        out.push(((bits_acc << (8 - bits_pending)) & 0xFF) as u8);
    }
    out
}

/// Decodes TIFF-variant LZW-compressed data: MSB-first bit packing, 9-bit codes
/// growing to 12 bits, clear code 256 and end-of-information code 257, and
/// TIFF's "early change" quirk (the code width grows one code earlier than
/// standard LZW/GIF). Decoding stops the instant exactly `expected_size` bytes
/// have been produced.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if a code references a
/// dictionary entry that doesn't exist, decoding would produce more than
/// `expected_size` bytes (decompression-bomb guard), the bitstream is exhausted
/// before an EOI code appears, or the decoded output is shorter than
/// `expected_size` once EOI is reached.
pub fn decode_lzw(input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    let mut table: Vec<Vec<u8>> = (0..256u32).map(|i| vec![i as u8]).collect();

    let mut bit_pos: usize = 0;
    let total_bits = input.len() * 8;

    let mut read_code = |width: u8, input: &[u8]| -> Result<u16> {
        let w = usize::from(width);
        if bit_pos + w > total_bits {
            return Err(Error::invalid_argument(
                "decodeLzw: bitstream exhausted before EOI",
            ));
        }
        let mut code: u16 = 0;
        for _ in 0..w {
            let byte_index = bit_pos / 8;
            let bit_index = 7 - (bit_pos % 8);
            let bit = (u16::from(input[byte_index]) >> bit_index) & 1;
            code = (code << 1) | bit;
            bit_pos += 1;
        }
        Ok(code)
    };

    let mut output = Vec::with_capacity(expected_size);
    let mut next_code: u16 = FIRST_DICTIONARY_CODE;
    let mut code_width: u8 = 9;
    let mut have_old_entry = false;
    let mut old_entry: Vec<u8> = Vec::new();

    loop {
        let code = read_code(code_width, input)?;

        if code == CLEAR_CODE {
            table.resize(FIRST_DICTIONARY_CODE as usize, Vec::new());
            next_code = FIRST_DICTIONARY_CODE;
            code_width = 9;
            have_old_entry = false;
            continue;
        }
        if code == EOI_CODE {
            break;
        }

        let entry: Vec<u8> =
            if u32::from(code) < 256 || (code >= FIRST_DICTIONARY_CODE && code < next_code) {
                table[code as usize].clone()
            } else if code == next_code && have_old_entry {
                let mut e = old_entry.clone();
                e.push(old_entry[0]);
                e
            } else {
                return Err(Error::invalid_argument(
                    "decodeLzw: invalid code in compressed stream",
                ));
            };

        if output.len() + entry.len() > expected_size {
            return Err(Error::invalid_argument(
                "decodeLzw: decoded output exceeds expectedSize",
            ));
        }
        output.extend_from_slice(&entry);

        if have_old_entry {
            if next_code > MAX_DICTIONARY_CODE {
                return Err(Error::invalid_argument(
                    "decodeLzw: dictionary exceeded 12-bit table",
                ));
            }
            let mut new_entry = old_entry.clone();
            new_entry.push(*entry.first().unwrap_or(&0));
            if next_code as usize >= table.len() {
                table.resize(next_code as usize + 1, Vec::new());
            }
            table[next_code as usize] = new_entry;
            next_code += 1;
            code_width = code_width_for(next_code);
        }

        old_entry = entry;
        have_old_entry = true;
    }

    if output.len() != expected_size {
        return Err(Error::invalid_argument(
            "decodeLzw: decoded output shorter than expectedSize",
        ));
    }
    Ok(output)
}

/// Encodes `input` with the TIFF-variant LZW, inverse of [`decode_lzw`]. Emits
/// an initial clear code 256, then greedy dictionary matches; the dictionary
/// grows from code 258 toward 4094 and is cleared when full. Codes are MSB-first
/// with the TIFF "early change" width schedule.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if `input.len()` exceeds
/// `u32::MAX` (the strip's `u32` byte count).
pub fn encode_lzw(input: &[u8]) -> Result<Vec<u8>> {
    if input.len() as u64 > u32::MAX as u64 {
        return Err(Error::invalid_argument(
            "encodeLzw: input exceeds uint32 strip byte count",
        ));
    }

    let mut codes: Vec<u32> = vec![u32::from(CLEAR_CODE)];
    let mut table: Table = BTreeMap::new();
    let mut next_code: u16 = FIRST_DICTIONARY_CODE;

    let reset_to_full = |table: &mut Table, next_code: &mut u16| {
        *next_code = FIRST_DICTIONARY_CODE;
        table.clear();
    };

    let n = input.len();
    let mut pos = 0usize;
    let mut current: Vec<u8> = Vec::new();
    if pos < n {
        current.push(input[pos]);
        pos += 1;
    }

    while pos < n {
        let next = input[pos];
        let key = (current.clone(), next);
        if table.contains_key(&key) {
            current.push(next);
            pos += 1;
        } else {
            codes.push(u32::from(code_for_byte_sequence(&current, &table)));
            if next_code < MAX_DICTIONARY_CODE {
                table.insert(key, next_code);
                next_code += 1;
                if next_code >= MAX_DICTIONARY_CODE {
                    codes.push(u32::from(CLEAR_CODE));
                    reset_to_full(&mut table, &mut next_code);
                }
            }
            current = vec![next];
            pos += 1;
        }
    }
    if !current.is_empty() {
        codes.push(u32::from(code_for_byte_sequence(&current, &table)));
    }
    codes.push(u32::from(EOI_CODE));

    Ok(write_codes_to_bits(&codes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_then_decode_round_trips() {
        // A small but not-trivial strip with repeated patterns.
        let data: Vec<u8> = b"abcabcabcabcXYZXYZXYZ".to_vec();
        let encoded = encode_lzw(&data).unwrap();
        let decoded = decode_lzw(&encoded, data.len()).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn encode_then_decode_round_trips_binary() {
        // Repetitive binary data exercises the dictionary well.
        let mut data = vec![0u8; 128];
        for i in 0..64 {
            data[i] = 0xAB;
            data[64 + i] = (i * 7) as u8;
        }
        let encoded = encode_lzw(&data).unwrap();
        let decoded = decode_lzw(&encoded, data.len()).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn encode_single_byte() {
        let data = vec![0x42u8];
        let encoded = encode_lzw(&data).unwrap();
        let decoded = decode_lzw(&encoded, data.len()).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn decode_rejects_expected_size_bomb() {
        // A short stream cannot produce more than its contents.
        let data: Vec<u8> = b"hello".to_vec();
        let encoded = encode_lzw(&data).unwrap();
        // Asking for many more bytes than decoded must fail the bomb guard.
        assert!(decode_lzw(&encoded, 10_000).is_err());
    }
}
