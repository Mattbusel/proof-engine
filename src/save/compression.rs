//! Compression algorithms for save data: RLE, LZ4-style, Huffman, and delta encoding.
//!
//! `Compressor::compress_auto` tries every algorithm and picks the smallest output,
//! storing the result in a self-describing `CompressedBlock`.

use std::collections::{BinaryHeap, HashMap};

// ─────────────────────────────────────────────────────────────────────────────
//  CompressionAlgorithm
// ─────────────────────────────────────────────────────────────────────────────

/// Which compression algorithm was used to produce a `CompressedBlock`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CompressionAlgorithm {
    /// Data is stored verbatim.
    None = 0,
    /// Run-length encoding.
    Rle = 1,
    /// LZ4-style byte-level LZ77 with a hash chain.
    Lz4Like = 2,
    /// Integer delta encoding followed by RLE.
    DeltaEncode = 3,
    /// Canonical Huffman coding (code lengths ≤ 15 bits).
    Huffman = 4,
}

impl CompressionAlgorithm {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::None),
            1 => Some(Self::Rle),
            2 => Some(Self::Lz4Like),
            3 => Some(Self::DeltaEncode),
            4 => Some(Self::Huffman),
            _ => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  CompressedBlock
// ─────────────────────────────────────────────────────────────────────────────

/// A self-describing compressed block.
///
/// Wire format: `[algorithm: u8][original_size: u32 LE][data…]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressedBlock {
    pub algorithm: CompressionAlgorithm,
    pub original_size: usize,
    pub data: Vec<u8>,
}

