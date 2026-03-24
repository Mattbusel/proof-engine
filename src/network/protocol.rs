//! Binary network protocol for Proof Engine multiplayer.
//!
//! Implements: packet framing, reliable/unreliable channels, fragmentation,
//! compact binary serialisation (LEB128, quantised floats, delta positions),
//! and a simple run-length compressor for snapshot payloads.

use std::collections::{HashMap, VecDeque};

// ── Constants ─────────────────────────────────────────────────────────────────

pub const MAX_PACKET_SIZE:  usize = 1400; // MTU-safe UDP payload
pub const HEADER_SIZE:      usize = 14;   // sizeof PacketHeader on wire
pub const MAX_FRAGMENT_DATA: usize = MAX_PACKET_SIZE - HEADER_SIZE - 4; // 4 bytes frag header
pub const MAX_SEQUENCE_GAP: u16   = 32768;

// ── PacketType ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    Handshake  = 0,
    Heartbeat  = 1,
    Disconnect = 2,
    Reliable   = 3,
    Unreliable = 4,
    Fragment   = 5,
    Ack        = 6,
}

impl PacketType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(PacketType::Handshake),
            1 => Some(PacketType::Heartbeat),
            2 => Some(PacketType::Disconnect),
            3 => Some(PacketType::Reliable),
            4 => Some(PacketType::Unreliable),
            5 => Some(PacketType::Fragment),
            6 => Some(PacketType::Ack),
            _ => None,
        }
    }

    pub fn is_reliable(self) -> bool {
        matches!(self, PacketType::Reliable | PacketType::Handshake)
    }
}

// ── PacketHeader ──────────────────────────────────────────────────────────────
//
// Wire layout (14 bytes):
//   [0..4]  magic        [u8; 4]  = [0x50, 0x52, 0x46, 0x45]  "PRFE"
//   [4]     version      u8
//   [5]     packet_type  u8
//   [6..8]  sequence     u16 LE
//   [8..10] ack          u16 LE
//   [10..14] ack_bits    u32 LE

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketHeader {
    pub magic:       [u8; 4],
    pub version:     u8,
    pub packet_type: PacketType,
    pub sequence:    u16,
    pub ack:         u16,
    pub ack_bits:    u32,
}

impl PacketHeader {
    pub const MAGIC: [u8; 4] = [0x50, 0x52, 0x46, 0x45]; // "PRFE"
    pub const CURRENT_VERSION: u8 = 1;
    pub const WIRE_SIZE: usize = 14;

    pub fn new(packet_type: PacketType, sequence: u16, ack: u16, ack_bits: u32) -> Self {
        PacketHeader {
            magic:       Self::MAGIC,
            version:     Self::CURRENT_VERSION,
            packet_type,
            sequence,
            ack,
            ack_bits,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::WIRE_SIZE);
        out.extend_from_slice(&self.magic);
        out.push(self.version);
        out.push(self.packet_type as u8);
        out.extend_from_slice(&self.sequence.to_le_bytes());
        out.extend_from_slice(&self.ack.to_le_bytes());
        out.extend_from_slice(&self.ack_bits.to_le_bytes());
        out
    }

    pub fn deserialize(data: &[u8]) -> Option<Self> {
        if data.len() < Self::WIRE_SIZE { return None; }
        let magic = [data[0], data[1], data[2], data[3]];
        if magic != Self::MAGIC { return None; }
        let version     = data[4];
        let packet_type = PacketType::from_u8(data[5])?;
        let sequence    = u16::from_le_bytes([data[6],  data[7]]);
        let ack         = u16::from_le_bytes([data[8],  data[9]]);
        let ack_bits    = u32::from_le_bytes([data[10], data[11], data[12], data[13]]);
        Some(PacketHeader { magic, version, packet_type, sequence, ack, ack_bits })
    }

    pub fn payload<'a>(&self, packet: &'a [u8]) -> &'a [u8] {
        if packet.len() > Self::WIRE_SIZE {
            &packet[Self::WIRE_SIZE..]
        } else {
            &[]
        }
    }
}

// ── Sequence helpers ──────────────────────────────────────────────────────────

/// Returns true if `a` is more recent than `b` (wrapping comparison).
pub fn sequence_greater_than(a: u16, b: u16) -> bool {
    let diff = a.wrapping_sub(b);
    diff > 0 && diff < MAX_SEQUENCE_GAP
}