impl CompressedBlock {
    /// Serialise to bytes (includes the 5-byte header).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(5 + self.data.len());
        out.push(self.algorithm as u8);
        let sz = self.original_size as u32;
        out.extend_from_slice(&sz.to_le_bytes());
        out.extend_from_slice(&self.data);
        out
    }

    /// Deserialise from bytes produced by `to_bytes`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 5 {
            return Err("CompressedBlock too short".into());
        }
        let algorithm = CompressionAlgorithm::from_u8(bytes[0])
            .ok_or_else(|| format!("unknown algorithm byte {}", bytes[0]))?;
        let original_size =
            u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
        let data = bytes[5..].to_vec();
        Ok(Self { algorithm, original_size, data })
    }

    /// Decompress the block back to the original bytes.
    pub fn decompress(&self) -> Result<Vec<u8>, String> {
        match self.algorithm {
            CompressionAlgorithm::None => Ok(self.data.clone()),
            CompressionAlgorithm::Rle => RunLengthEncoder::decode(&self.data),
            CompressionAlgorithm::Lz4Like => Lz4Decoder::decompress(&self.data),
            CompressionAlgorithm::DeltaEncode => DeltaDecoder::decode_bytes(&self.data),
            CompressionAlgorithm::Huffman => HuffmanDecoder::decompress(&self.data),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  RunLengthEncoder
// ─────────────────────────────────────────────────────────────────────────────

/// Byte-level run-length encoding.
///
/// Format: alternating `[count: u8][byte]` pairs.  Counts are 1-based (a
/// count byte of 0 means 1 repetition).  Runs longer than 255 are split.
pub struct RunLengthEncoder;

impl RunLengthEncoder {
    /// Encode `input` bytes into RLE form.
    pub fn encode(input: &[u8]) -> Vec<u8> {
        if input.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(input.len());
        let mut i = 0;
        while i < input.len() {
            let byte = input[i];
            let mut run = 1usize;
            while i + run < input.len() && input[i + run] == byte && run < 255 {
                run += 1;
            }
            out.push(run as u8);
            out.push(byte);
            i += run;
        }
        out
    }

    /// Decode RLE bytes back to the original stream.
    pub fn decode(input: &[u8]) -> Result<Vec<u8>, String> {
        if input.len() % 2 != 0 {
            return Err("RLE input has odd length".into());
        }
        let mut out = Vec::new();
        let mut i = 0;
        while i + 1 < input.len() {
            let count = input[i] as usize;
            let byte = input[i + 1];
            for _ in 0..count {
                out.push(byte);
            }
            i += 2;
        }
        Ok(out)
    }

    /// Encode a u16 stream as bytes (little-endian pairs) then RLE-compress.
    pub fn encode_u16(input: &[u16]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(input.len() * 2);
        for &v in input {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        Self::encode(&bytes)
    }

    /// Decode an RLE-compressed byte stream back to u16 values.
    pub fn decode_u16(input: &[u8]) -> Result<Vec<u16>, String> {
        let bytes = Self::decode(input)?;
        if bytes.len() % 2 != 0 {
            return Err("decoded byte count is not even".into());
        }
        let mut out = Vec::with_capacity(bytes.len() / 2);
        for chunk in bytes.chunks_exact(2) {
            out.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        Ok(out)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Lz4Encoder / Lz4Decoder
// ─────────────────────────────────────────────────────────────────────────────

/// LZ4-style block compressor using a 4-byte hash chain.
///
/// Sequence format: `[token: u8][extra literal len…][literals…][offset: u16 LE][extra match len…]`
/// The token's high nibble is the literal count (0-14, 15 = overflow), low nibble is
/// match-length minus 4 (0-14, 15 = overflow).  Minimum match length is 4.
pub struct Lz4Encoder;

impl Lz4Encoder {
    const HASH_LOG: usize = 16;
    const HASH_SIZE: usize = 1 << Self::HASH_LOG;
    const MIN_MATCH: usize = 4;
    const WINDOW: usize = 65535;

    fn hash4(data: &[u8], pos: usize) -> usize {
        if pos + 4 > data.len() {
            return 0;
        }
        let v = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
        ((v.wrapping_mul(0x9E37_79B1)) >> (32 - Self::HASH_LOG)) as usize
    }

    fn write_varint(out: &mut Vec<u8>, mut n: usize) {
        while n >= 255 {
            out.push(255);
            n -= 255;
        }
        out.push(n as u8);
    }

    /// Compress `input` using LZ4-style block format.
    pub fn compress(input: &[u8]) -> Vec<u8> {
        let n = input.len();
        let mut out = Vec::with_capacity(n);
        let mut hash_table = vec![0usize; Self::HASH_SIZE];
        let mut pos = 0usize;
        let mut anchor = 0usize;

        while pos + Self::MIN_MATCH <= n {
            let h = Self::hash4(input, pos);
            let candidate = hash_table[h];
            hash_table[h] = pos;

            // Check for a valid match
            let match_ok = candidate < pos
                && pos - candidate <= Self::WINDOW
                && pos + Self::MIN_MATCH <= n
                && candidate + Self::MIN_MATCH <= n
                && input[pos..pos+Self::MIN_MATCH] == input[candidate..candidate+Self::MIN_MATCH];

            if !match_ok {
                pos += 1;
                continue;
            }

            // Extend match
            let mut match_len = Self::MIN_MATCH;
            while pos + match_len < n && candidate + match_len < pos {
                if input[pos + match_len] != input[candidate + match_len] {
                    break;
                }
                match_len += 1;
            }

            // Write sequence: literals + match
            let lit_len = pos - anchor;
            let ml_extra = match_len - Self::MIN_MATCH;

            let lit_tok = lit_len.min(15) as u8;
            let ml_tok  = ml_extra.min(15) as u8;
            out.push((lit_tok << 4) | ml_tok);

            if lit_len >= 15 {
                Self::write_varint(&mut out, lit_len - 15);
            }
            out.extend_from_slice(&input[anchor..pos]);

            let offset = (pos - candidate) as u16;
            out.extend_from_slice(&offset.to_le_bytes());

            if ml_extra >= 15 {
                Self::write_varint(&mut out, ml_extra - 15);
            }

            pos += match_len;
            anchor = pos;
        }

        // Last literals
        let lit_len = n - anchor;
        let lit_tok = lit_len.min(15) as u8;
        out.push(lit_tok << 4);
        if lit_len >= 15 {
            Self::write_varint(&mut out, lit_len - 15);
        }
        out.extend_from_slice(&input[anchor..]);

        out
    }
}

/// LZ4-style block decompressor.
pub struct Lz4Decoder;

impl Lz4Decoder {
    fn read_varint(data: &[u8], pos: &mut usize, base: usize) -> Result<usize, String> {
        let mut total = base;
        loop {
            if *pos >= data.len() {
                return Err("unexpected end of stream in varint".into());
            }
            let b = data[*pos] as usize;
            *pos += 1;
            total += b;
            if b < 255 {
                break;
            }
        }
        Ok(total)
    }

    /// Decompress an LZ4-style block.
    pub fn decompress(input: &[u8]) -> Result<Vec<u8>, String> {
        let mut out: Vec<u8> = Vec::new();
        let mut pos = 0usize;

        while pos < input.len() {
            let token = input[pos];
            pos += 1;

            // Literal length
            let mut lit_len = (token >> 4) as usize;
            if lit_len == 15 {
                lit_len = Self::read_varint(input, &mut pos, 15)?;
            }

            // Copy literals
            if pos + lit_len > input.len() {
                return Err("literal copy out of bounds".into());
            }
            out.extend_from_slice(&input[pos..pos + lit_len]);
            pos += lit_len;

            // End of block: last sequence has no match part
            if pos >= input.len() {
                break;
            }

            // Match offset
            if pos + 2 > input.len() {
                return Err("truncated match offset".into());
            }
            let offset = u16::from_le_bytes([input[pos], input[pos+1]]) as usize;
            pos += 2;
            if offset == 0 || offset > out.len() {
                return Err(format!("invalid match offset {offset}"));
            }

            // Match length
            let mut match_len = (token & 0x0F) as usize + 4;
            if (token & 0x0F) == 15 {
                match_len = Self::read_varint(input, &mut pos, match_len)?;
            }

            // Copy match (may overlap)
            let match_start = out.len() - offset;
            for i in 0..match_len {
                let b = out[match_start + i];
                out.push(b);
            }
        }

        Ok(out)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  HuffmanEncoder / HuffmanDecoder
// ─────────────────────────────────────────────────────────────────────────────

/// Node used when building the Huffman tree.
#[derive(Debug, Clone, Eq, PartialEq)]
struct HuffNode {
    freq: u64,
    symbol: Option<u8>,
    left: Option<Box<HuffNode>>,
    right: Option<Box<HuffNode>>,
}

impl Ord for HuffNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.freq.cmp(&self.freq) // min-heap
    }
}

impl PartialOrd for HuffNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Canonical Huffman encoder.
///
/// Output format:
/// `[original_len: u32 LE][code_lengths: 256 bytes][bit_stream…][padding_bits: u8]`
pub struct HuffmanEncoder;

impl HuffmanEncoder {
    const MAX_BITS: u32 = 15;

    /// Build frequency table.
    fn freq_table(data: &[u8]) -> [u64; 256] {
        let mut freq = [0u64; 256];
        for &b in data {
            freq[b as usize] += 1;
        }
        freq
    }

    /// Build the Huffman tree using a min-heap.
    fn build_tree(freq: &[u64; 256]) -> HuffNode {
        let mut heap: BinaryHeap<HuffNode> = BinaryHeap::new();
        for (sym, &f) in freq.iter().enumerate() {
            if f > 0 {
                heap.push(HuffNode { freq: f, symbol: Some(sym as u8), left: None, right: None });
            }
        }
        if heap.is_empty() {
            // All zeros – just push a dummy so the tree is non-empty
            heap.push(HuffNode { freq: 1, symbol: Some(0), left: None, right: None });
        }
        while heap.len() > 1 {
            let a = heap.pop().unwrap();
            let b = heap.pop().unwrap();
            heap.push(HuffNode {
                freq: a.freq + b.freq,
                symbol: None,
                left: Some(Box::new(a)),
                right: Some(Box::new(b)),
            });
        }
        heap.pop().unwrap()
    }

    /// Walk the tree to assign code lengths.
    fn assign_lengths(node: &HuffNode, depth: u32, lengths: &mut [u32; 256]) {
        if let Some(sym) = node.symbol {
            lengths[sym as usize] = depth.max(1);
        } else {
            if let Some(l) = &node.left  { Self::assign_lengths(l, depth + 1, lengths); }
            if let Some(r) = &node.right { Self::assign_lengths(r, depth + 1, lengths); }
        }
    }

    /// Limit code lengths to `MAX_BITS` using the package-merge approach (simplified: clamp + rebalance).
    fn limit_lengths(lengths: &mut [u32; 256]) {
        let mut over: i32 = 0;
        for l in lengths.iter_mut() {
            if *l > Self::MAX_BITS {
                over += (1 << (*l - Self::MAX_BITS)) - 1;
                *l = Self::MAX_BITS;
            }
        }
        // Remove `over` worth of codes by bumping some short lengths up by 1
        for l in lengths.iter_mut() {
            if *l > 0 && *l < Self::MAX_BITS && over > 0 {
                over -= 1;
                *l += 1;
            }
        }
    }

    /// Assign canonical codes from lengths.
    fn canonical_codes(lengths: &[u32; 256]) -> [u32; 256] {
        let mut bl_count = [0u32; 16];
        for &l in lengths.iter() {
            if l > 0 {
                bl_count[l as usize] += 1;
            }
        }
        let mut next_code = [0u32; 16];
        let mut code = 0u32;
        for bits in 1..=15usize {
            code = (code + bl_count[bits - 1]) << 1;
            next_code[bits] = code;
        }
        let mut codes = [0u32; 256];
        let mut order: Vec<usize> = (0..256).filter(|&i| lengths[i] > 0).collect();
        order.sort_by_key(|&i| (lengths[i], i));
        for i in order {
            let l = lengths[i] as usize;
            codes[i] = next_code[l];
            next_code[l] += 1;
        }
        codes
    }

    /// Compress `data` using canonical Huffman coding.
    pub fn compress(data: &[u8]) -> Vec<u8> {
        if data.is_empty() {
            let mut out = vec![0u8; 4 + 256 + 1];
            return out;
        }
        let freq = Self::freq_table(data);
        let tree = Self::build_tree(&freq);
        let mut lengths = [0u32; 256];
        Self::assign_lengths(&tree, 0, &mut lengths);
        Self::limit_lengths(&mut lengths);
        let codes = Self::canonical_codes(&lengths);

        let orig_len = data.len() as u32;
        let mut out = Vec::with_capacity(4 + 256 + data.len());
        out.extend_from_slice(&orig_len.to_le_bytes());
        for &l in &lengths {
            out.push(l as u8);
        }

        // Bit-pack the encoded stream
        let mut bit_buf = 0u32;
        let mut bit_count = 0u32;
        let mut encoded_bits: Vec<u8> = Vec::new();

        for &byte in data {
            let code = codes[byte as usize];
            let len  = lengths[byte as usize];
            bit_buf |= code << (32 - len - bit_count);
            bit_count += len;
            while bit_count >= 8 {
                encoded_bits.push((bit_buf >> 24) as u8);
                bit_buf <<= 8;
                bit_count -= 8;
            }
        }
        let padding = if bit_count > 0 { 8 - bit_count } else { 0 };
        if bit_count > 0 {
            encoded_bits.push((bit_buf >> 24) as u8);
        }

        out.extend_from_slice(&encoded_bits);
        out.push(padding as u8);
        out
    }
}

/// Canonical Huffman decoder.
pub struct HuffmanDecoder;

impl HuffmanDecoder {
    /// Decompress data produced by `HuffmanEncoder::compress`.
    pub fn decompress(input: &[u8]) -> Result<Vec<u8>, String> {
        if input.len() < 4 + 256 + 1 {
            return Err("Huffman block too short".into());
        }
        let orig_len =
            u32::from_le_bytes([input[0], input[1], input[2], input[3]]) as usize;
        if orig_len == 0 {
            return Ok(Vec::new());
        }
        let mut lengths = [0u32; 256];
        for i in 0..256 {
            lengths[i] = input[4 + i] as u32;
        }

        // Rebuild canonical codes
        let codes = HuffmanEncoder::canonical_codes(&lengths);

        // Build decode table: (code, length) → symbol
        let mut decode_table: HashMap<(u32, u32), u8> = HashMap::new();
        for (sym, &l) in lengths.iter().enumerate() {
            if l > 0 {
                decode_table.insert((codes[sym], l), sym as u8);
            }
        }

        let payload = &input[4 + 256..];
        if payload.is_empty() {
            return Err("Huffman block has no payload".into());
        }
        let padding = *payload.last().unwrap() as u32;
        let bit_data = &payload[..payload.len() - 1];

        let mut out = Vec::with_capacity(orig_len);
        let mut bit_buf = 0u32;
        let mut bits_available = 0u32;
        let mut byte_idx = 0usize;

        let mut fill = |bit_buf: &mut u32, bits_available: &mut u32| {
            while *bits_available < 24 && byte_idx < bit_data.len() {
                *bit_buf |= (bit_data[byte_idx] as u32) << (24 - *bits_available);
                *bits_available += 8;
                byte_idx += 1;
            }
        };

        while out.len() < orig_len {
            fill(&mut bit_buf, &mut bits_available);
            if bits_available == 0 {
                break;
            }
            let mut found = false;
            for try_len in 1..=15u32 {
                if bits_available < try_len {
                    if bits_available + padding >= try_len {
                        // padding bits
                        break;
                    }
                    break;
                }
                let candidate = bit_buf >> (32 - try_len);
                if let Some(&sym) = decode_table.get(&(candidate, try_len)) {
                    out.push(sym);
                    bit_buf <<= try_len;
                    bits_available -= try_len;
                    found = true;
                    break;
                }
            }
            if !found {
                break;
            }
        }

        if out.len() != orig_len {
            return Err(format!("Huffman decode produced {} bytes, expected {}", out.len(), orig_len));
        }
        Ok(out)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  DeltaEncoder / DeltaDecoder
// ─────────────────────────────────────────────────────────────────────────────

/// Delta-encode sorted integer arrays then apply RLE on the deltas.
///
/// Useful for compact storage of entity ID lists.
pub struct DeltaEncoder;

impl DeltaEncoder {
    /// Delta-encode a sorted slice of `u32` values, then RLE-compress the result.
    ///
    /// Wire: first encode `[count: u32 LE]`, then for each delta `[delta: u32 LE]`,
    /// wrap the whole byte stream with RLE.
    pub fn encode(values: &[u32]) -> Vec<u8> {
        let mut raw = Vec::with_capacity(4 + values.len() * 4);
        let count = values.len() as u32;
        raw.extend_from_slice(&count.to_le_bytes());
        let mut prev = 0u32;
        for &v in values {
            let delta = v.wrapping_sub(prev);
            raw.extend_from_slice(&delta.to_le_bytes());
            prev = v;
        }
        RunLengthEncoder::encode(&raw)
    }

    /// Encode bytes directly with delta+RLE (treats bytes as u8 deltas).
    pub fn encode_bytes(data: &[u8]) -> Vec<u8> {
        let mut raw = Vec::with_capacity(data.len());
        let mut prev = 0u8;
        for &b in data {
            raw.push(b.wrapping_sub(prev));
            prev = b;
        }
        RunLengthEncoder::encode(&raw)
    }
}

/// Delta decoder.
pub struct DeltaDecoder;

impl DeltaDecoder {
    /// Decode a stream produced by `DeltaEncoder::encode`.
    pub fn decode(input: &[u8]) -> Result<Vec<u32>, String> {
        let raw = RunLengthEncoder::decode(input)?;
        if raw.len() < 4 {
            return Err("DeltaDecoder: too short".into());
        }
        let count = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
        if raw.len() < 4 + count * 4 {
            return Err("DeltaDecoder: truncated".into());
        }
        let mut out = Vec::with_capacity(count);
        let mut prev = 0u32;
        for i in 0..count {
            let off = 4 + i * 4;
            let delta = u32::from_le_bytes([raw[off], raw[off+1], raw[off+2], raw[off+3]]);
            let v = prev.wrapping_add(delta);
            out.push(v);
            prev = v;
        }
        Ok(out)
    }

    /// Decode a byte stream produced by `DeltaEncoder::encode_bytes`.
    pub fn decode_bytes(input: &[u8]) -> Result<Vec<u8>, String> {
        let raw = RunLengthEncoder::decode(input)?;
        let mut out = Vec::with_capacity(raw.len());
        let mut prev = 0u8;
        for &delta in &raw {
            let v = prev.wrapping_add(delta);
            out.push(v);
            prev = v;
        }
        Ok(out)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Compressor  (compress_auto)
// ─────────────────────────────────────────────────────────────────────────────

/// High-level compressor: tries every algorithm and picks the smallest result.
pub struct Compressor;

impl Compressor {
    /// Compress `data` using each available algorithm, return the smallest `CompressedBlock`.
    pub fn compress_auto(data: &[u8]) -> CompressedBlock {
        let original_size = data.len();

        let candidates: Vec<(CompressionAlgorithm, Vec<u8>)> = vec![
            (CompressionAlgorithm::None,        data.to_vec()),
            (CompressionAlgorithm::Rle,         RunLengthEncoder::encode(data)),
            (CompressionAlgorithm::Lz4Like,     Lz4Encoder::compress(data)),
            (CompressionAlgorithm::DeltaEncode, DeltaEncoder::encode_bytes(data)),
            (CompressionAlgorithm::Huffman,     HuffmanEncoder::compress(data)),
        ];

        let best = candidates
            .into_iter()
            .min_by_key(|(_, d)| d.len())
            .unwrap();

        CompressedBlock { algorithm: best.0, original_size, data: best.1 }
    }

    /// Compress with a specific algorithm.
    pub fn compress_with(data: &[u8], algo: CompressionAlgorithm) -> CompressedBlock {
        let original_size = data.len();
        let compressed = match algo {
            CompressionAlgorithm::None        => data.to_vec(),
            CompressionAlgorithm::Rle         => RunLengthEncoder::encode(data),
            CompressionAlgorithm::Lz4Like     => Lz4Encoder::compress(data),
            CompressionAlgorithm::DeltaEncode => DeltaEncoder::encode_bytes(data),
            CompressionAlgorithm::Huffman     => HuffmanEncoder::compress(data),
        };
        CompressedBlock { algorithm: algo, original_size, data: compressed }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip_rle(input: &[u8]) {
        let encoded = RunLengthEncoder::encode(input);
        let decoded = RunLengthEncoder::decode(&encoded).expect("RLE decode failed");
        assert_eq!(&decoded, input, "RLE roundtrip failed for {:?}", input);
    }

    #[test]
    fn test_rle_empty() {
        roundtrip_rle(&[]);
    }

    #[test]
    fn test_rle_no_runs() {
        roundtrip_rle(&[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_rle_long_run() {
        let input: Vec<u8> = vec![42u8; 300];
        roundtrip_rle(&input);
    }

    #[test]
    fn test_rle_mixed() {
        roundtrip_rle(&[0, 0, 0, 1, 2, 2, 3]);
    }

    #[test]
    fn test_rle_u16_roundtrip() {
        let values: Vec<u16> = vec![0, 100, 200, 200, 300, 300, 300];
        let encoded = RunLengthEncoder::encode_u16(&values);
        let decoded = RunLengthEncoder::decode_u16(&encoded).expect("u16 decode failed");
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_lz4_empty() {
        let compressed = Lz4Encoder::compress(&[]);
        let decompressed = Lz4Decoder::decompress(&compressed).expect("lz4 decompress failed");
        assert_eq!(decompressed, &[]);
    }

    #[test]
    fn test_lz4_roundtrip_simple() {
        let input = b"hello world hello world hello world";
        let compressed = Lz4Encoder::compress(input);
        let decompressed = Lz4Decoder::decompress(&compressed).expect("lz4 decompress failed");
        assert_eq!(&decompressed, input);
    }

    #[test]
    fn test_lz4_roundtrip_random_like() {
        let input: Vec<u8> = (0u8..=255).cycle().take(512).collect();
        let compressed = Lz4Encoder::compress(&input);
        let decompressed = Lz4Decoder::decompress(&compressed).expect("lz4 decompress failed");
        assert_eq!(decompressed, input);
    }

    #[test]
    fn test_huffman_roundtrip() {
        let input = b"abracadabra";
        let compressed = HuffmanEncoder::compress(input);
        let decompressed = HuffmanDecoder::decompress(&compressed).expect("huffman decompress failed");
        assert_eq!(&decompressed, input);
    }

    #[test]
    fn test_huffman_uniform() {
        let input: Vec<u8> = vec![0xAAu8; 100];
        let compressed = HuffmanEncoder::compress(&input);
        let decompressed = HuffmanDecoder::decompress(&compressed).expect("huffman decompress failed");
        assert_eq!(decompressed, input);
    }

    #[test]
    fn test_delta_encode_roundtrip() {
        let values = vec![0u32, 5, 10, 15, 100, 200, 201];
        let encoded = DeltaEncoder::encode(&values);
        let decoded = DeltaDecoder::decode(&encoded).expect("delta decode failed");
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_delta_bytes_roundtrip() {
        let data: Vec<u8> = (0u8..50).collect();
        let encoded = DeltaEncoder::encode_bytes(&data);
        let decoded = DeltaDecoder::decode_bytes(&encoded).expect("delta bytes decode failed");
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_compressed_block_serialization() {
        let data = b"save data payload for block test";
        let block = Compressor::compress_auto(data);
        let bytes = block.to_bytes();
        let restored = CompressedBlock::from_bytes(&bytes).expect("from_bytes failed");
        let decompressed = restored.decompress().expect("decompress failed");
        assert_eq!(&decompressed, data);
    }

    #[test]
    fn test_compress_auto_correctness() {
        let data: Vec<u8> = b"the quick brown fox jumps over the lazy dog".to_vec();
        let block = Compressor::compress_auto(&data);
        let decompressed = block.decompress().expect("auto decompress failed");
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_compress_with_all_algorithms() {
        let data: Vec<u8> = vec![1, 2, 3, 4, 5, 1, 2, 3, 4, 5, 1, 2, 3, 4, 5];
        for algo in [
            CompressionAlgorithm::None,
            CompressionAlgorithm::Rle,
            CompressionAlgorithm::Lz4Like,
            CompressionAlgorithm::DeltaEncode,
            CompressionAlgorithm::Huffman,
        ] {
            let block = Compressor::compress_with(&data, algo);
            let decompressed = block.decompress()
                .unwrap_or_else(|e| panic!("decompress failed for {:?}: {}", algo, e));
            assert_eq!(decompressed, data, "mismatch for {:?}", algo);
        }
    }
}