/// Wrapping difference: how many steps newer is `a` than `b`.
pub fn sequence_diff(a: u16, b: u16) -> i32 {
    let raw = a.wrapping_sub(b) as i32;
    if raw > 32767 { raw - 65536 } else { raw }
}

// ── ReliableChannel ───────────────────────────────────────────────────────────

/// Reliable ordered channel over unreliable transport.
///
/// Tracks local/remote sequences, ack bitmask, and per-message send queues
/// for retransmission. RTT and packet loss are estimated from ack timing.
pub struct ReliableChannel {
    pub send_queue:      VecDeque<(u16, Vec<u8>)>,
    pub recv_buffer:     HashMap<u16, Vec<u8>>,
    pub local_sequence:  u16,
    pub remote_sequence: u16,
    pub ack_bits:        u32,

    /// Tracks when each packet was sent (sequence → send_time) for RTT calc
    send_times:    HashMap<u16, f64>,
    /// Running RTT estimate in milliseconds
    pub rtt_ms:    f32,
    /// Estimated packet loss 0..1
    pub packet_loss: f32,

    /// Monotonic clock value — updated externally each tick
    current_time:  f64,

    /// Retransmission timeout in seconds
    rto_secs:      f32,

    /// Packets pending ack: sequence → (payload, send_time, retry_count)
    pending_acks:  HashMap<u16, (Vec<u8>, f64, u32)>,

    lost_count:    u64,
    sent_count:    u64,
}

impl ReliableChannel {
    pub fn new() -> Self {
        ReliableChannel {
            send_queue:      VecDeque::new(),
            recv_buffer:     HashMap::new(),
            local_sequence:  0,
            remote_sequence: 0,
            ack_bits:        0,
            send_times:      HashMap::new(),
            rtt_ms:          50.0,
            packet_loss:     0.0,
            current_time:    0.0,
            rto_secs:        0.25,
            pending_acks:    HashMap::new(),
            lost_count:      0,
            sent_count:      0,
        }
    }

    /// Update the current time (call each tick).
    pub fn set_time(&mut self, t: f64) {
        self.current_time = t;
    }

    /// Queue a payload for reliable delivery. Returns the sequence number assigned.
    pub fn send(&mut self, payload: Vec<u8>) -> u16 {
        let seq = self.local_sequence;
        self.local_sequence = self.local_sequence.wrapping_add(1);
        self.send_queue.push_back((seq, payload.clone()));
        self.pending_acks.insert(seq, (payload, self.current_time, 0));
        self.send_times.insert(seq, self.current_time);
        self.sent_count += 1;
        seq
    }

    /// Receive a packet with the given sequence. Returns the payload if it
    /// is the next expected message, otherwise buffers it for in-order delivery.
    pub fn receive(&mut self, sequence: u16, payload: Vec<u8>) -> Option<Vec<u8>> {
        self.update_ack(sequence);
        let next = self.remote_sequence;
        if sequence == next {
            self.remote_sequence = self.remote_sequence.wrapping_add(1);
            // Deliver any buffered packets that follow
            while self.recv_buffer.contains_key(&self.remote_sequence) {
                self.remote_sequence = self.remote_sequence.wrapping_add(1);
            }
            Some(payload)
        } else if sequence_greater_than(sequence, next) {
            // Out-of-order: buffer
            self.recv_buffer.insert(sequence, payload);
            None
        } else {
            // Duplicate / old — discard
            None
        }
    }

    /// Process an incoming ack header. Retires confirmed packets and updates RTT.
    pub fn process_ack(&mut self, ack: u16, ack_bits: u32) {
        self.ack_single(ack);
        for i in 0..32u16 {
            if ack_bits & (1 << i) != 0 {
                let seq = ack.wrapping_sub(i + 1);
                self.ack_single(seq);
            }
        }
        // Detect lost packets (pending for more than 2× RTO)
        let threshold = self.current_time - (self.rto_secs as f64 * 2.0);
        let lost: Vec<u16> = self.pending_acks
            .iter()
            .filter(|(_, (_, t, retries))| *t < threshold && *retries >= 3)
            .map(|(seq, _)| *seq)
            .collect();
        for seq in lost {
            self.pending_acks.remove(&seq);
            self.lost_count += 1;
        }
        // Update loss estimate
        let total = self.sent_count.max(1) as f32;
        self.packet_loss = self.lost_count as f32 / total;
    }

    fn ack_single(&mut self, seq: u16) {
        if let Some((_, send_time, _)) = self.pending_acks.remove(&seq) {
            let rtt_sample = ((self.current_time - send_time) * 1000.0) as f32;
            // EWMA
            self.rtt_ms = self.rtt_ms + 0.125 * (rtt_sample - self.rtt_ms);
            self.rto_secs = (self.rtt_ms / 1000.0 * 2.0).max(0.05).min(2.0);
        }
    }

    /// Collect packets that need retransmission.
    pub fn get_retransmissions(&mut self) -> Vec<(u16, Vec<u8>)> {
        let threshold = self.current_time - self.rto_secs as f64;
        let mut result = Vec::new();
        for (seq, (payload, send_time, retries)) in &mut self.pending_acks {
            if *send_time < threshold && *retries < 8 {
                *send_time = self.current_time;
                *retries += 1;
                result.push((*seq, payload.clone()));
            }
        }
        result
    }

    /// Update ack_bits to record that we received `sequence`.
    pub fn update_ack(&mut self, sequence: u16) {
        if sequence_greater_than(sequence, self.remote_sequence) {
            let shift = sequence_diff(sequence, self.remote_sequence) as u32;
            if shift < 32 {
                self.ack_bits = (self.ack_bits << shift) | (1 << (shift - 1));
            } else {
                self.ack_bits = 0;
            }
        } else {
            let diff = sequence_diff(self.remote_sequence, sequence) as u32;
            if diff > 0 && diff <= 32 {
                self.ack_bits |= 1 << (diff - 1);
            }
        }
    }

    pub fn is_acked(&self, sequence: u16) -> bool {
        !self.pending_acks.contains_key(&sequence)
    }

    pub fn pending_count(&self) -> usize {
        self.pending_acks.len()
    }

    pub fn drain_send_queue(&mut self) -> Vec<(u16, Vec<u8>)> {
        self.send_queue.drain(..).collect()
    }
}

impl Default for ReliableChannel {
    fn default() -> Self { Self::new() }
}

// ── FragmentSystem ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Fragment {
    pub id:    u16,
    pub index: u8,
    pub total: u8,
    pub data:  Vec<u8>,
}

impl Fragment {
    /// Wire header size for a fragment (2 id + 1 index + 1 total = 4 bytes).
    pub const HEADER_SIZE: usize = 4;

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::HEADER_SIZE + self.data.len());
        out.extend_from_slice(&self.id.to_le_bytes());
        out.push(self.index);
        out.push(self.total);
        out.extend_from_slice(&self.data);
        out
    }

    pub fn deserialize(data: &[u8]) -> Option<Self> {
        if data.len() < Self::HEADER_SIZE { return None; }
        let id    = u16::from_le_bytes([data[0], data[1]]);
        let index = data[2];
        let total = data[3];
        let frag_data = data[4..].to_vec();
        Some(Fragment { id, index, total, data: frag_data })
    }
}

/// Splits large messages into MTU-safe fragments and reassembles them.
pub struct FragmentSystem {
    /// in-progress reassembly: message_id → (total_fragments, received_pieces)
    assembly: HashMap<u16, (u8, Vec<Option<Vec<u8>>>)>,
    next_message_id: u16,
}

impl FragmentSystem {
    pub fn new() -> Self {
        FragmentSystem {
            assembly:        HashMap::new(),
            next_message_id: 0,
        }
    }

    /// Split `data` into fragments. Returns `vec![data]` if it fits in one packet.
    pub fn fragment(&mut self, data: Vec<u8>) -> Vec<Fragment> {
        let chunk_size = MAX_FRAGMENT_DATA;
        let total_chunks = (data.len() + chunk_size - 1) / chunk_size;

        if total_chunks == 1 {
            let id = self.alloc_id();
            return vec![Fragment { id, index: 0, total: 1, data }];
        }

        let id = self.alloc_id();
        let total = total_chunks.min(255) as u8;
        data.chunks(chunk_size)
            .enumerate()
            .map(|(i, chunk)| Fragment {
                id,
                index: i as u8,
                total,
                data: chunk.to_vec(),
            })
            .collect()
    }

    /// Feed a received fragment. Returns the reassembled message when complete.
    pub fn reassemble(&mut self, frag: Fragment) -> Option<Vec<u8>> {
        let entry = self.assembly
            .entry(frag.id)
            .or_insert_with(|| {
                let total = frag.total as usize;
                (frag.total, vec![None; total])
            });

        let (total, pieces) = entry;
        if frag.index as usize >= *total as usize { return None; }
        pieces[frag.index as usize] = Some(frag.data);

        // Check if complete
        if pieces.iter().all(|p| p.is_some()) {
            let mut out = Vec::new();
            for piece in pieces.iter() {
                out.extend_from_slice(piece.as_ref().unwrap());
            }
            self.assembly.remove(&frag.id);
            Some(out)
        } else {
            None
        }
    }

    /// Remove stale in-progress assemblies (call periodically).
    pub fn prune_stale(&mut self, keep_ids: &[u16]) {
        self.assembly.retain(|id, _| keep_ids.contains(id));
    }

    fn alloc_id(&mut self) -> u16 {
        let id = self.next_message_id;
        self.next_message_id = self.next_message_id.wrapping_add(1);
        id
    }

    pub fn in_progress_count(&self) -> usize { self.assembly.len() }
}

impl Default for FragmentSystem {
    fn default() -> Self { Self::new() }
}

// ── Quantised floats ──────────────────────────────────────────────────────────

/// Quantise a float in [min, max] to a fixed-point integer with `bits` of precision.
pub fn quantize_float(v: f32, min: f32, max: f32, bits: u32) -> u32 {
    let range = max - min;
    if range <= 0.0 { return 0; }
    let max_val = ((1u64 << bits) - 1) as f32;
    let clamped = v.max(min).min(max);
    ((clamped - min) / range * max_val) as u32
}

/// Recover a float from a quantised value.
pub fn dequantize_float(v: u32, min: f32, max: f32, bits: u32) -> f32 {
    let max_val = ((1u64 << bits) - 1) as f32;
    let t = v as f32 / max_val;
    min + t * (max - min)
}

// ── Serializer ────────────────────────────────────────────────────────────────

/// Compact binary encoder/decoder for network messages.
pub struct Serializer {
    pub buf: Vec<u8>,
}

impl Serializer {
    pub fn new() -> Self { Serializer { buf: Vec::new() } }
    pub fn with_capacity(cap: usize) -> Self { Serializer { buf: Vec::with_capacity(cap) } }

    // ── Write primitives ─────────────────────────────────────────────────────

    pub fn write_u8(&mut self, v: u8)   { self.buf.push(v); }
    pub fn write_u16(&mut self, v: u16) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn write_u32(&mut self, v: u32) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn write_u64(&mut self, v: u64) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn write_i8(&mut self,  v: i8)  { self.buf.push(v as u8); }
    pub fn write_i16(&mut self, v: i16) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn write_i32(&mut self, v: i32) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn write_f32(&mut self, v: f32) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn write_f64(&mut self, v: f64) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn write_bool(&mut self, v: bool) { self.buf.push(if v { 1 } else { 0 }); }

    /// Write a byte slice prefixed with its u16 length.
    pub fn write_bytes(&mut self, data: &[u8]) {
        self.write_u16(data.len() as u16);
        self.buf.extend_from_slice(data);
    }

    /// Write a UTF-8 string prefixed with its u16 byte length.
    pub fn write_str(&mut self, s: &str) {
        self.write_bytes(s.as_bytes());
    }

    /// LEB128 variable-length unsigned integer encoding.
    pub fn write_varint(&mut self, mut v: u64) {
        loop {
            let byte = (v & 0x7F) as u8;
            v >>= 7;
            if v == 0 {
                self.buf.push(byte);
                break;
            } else {
                self.buf.push(byte | 0x80);
            }
        }
    }

    /// LEB128 signed integer (zigzag-encoded).
    pub fn write_varsint(&mut self, v: i64) {
        let encoded = ((v << 1) ^ (v >> 63)) as u64;
        self.write_varint(encoded);
    }

    /// Write Vec3 as three f32 (12 bytes).
    pub fn write_vec3(&mut self, v: [f32; 3]) {
        self.write_f32(v[0]);
        self.write_f32(v[1]);
        self.write_f32(v[2]);
    }

    /// Write Vec3 as three u16 half-precision floats (6 bytes, quantised ±4096 range).
    pub fn write_vec3_quantized(&mut self, v: [f32; 3], range: f32) {
        for component in &v {
            let q = quantize_float(*component, -range, range, 16);
            self.write_u16(q as u16);
        }
    }

    /// Write a Vec3 as delta from a reference position (quantised i16 offsets, ±32m at mm precision).
    pub fn write_vec3_delta(&mut self, current: [f32; 3], reference: [f32; 3]) {
        for i in 0..3 {
            let diff = current[i] - reference[i];
            // Quantise to i16: 1 unit = 1mm over ±32.767m
            let quant = (diff * 1000.0).round().max(i16::MIN as f32).min(i16::MAX as f32) as i16;
            self.write_i16(quant);
        }
    }

    /// Write quaternion as four f32 (16 bytes).
    pub fn write_quat(&mut self, q: [f32; 4]) {
        for &v in &q { self.write_f32(v); }
    }

    /// Write the largest-component compressed quaternion (smallest-3, 10 bits each = 32 bits total).
    pub fn write_quat_compressed(&mut self, q: [f32; 4]) {
        // Find the component with largest absolute value
        let abs_q: [f32; 4] = [q[0].abs(), q[1].abs(), q[2].abs(), q[3].abs()];
        let largest = abs_q.iter().enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(3);

        let sign = if q[largest] < 0.0 { -1.0f32 } else { 1.0 };
        let mut small = [0u32; 3];
        let mut j = 0;
        for i in 0..4 {
            if i != largest {
                let v = q[i] * sign;
                small[j] = quantize_float(v, -0.70711, 0.70711, 10);
                j += 1;
            }
        }
        // Pack: 2 bits for largest index, 10 bits × 3 = 32 bits
        let packed = ((largest as u32) << 30)
            | (small[0] << 20)
            | (small[1] << 10)
            | small[2];
        self.write_u32(packed);
    }

    pub fn finish(self) -> Vec<u8> { self.buf }
    pub fn len(&self) -> usize { self.buf.len() }
    pub fn is_empty(&self) -> bool { self.buf.is_empty() }
}

impl Default for Serializer {
    fn default() -> Self { Self::new() }
}

// ── Deserializer ──────────────────────────────────────────────────────────────

/// Cursor-based binary decoder matching Serializer's encoding.
pub struct Deserializer<'a> {
    data:   &'a [u8],
    cursor: usize,
}

impl<'a> Deserializer<'a> {
    pub fn new(data: &'a [u8]) -> Self { Deserializer { data, cursor: 0 } }

    pub fn remaining(&self) -> usize { self.data.len() - self.cursor }
    pub fn is_empty(&self) -> bool   { self.cursor >= self.data.len() }
    pub fn position(&self) -> usize  { self.cursor }

    fn read_bytes_raw(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.cursor + n > self.data.len() { return None; }
        let s = &self.data[self.cursor..self.cursor + n];
        self.cursor += n;
        Some(s)
    }

    pub fn read_u8(&mut self) -> Option<u8> {
        self.read_bytes_raw(1).map(|b| b[0])
    }
    pub fn read_u16(&mut self) -> Option<u16> {
        self.read_bytes_raw(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    pub fn read_u32(&mut self) -> Option<u32> {
        self.read_bytes_raw(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn read_u64(&mut self) -> Option<u64> {
        self.read_bytes_raw(8).map(|b| {
            u64::from_le_bytes([b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7]])
        })
    }
    pub fn read_i8(&mut self)  -> Option<i8>  { self.read_u8().map(|v| v as i8) }
    pub fn read_i16(&mut self) -> Option<i16> { self.read_u16().map(|v| v as i16) }
    pub fn read_i32(&mut self) -> Option<i32> { self.read_u32().map(|v| v as i32) }
    pub fn read_f32(&mut self) -> Option<f32> {
        self.read_bytes_raw(4).map(|b| f32::from_le_bytes([b[0],b[1],b[2],b[3]]))
    }
    pub fn read_f64(&mut self) -> Option<f64> {
        self.read_bytes_raw(8).map(|b| f64::from_le_bytes([b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7]]))
    }
    pub fn read_bool(&mut self) -> Option<bool> { self.read_u8().map(|v| v != 0) }

    pub fn read_bytes(&mut self) -> Option<Vec<u8>> {
        let len = self.read_u16()? as usize;
        self.read_bytes_raw(len).map(|s| s.to_vec())
    }

    pub fn read_str(&mut self) -> Option<String> {
        let bytes = self.read_bytes()?;
        String::from_utf8(bytes).ok()
    }

    /// LEB128 unsigned varint.
    pub fn read_varint(&mut self) -> Option<u64> {
        let mut result = 0u64;
        let mut shift  = 0u32;
        loop {
            let byte = self.read_u8()?;
            result |= ((byte & 0x7F) as u64) << shift;
            shift += 7;
            if byte & 0x80 == 0 { break; }
            if shift > 63 { return None; } // overflow guard
        }
        Some(result)
    }

    /// LEB128 signed varint (zigzag).
    pub fn read_varsint(&mut self) -> Option<i64> {
        let encoded = self.read_varint()?;
        let v = ((encoded >> 1) as i64) ^ (-((encoded & 1) as i64));
        Some(v)
    }

    pub fn read_vec3(&mut self) -> Option<[f32; 3]> {
        let x = self.read_f32()?;
        let y = self.read_f32()?;
        let z = self.read_f32()?;
        Some([x, y, z])
    }

    pub fn read_vec3_quantized(&mut self, range: f32) -> Option<[f32; 3]> {
        let xq = self.read_u16()? as u32;
        let yq = self.read_u16()? as u32;
        let zq = self.read_u16()? as u32;
        Some([
            dequantize_float(xq, -range, range, 16),
            dequantize_float(yq, -range, range, 16),
            dequantize_float(zq, -range, range, 16),
        ])
    }

    pub fn read_vec3_delta(&mut self, reference: [f32; 3]) -> Option<[f32; 3]> {
        let dx = self.read_i16()? as f32 / 1000.0;
        let dy = self.read_i16()? as f32 / 1000.0;
        let dz = self.read_i16()? as f32 / 1000.0;
        Some([reference[0] + dx, reference[1] + dy, reference[2] + dz])
    }

    pub fn read_quat(&mut self) -> Option<[f32; 4]> {
        let x = self.read_f32()?;
        let y = self.read_f32()?;
        let z = self.read_f32()?;
        let w = self.read_f32()?;
        Some([x, y, z, w])
    }

    pub fn read_quat_compressed(&mut self) -> Option<[f32; 4]> {
        let packed = self.read_u32()?;
        let largest = (packed >> 30) as usize;
        let a = dequantize_float((packed >> 20) & 0x3FF, -0.70711, 0.70711, 10);
        let b = dequantize_float((packed >> 10) & 0x3FF, -0.70711, 0.70711, 10);
        let c = dequantize_float( packed        & 0x3FF, -0.70711, 0.70711, 10);
        let d_sq = (1.0 - a*a - b*b - c*c).max(0.0);
        let d = d_sq.sqrt();
        let mut q = [0.0f32; 4];
        let small_arr = [a, b, c];
        let mut small = small_arr.iter();
        for i in 0..4 {
            if i == largest {
                q[i] = d;
            } else {
                q[i] = *small.next().unwrap_or(&0.0);
            }
        }
        Some(q)
    }
}

// ── Compressor ────────────────────────────────────────────────────────────────

/// Simple run-length encoder/decoder for snapshot data.
///
/// Encoding format per run:
///   If run length > 1: [0xFF, len(u8), byte]
///   Otherwise:         [byte]  (0xFF literal → [0xFF, 0x01, 0xFF])
pub struct Compressor;

impl Compressor {
    pub fn compress(data: &[u8]) -> Vec<u8> {
        if data.is_empty() { return Vec::new(); }
        let mut out = Vec::with_capacity(data.len());
        let mut i = 0;
        while i < data.len() {
            let byte = data[i];
            let mut run = 1usize;
            while i + run < data.len() && data[i + run] == byte && run < 255 {
                run += 1;
            }
            if run > 1 || byte == 0xFF {
                out.push(0xFF);
                out.push(run as u8);
                out.push(byte);
            } else {
                out.push(byte);
            }
            i += run;
        }
        out
    }

    pub fn decompress(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < data.len() {
            let byte = data[i];
            if byte == 0xFF {
                if i + 2 < data.len() {
                    let count = data[i + 1] as usize;
                    let value = data[i + 2];
                    for _ in 0..count { out.push(value); }
                    i += 3;
                } else {
                    i += 1; // malformed, skip
                }
            } else {
                out.push(byte);
                i += 1;
            }
        }
        out
    }

    /// Returns the compression ratio (compressed / original). Lower is better.
    pub fn ratio(original: &[u8], compressed: &[u8]) -> f32 {
        if original.is_empty() { return 1.0; }
        compressed.len() as f32 / original.len() as f32
    }
}

// ── PacketStats ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct PacketStats {
    pub sent:           u64,
    pub received:       u64,
    pub lost:           u64,
    pub bytes_sent:     u64,
    pub bytes_received: u64,
    pub fragments_sent: u64,
    pub fragments_recv: u64,
}

impl PacketStats {
    pub fn new() -> Self { PacketStats::default() }

    pub fn record_send(&mut self, bytes: usize) {
        self.sent += 1;
        self.bytes_sent += bytes as u64;
    }

    pub fn record_receive(&mut self, bytes: usize) {
        self.received += 1;
        self.bytes_received += bytes as u64;
    }

    pub fn record_loss(&mut self) { self.lost += 1; }

    pub fn loss_rate(&self) -> f32 {
        if self.sent == 0 { return 0.0; }
        self.lost as f32 / self.sent as f32
    }

    pub fn bytes_per_second_out(&self, elapsed_secs: f32) -> f32 {
        if elapsed_secs <= 0.0 { return 0.0; }
        self.bytes_sent as f32 / elapsed_secs
    }

    pub fn bytes_per_second_in(&self, elapsed_secs: f32) -> f32 {
        if elapsed_secs <= 0.0 { return 0.0; }
        self.bytes_received as f32 / elapsed_secs
    }

    pub fn reset(&mut self) { *self = PacketStats::default(); }
}

// ── Connection ────────────────────────────────────────────────────────────────

/// Combined per-peer connection state: header building + reliable channel + fragments.
pub struct Connection {
    pub reliable:   ReliableChannel,
    pub fragments:  FragmentSystem,
    pub stats:      PacketStats,
    local_seq:      u16,
}

impl Connection {
    pub fn new() -> Self {
        Connection {
            reliable:  ReliableChannel::new(),
            fragments: FragmentSystem::new(),
            stats:     PacketStats::new(),
            local_seq: 0,
        }
    }

    /// Build a complete unreliable packet (header + payload).
    pub fn build_unreliable(&mut self, payload: &[u8]) -> Vec<u8> {
        let seq = self.alloc_seq();
        let header = PacketHeader::new(
            PacketType::Unreliable,
            seq,
            self.reliable.remote_sequence,
            self.reliable.ack_bits,
        );
        let mut out = header.serialize();
        out.extend_from_slice(payload);
        self.stats.record_send(out.len());
        out
    }

    /// Build a reliable packet. Queues for retransmission.
    pub fn build_reliable(&mut self, payload: Vec<u8>) -> Vec<u8> {
        let seq = self.reliable.send(payload.clone());
        let header = PacketHeader::new(
            PacketType::Reliable,
            seq,
            self.reliable.remote_sequence,
            self.reliable.ack_bits,
        );
        let mut out = header.serialize();
        out.extend_from_slice(&payload);
        self.stats.record_send(out.len());
        out
    }

    /// Fragment a large payload and build fragment packets.
    pub fn build_fragments(&mut self, payload: Vec<u8>) -> Vec<Vec<u8>> {
        let frags = self.fragments.fragment(payload);
        frags.into_iter().map(|f| {
            let seq = self.alloc_seq();
            let header = PacketHeader::new(
                PacketType::Fragment,
                seq,
                self.reliable.remote_sequence,
                self.reliable.ack_bits,
            );
            let mut out = header.serialize();
            out.extend_from_slice(&f.serialize());
            self.stats.record_send(out.len());
            self.stats.fragments_sent += 1;
            out
        }).collect()
    }

    /// Process an incoming raw packet. Returns the payload if parseable.
    pub fn receive_packet(&mut self, data: &[u8]) -> Option<(PacketType, Vec<u8>)> {
        let header = PacketHeader::deserialize(data)?;
        self.reliable.process_ack(header.ack, header.ack_bits);
        self.reliable.update_ack(header.sequence);
        let payload = header.payload(data).to_vec();
        self.stats.record_receive(data.len());

        match header.packet_type {
            PacketType::Fragment => {
                self.stats.fragments_recv += 1;
                let frag = Fragment::deserialize(&payload)?;
                let assembled = self.fragments.reassemble(frag)?;
                Some((PacketType::Fragment, assembled))
            }
            PacketType::Reliable => {
                let delivered = self.reliable.receive(header.sequence, payload)?;
                Some((PacketType::Reliable, delivered))
            }
            other => Some((other, payload)),
        }
    }

    /// Build a standalone ack packet.
    pub fn build_ack(&mut self) -> Vec<u8> {
        let seq = self.alloc_seq();
        let header = PacketHeader::new(
            PacketType::Ack,
            seq,
            self.reliable.remote_sequence,
            self.reliable.ack_bits,
        );
        header.serialize()
    }

    pub fn get_retransmissions(&mut self) -> Vec<Vec<u8>> {
        self.reliable.get_retransmissions()
            .into_iter()
            .map(|(seq, payload)| {
                let header = PacketHeader::new(
                    PacketType::Reliable,
                    seq,
                    self.reliable.remote_sequence,
                    self.reliable.ack_bits,
                );
                let mut out = header.serialize();
                out.extend_from_slice(&payload);
                out
            })
            .collect()
    }

    fn alloc_seq(&mut self) -> u16 {
        let s = self.local_seq;
        self.local_seq = self.local_seq.wrapping_add(1);
        s
    }
}

impl Default for Connection {
    fn default() -> Self { Self::new() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let h = PacketHeader::new(PacketType::Reliable, 42, 40, 0b1010);
        let bytes = h.serialize();
        assert_eq!(bytes.len(), PacketHeader::WIRE_SIZE);
        let back = PacketHeader::deserialize(&bytes).unwrap();
        assert_eq!(back, h);
    }

    #[test]
    fn header_wrong_magic() {
        let mut bytes = PacketHeader::new(PacketType::Reliable, 1, 0, 0).serialize();
        bytes[0] = 0x00;
        assert!(PacketHeader::deserialize(&bytes).is_none());
    }

    #[test]
    fn quantize_roundtrip() {
        let v = 0.333f32;
        let q = quantize_float(v, 0.0, 1.0, 16);
        let back = dequantize_float(q, 0.0, 1.0, 16);
        assert!((back - v).abs() < 0.001);
    }

    #[test]
    fn compressor_roundtrip() {
        let data: Vec<u8> = (0..200).map(|i| (i % 8) as u8).collect();
        let compressed = Compressor::compress(&data);
        let decompressed = Compressor::decompress(&compressed);
        assert_eq!(data, decompressed);
    }

    #[test]
    fn compressor_handles_0xff() {
        let data = vec![0xFF, 0xFF, 0xAA, 0xAA, 0xAA];
        let c = Compressor::compress(&data);
        let d = Compressor::decompress(&c);
        assert_eq!(data, d);
    }

    #[test]
    fn fragment_reassembly() {
        let mut fs = FragmentSystem::new();
        let large = vec![0xABu8; MAX_PACKET_SIZE * 3];
        let frags = fs.fragment(large.clone());
        assert!(frags.len() > 1);

        let mut result = None;
        for f in frags {
            result = fs.reassemble(f);
        }
        assert_eq!(result.unwrap(), large);
    }

    #[test]
    fn leb128_varint_roundtrip() {
        let values: &[u64] = &[0, 1, 127, 128, 16384, u32::MAX as u64, u64::MAX / 2];
        for &v in values {
            let mut ser = Serializer::new();
            ser.write_varint(v);
            let bytes = ser.finish();
            let mut de = Deserializer::new(&bytes);
            assert_eq!(de.read_varint().unwrap(), v, "varint mismatch for {}", v);
        }
    }

    #[test]
    fn vec3_roundtrip() {
        let pos = [1.5f32, -3.25, 100.0];
        let mut ser = Serializer::new();
        ser.write_vec3(pos);
        let bytes = ser.finish();
        let mut de = Deserializer::new(&bytes);
        let back = de.read_vec3().unwrap();
        for i in 0..3 { assert!((back[i] - pos[i]).abs() < 1e-5); }
    }

    #[test]
    fn vec3_delta_roundtrip() {
        let reference = [10.0f32, 20.0, 30.0];
        let current   = [10.5f32, 19.8, 30.001];
        let mut ser = Serializer::new();
        ser.write_vec3_delta(current, reference);
        let bytes = ser.finish();
        let mut de = Deserializer::new(&bytes);
        let back = de.read_vec3_delta(reference).unwrap();
        for i in 0..3 { assert!((back[i] - current[i]).abs() < 0.002, "delta[{}]", i); }
    }

    #[test]
    fn connection_unreliable_packet() {
        let mut conn = Connection::new();
        let payload = b"hello world".to_vec();
        let packet = conn.build_unreliable(&payload);
        assert!(packet.len() > PacketHeader::WIRE_SIZE);
        let result = conn.receive_packet(&packet);
        assert!(result.is_some());
        let (ptype, data) = result.unwrap();
        assert_eq!(ptype, PacketType::Unreliable);
        assert_eq!(data, payload);
    }

    #[test]
    fn sequence_ordering() {
        assert!(sequence_greater_than(5, 3));
        assert!(!sequence_greater_than(3, 5));
        // wrap-around
        assert!(sequence_greater_than(1, 65530));
    }
}
