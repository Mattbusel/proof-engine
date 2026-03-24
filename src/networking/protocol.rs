//! Multiplayer networking protocol: packet encoding, decoding, auth tokens,
//! bandwidth tracking, and packet filtering.
//!
//! Custom binary wire format (no serde dependency):
//! - Variable-length integers for sequence numbers
//! - Bit-packed flags in the header
//! - Delta encoding hints for position payloads
//! - 16-byte header, variable payload

// ─── Constants ───────────────────────────────────────────────────────────────

/// Wire protocol version negotiated on connect.
pub const PROTOCOL_VERSION: u8 = 1;

/// Magic bytes at the start of every packet (PEMP = Proof-Engine Multiplayer Protocol).
pub const MAGIC: [u8; 4] = [0x50, 0x45, 0x4D, 0x50];

/// Maximum allowed payload length (64 KiB).
pub const MAX_PAYLOAD_LEN: usize = 65535;

/// Maximum packets stored in replay/filter history.
pub const FILTER_HISTORY_LEN: usize = 1024;

// ─── PacketKind ──────────────────────────────────────────────────────────────

/// Discriminant for every packet type on the wire.
///
/// Encoded as a `u16` in the packet header so `Custom(u16)` can carry
/// application-defined packet kinds without colliding with the well-known range
/// (0x0000–0x001D).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PacketKind {
    Connect,           // 0x00
    Disconnect,        // 0x01
    Heartbeat,         // 0x02
    StateUpdate,       // 0x03
    InputEvent,        // 0x04
    ChatMessage,       // 0x05
    SpawnEntity,       // 0x06
    DespawnEntity,     // 0x07
    UpdateTransform,   // 0x08
    AnimationState,    // 0x09
    SoundEvent,        // 0x0A
    ParticleEvent,     // 0x0B
    ForceFieldSync,    // 0x0C
    CameraUpdate,      // 0x0D
    ScriptCall,        // 0x0E
    ScriptResult,      // 0x0F
    Ack,               // 0x10
    Nack,              // 0x11
    Ping,              // 0x12
    Pong,              // 0x13
    LobbyJoin,         // 0x14
    LobbyLeave,        // 0x15
    GameStart,         // 0x16
    GameEnd,           // 0x17
    VoteKick,          // 0x18
    VoteBan,           // 0x19
    FileRequest,       // 0x1A
    FileChunk,         // 0x1B
    Error,             // 0x1C
    Custom(u16),       // 0x8000–0xFFFF (high-bit set)
}

impl PacketKind {
    /// Encode to a `u16` discriminant.
    pub fn to_u16(self) -> u16 {
        match self {
            PacketKind::Connect        => 0x00,
            PacketKind::Disconnect     => 0x01,
            PacketKind::Heartbeat      => 0x02,
            PacketKind::StateUpdate    => 0x03,
            PacketKind::InputEvent     => 0x04,
            PacketKind::ChatMessage    => 0x05,
            PacketKind::SpawnEntity    => 0x06,
            PacketKind::DespawnEntity  => 0x07,
            PacketKind::UpdateTransform => 0x08,
            PacketKind::AnimationState => 0x09,
            PacketKind::SoundEvent     => 0x0A,
            PacketKind::ParticleEvent  => 0x0B,
            PacketKind::ForceFieldSync => 0x0C,
            PacketKind::CameraUpdate   => 0x0D,
            PacketKind::ScriptCall     => 0x0E,
            PacketKind::ScriptResult   => 0x0F,
            PacketKind::Ack            => 0x10,
            PacketKind::Nack           => 0x11,
            PacketKind::Ping           => 0x12,
            PacketKind::Pong           => 0x13,
            PacketKind::LobbyJoin      => 0x14,
            PacketKind::LobbyLeave     => 0x15,
            PacketKind::GameStart      => 0x16,
            PacketKind::GameEnd        => 0x17,
            PacketKind::VoteKick       => 0x18,
            PacketKind::VoteBan        => 0x19,
            PacketKind::FileRequest    => 0x1A,
            PacketKind::FileChunk      => 0x1B,
            PacketKind::Error          => 0x1C,
            PacketKind::Custom(v)      => 0x8000 | v,
        }
    }

    /// Decode from a `u16` discriminant.
    pub fn from_u16(v: u16) -> Result<Self, ProtocolError> {
        if v & 0x8000 != 0 {
            return Ok(PacketKind::Custom(v & 0x7FFF));
        }
        match v {
            0x00 => Ok(PacketKind::Connect),
            0x01 => Ok(PacketKind::Disconnect),
            0x02 => Ok(PacketKind::Heartbeat),
            0x03 => Ok(PacketKind::StateUpdate),
            0x04 => Ok(PacketKind::InputEvent),
            0x05 => Ok(PacketKind::ChatMessage),
            0x06 => Ok(PacketKind::SpawnEntity),
            0x07 => Ok(PacketKind::DespawnEntity),
            0x08 => Ok(PacketKind::UpdateTransform),
            0x09 => Ok(PacketKind::AnimationState),
            0x0A => Ok(PacketKind::SoundEvent),
            0x0B => Ok(PacketKind::ParticleEvent),
            0x0C => Ok(PacketKind::ForceFieldSync),
            0x0D => Ok(PacketKind::CameraUpdate),
            0x0E => Ok(PacketKind::ScriptCall),
            0x0F => Ok(PacketKind::ScriptResult),
            0x10 => Ok(PacketKind::Ack),
            0x11 => Ok(PacketKind::Nack),
            0x12 => Ok(PacketKind::Ping),
            0x13 => Ok(PacketKind::Pong),
            0x14 => Ok(PacketKind::LobbyJoin),
            0x15 => Ok(PacketKind::LobbyLeave),
            0x16 => Ok(PacketKind::GameStart),
            0x17 => Ok(PacketKind::GameEnd),
            0x18 => Ok(PacketKind::VoteKick),
            0x19 => Ok(PacketKind::VoteBan),
            0x1A => Ok(PacketKind::FileRequest),
            0x1B => Ok(PacketKind::FileChunk),
            0x1C => Ok(PacketKind::Error),
            other => Err(ProtocolError::UnknownPacketKind(other)),
        }
    }
}

// ─── CompressionHint ─────────────────────────────────────────────────────────

/// Indicates how the payload bytes are compressed.
/// The receiver must use matching decompression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionHint {
    None,
    Zlib,
    Lz4,
}

impl CompressionHint {
    pub fn to_u8(self) -> u8 {
        match self {
            CompressionHint::None => 0,
            CompressionHint::Zlib => 1,
            CompressionHint::Lz4  => 2,
        }
    }

    pub fn from_u8(v: u8) -> Result<Self, ProtocolError> {
        match v {
            0 => Ok(CompressionHint::None),
            1 => Ok(CompressionHint::Zlib),
            2 => Ok(CompressionHint::Lz4),
            _ => Err(ProtocolError::InvalidCompression(v)),
        }
    }
}

// ─── PacketHeader ─────────────────────────────────────────────────────────────

/// Fixed-size header that precedes every packet on the wire.
///
/// Wire layout (20 bytes):
/// ```text
/// [0..4]   magic       PEMP
/// [4]      version     u8
/// [5]      flags       u8  (bits: 0-1 compression, 2 reliable, 3 ordered, 4 fragmented, 5-7 reserved)
/// [6..8]   kind        u16 big-endian
/// [8..12]  sequence    u32 big-endian
/// [12..16] ack         u32 big-endian
/// [16..20] ack_bits    u32 big-endian
/// [20..22] payload_len u16 big-endian
/// ```
/// Total header: 22 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketHeader {
    pub version:     u8,
    pub flags:       u8,
    pub kind:        PacketKind,
    pub sequence:    u32,
    pub ack:         u32,
    pub ack_bits:    u32,
    pub payload_len: u16,
}

impl PacketHeader {
    pub const SIZE: usize = 22;

    /// Flag bit: payload is reliable (must be acked).
    pub const FLAG_RELIABLE:   u8 = 0b0000_0100;
    /// Flag bit: channel is ordered.
    pub const FLAG_ORDERED:    u8 = 0b0000_1000;
    /// Flag bit: packet is a fragment of a larger message.
    pub const FLAG_FRAGMENTED: u8 = 0b0001_0000;

    /// Extract compression hint from flags bits 0-1.
    pub fn compression(&self) -> Result<CompressionHint, ProtocolError> {
        CompressionHint::from_u8(self.flags & 0x03)
    }

    pub fn is_reliable(&self) -> bool {
        self.flags & Self::FLAG_RELIABLE != 0
    }

    pub fn is_ordered(&self) -> bool {
        self.flags & Self::FLAG_ORDERED != 0
    }

    pub fn is_fragmented(&self) -> bool {
        self.flags & Self::FLAG_FRAGMENTED != 0
    }
}

// ─── Packet ───────────────────────────────────────────────────────────────────

/// A fully-parsed network packet ready for dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub kind:     PacketKind,
    pub sequence: u32,
    pub ack:      u32,
    pub ack_bits: u32,
    pub payload:  Vec<u8>,
    /// Extra header flags preserved for routing decisions.
    pub flags:    u8,
}

impl Packet {
    pub fn new(kind: PacketKind, sequence: u32, ack: u32, ack_bits: u32, payload: Vec<u8>) -> Self {
        Self { kind, sequence, ack, ack_bits, payload, flags: 0 }
    }

    pub fn with_flags(mut self, flags: u8) -> Self {
        self.flags = flags;
        self
    }

    pub fn is_reliable(&self) -> bool {
        self.flags & PacketHeader::FLAG_RELIABLE != 0
    }

    /// Heartbeat shorthand.
    pub fn heartbeat(sequence: u32, ack: u32, ack_bits: u32) -> Self {
        Self::new(PacketKind::Heartbeat, sequence, ack, ack_bits, Vec::new())
    }

    /// Ping with 8-byte timestamp payload.
    pub fn ping(sequence: u32, ack: u32, ack_bits: u32, timestamp_us: u64) -> Self {
        let mut payload = Vec::with_capacity(8);
        payload.extend_from_slice(&timestamp_us.to_be_bytes());
        Self::new(PacketKind::Ping, sequence, ack, ack_bits, payload)
    }

    /// Pong mirrors the ping timestamp plus local receive timestamp.
    pub fn pong(sequence: u32, ack: u32, ack_bits: u32, ping_ts: u64, recv_ts: u64) -> Self {
        let mut payload = Vec::with_capacity(16);
        payload.extend_from_slice(&ping_ts.to_be_bytes());
        payload.extend_from_slice(&recv_ts.to_be_bytes());
        Self::new(PacketKind::Pong, sequence, ack, ack_bits, payload)
    }
}

// ─── ProtocolError ────────────────────────────────────────────────────────────

/// All errors that can arise from encoding or decoding packets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// Input buffer is too short for a complete header.
    BufferTooShort { needed: usize, got: usize },
    /// Magic bytes do not match.
    BadMagic([u8; 4]),
    /// Protocol version mismatch.
    VersionMismatch { expected: u8, got: u8 },
    /// Payload length in header exceeds remaining buffer.
    PayloadTruncated { declared: usize, available: usize },
    /// Payload exceeds the maximum allowed size.
    PayloadTooLarge(usize),
    /// Unknown packet kind discriminant.
    UnknownPacketKind(u16),
    /// Unknown compression tag.
    InvalidCompression(u8),
    /// Packet was identified as a replay attack.
    ReplayDetected { sequence: u32 },
    /// Generic encode error with a message.
    EncodeError(String),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BufferTooShort { needed, got } =>
                write!(f, "buffer too short: need {needed} bytes, got {got}"),
            Self::BadMagic(m) =>
                write!(f, "bad magic bytes: {:02x}{:02x}{:02x}{:02x}", m[0], m[1], m[2], m[3]),
            Self::VersionMismatch { expected, got } =>
                write!(f, "version mismatch: expected {expected}, got {got}"),
            Self::PayloadTruncated { declared, available } =>
                write!(f, "payload truncated: declared {declared} bytes, only {available} available"),
            Self::PayloadTooLarge(n) =>
                write!(f, "payload too large: {n} bytes"),
            Self::UnknownPacketKind(k) =>
                write!(f, "unknown packet kind: 0x{k:04x}"),
            Self::InvalidCompression(c) =>
                write!(f, "invalid compression tag: {c}"),
            Self::ReplayDetected { sequence } =>
                write!(f, "replay attack detected: sequence {sequence}"),
            Self::EncodeError(s) =>
                write!(f, "encode error: {s}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

// ─── PacketEncoder ────────────────────────────────────────────────────────────

/// Serializes `Packet` values to a contiguous byte buffer.
///
/// The encoder does NOT own any state between calls; call `encode` for each
/// packet and write the returned `Vec<u8>` to your socket.
pub struct PacketEncoder {
    /// Compression hint written into the header flags.
    pub compression: CompressionHint,
    /// Whether the reliable flag should be set on encoded packets.
    pub reliable: bool,
    /// Whether the ordered flag should be set on encoded packets.
    pub ordered: bool,
}

impl Default for PacketEncoder {
    fn default() -> Self {
        Self {
            compression: CompressionHint::None,
            reliable: false,
            ordered: false,
        }
    }
}

impl PacketEncoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build flags byte from encoder settings and any packet-level flags.
    fn build_flags(&self, extra: u8) -> u8 {
        let mut f = self.compression.to_u8(); // bits 0-1
        if self.reliable { f |= PacketHeader::FLAG_RELIABLE; }
        if self.ordered  { f |= PacketHeader::FLAG_ORDERED; }
        f | (extra & PacketHeader::FLAG_FRAGMENTED)
    }

    /// Encode a `Packet` to bytes.  Returns `Err` if payload exceeds limit.
    pub fn encode(&self, packet: &Packet) -> Result<Vec<u8>, ProtocolError> {
        let payload_len = packet.payload.len();
        if payload_len > MAX_PAYLOAD_LEN {
            return Err(ProtocolError::PayloadTooLarge(payload_len));
        }
        let total = PacketHeader::SIZE + payload_len;
        let mut buf = Vec::with_capacity(total);

        // Magic
        buf.extend_from_slice(&MAGIC);
        // Version
        buf.push(PROTOCOL_VERSION);
        // Flags
        buf.push(self.build_flags(packet.flags));
        // Kind (u16 BE)
        buf.extend_from_slice(&packet.kind.to_u16().to_be_bytes());
        // Sequence (u32 BE)
        buf.extend_from_slice(&packet.sequence.to_be_bytes());
        // Ack (u32 BE)
        buf.extend_from_slice(&packet.ack.to_be_bytes());
        // Ack-bits (u32 BE)
        buf.extend_from_slice(&packet.ack_bits.to_be_bytes());
        // Payload length (u16 BE)
        buf.extend_from_slice(&(payload_len as u16).to_be_bytes());
        // Payload
        buf.extend_from_slice(&packet.payload);

        debug_assert_eq!(buf.len(), total);
        Ok(buf)
    }

    /// Encode a sequence of packets back-to-back (UDP batch / TCP stream).
    pub fn encode_batch(&self, packets: &[Packet]) -> Result<Vec<u8>, ProtocolError> {
        let mut out = Vec::new();
        for p in packets {
            out.extend(self.encode(p)?);
        }
        Ok(out)
    }

    /// Variable-length encode a u64 (LEB128).
    pub fn encode_varint(value: u64, out: &mut Vec<u8>) {
        let mut v = value;
        loop {
            let byte = (v & 0x7F) as u8;
            v >>= 7;
            if v == 0 {
                out.push(byte);
                break;
            } else {
                out.push(byte | 0x80);
            }
        }
    }

    /// Delta-encode an f32 position value as a 16-bit fixed-point delta.
    /// Returns the quantised delta in centimetres (±327.67 m range at 1 cm precision).
    pub fn encode_position_delta(from: f32, to: f32) -> i16 {
        let delta_cm = ((to - from) * 100.0).round();
        delta_cm.clamp(i16::MIN as f32, i16::MAX as f32) as i16
    }

    /// Bit-pack up to 8 booleans into a single byte.
    pub fn pack_bools(flags: &[bool]) -> u8 {
        let mut byte = 0u8;
        for (i, &b) in flags.iter().take(8).enumerate() {
            if b { byte |= 1 << i; }
        }
        byte
    }
}

// ─── PacketDecoder ────────────────────────────────────────────────────────────

/// Deserializes packets from a byte buffer with strict bounds checking.
pub struct PacketDecoder {
    /// When `true` reject packets whose version differs from `PROTOCOL_VERSION`.
    pub strict_version: bool,
}

impl Default for PacketDecoder {
    fn default() -> Self {
        Self { strict_version: true }
    }
}

impl PacketDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode exactly one packet starting at the beginning of `buf`.
    /// Returns `(packet, bytes_consumed)` on success.
    pub fn decode(&self, buf: &[u8]) -> Result<(Packet, usize), ProtocolError> {
        // Minimum header check
        if buf.len() < PacketHeader::SIZE {
            return Err(ProtocolError::BufferTooShort {
                needed: PacketHeader::SIZE,
                got:    buf.len(),
            });
        }

        // Magic
        let magic: [u8; 4] = buf[0..4].try_into().unwrap();
        if magic != MAGIC {
            return Err(ProtocolError::BadMagic(magic));
        }

        let version     = buf[4];
        let flags       = buf[5];
        let kind_raw    = u16::from_be_bytes([buf[6], buf[7]]);
        let sequence    = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let ack         = u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]);
        let ack_bits    = u32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]);
        let payload_len = u16::from_be_bytes([buf[20], buf[21]]) as usize;

        if self.strict_version && version != PROTOCOL_VERSION {
            return Err(ProtocolError::VersionMismatch {
                expected: PROTOCOL_VERSION,
                got:      version,
            });
        }

        if payload_len > MAX_PAYLOAD_LEN {
            return Err(ProtocolError::PayloadTooLarge(payload_len));
        }

        let total = PacketHeader::SIZE + payload_len;
        if buf.len() < total {
            return Err(ProtocolError::PayloadTruncated {
                declared:  payload_len,
                available: buf.len().saturating_sub(PacketHeader::SIZE),
            });
        }

        let kind = PacketKind::from_u16(kind_raw)?;
        let payload = buf[PacketHeader::SIZE..total].to_vec();

        let packet = Packet {
            kind,
            sequence,
            ack,
            ack_bits,
            payload,
            flags,
        };

        Ok((packet, total))
    }

    /// Decode all packets packed end-to-end in `buf`.
    pub fn decode_all(&self, buf: &[u8]) -> Result<Vec<Packet>, ProtocolError> {
        let mut packets = Vec::new();
        let mut offset  = 0usize;
        while offset < buf.len() {
            let (pkt, consumed) = self.decode(&buf[offset..])?;
            packets.push(pkt);
            offset += consumed;
        }
        Ok(packets)
    }

    /// Decode a variable-length integer (LEB128) from `buf` at `offset`.
    /// Returns `(value, new_offset)`.
    pub fn decode_varint(buf: &[u8], offset: usize) -> Result<(u64, usize), ProtocolError> {
        let mut result = 0u64;
        let mut shift  = 0u32;
        let mut pos    = offset;
        loop {
            if pos >= buf.len() {
                return Err(ProtocolError::BufferTooShort {
                    needed: pos + 1,
                    got:    buf.len(),
                });
            }
            let byte = buf[pos] as u64;
            pos += 1;
            result |= (byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift >= 64 {
                return Err(ProtocolError::EncodeError("varint overflow".into()));
            }
        }
        Ok((result, pos))
    }

    /// Unpack up to 8 booleans from a flags byte.
    pub fn unpack_bools(byte: u8, count: usize) -> [bool; 8] {
        let mut out = [false; 8];
        for i in 0..count.min(8) {
            out[i] = (byte >> i) & 1 != 0;
        }
        out
    }

    /// Decode a 16-bit position delta back to an f32 offset.
    pub fn decode_position_delta(delta: i16) -> f32 {
        delta as f32 / 100.0
    }
}

// ─── ConnectionToken ─────────────────────────────────────────────────────────

/// Opaque token issued by the auth server, embedded in the Connect packet.
///
/// The server verifies `client_id` matches `server_key` HMAC and that
/// `expires_at` (Unix seconds) has not elapsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionToken {
    pub client_id:  u64,
    pub server_key: [u8; 16],
    pub expires_at: u64,
}

impl ConnectionToken {
    pub const SIZE: usize = 32; // 8 + 16 + 8

    pub fn new(client_id: u64, server_key: [u8; 16], expires_at: u64) -> Self {
        Self { client_id, server_key, expires_at }
    }

    /// Serialize to exactly 32 bytes.
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut out = [0u8; Self::SIZE];
        out[0..8].copy_from_slice(&self.client_id.to_be_bytes());
        out[8..24].copy_from_slice(&self.server_key);
        out[24..32].copy_from_slice(&self.expires_at.to_be_bytes());
        out
    }

    /// Deserialize from 32 bytes.
    pub fn from_bytes(b: &[u8]) -> Result<Self, ProtocolError> {
        if b.len() < Self::SIZE {
            return Err(ProtocolError::BufferTooShort {
                needed: Self::SIZE,
                got:    b.len(),
            });
        }
        let client_id = u64::from_be_bytes(b[0..8].try_into().unwrap());
        let server_key: [u8; 16] = b[8..24].try_into().unwrap();
        let expires_at = u64::from_be_bytes(b[24..32].try_into().unwrap());
        Ok(Self { client_id, server_key, expires_at })
    }

    /// Returns `true` when the token has not expired relative to `now_secs`.
    pub fn is_valid(&self, now_secs: u64) -> bool {
        now_secs < self.expires_at
    }

    /// Produce a simple 4-byte checksum used to verify the key field.
    /// Real deployments should use HMAC-SHA256; this is a stand-in.
    pub fn checksum(&self) -> u32 {
        let mut h = 0x811c9dc5u32;
        for b in &self.server_key {
            h ^= *b as u32;
            h = h.wrapping_mul(0x01000193);
        }
        h ^= self.client_id as u32;
        h = h.wrapping_mul(0x01000193);
        h
    }
}

// ─── PacketFilter ─────────────────────────────────────────────────────────────

/// Tracks recently seen sequence numbers to detect replay attacks and
/// malformed packets before they reach higher-level code.
pub struct PacketFilter {
    /// Circular buffer of recently seen sequence numbers per peer.
    seen: std::collections::HashMap<u64, SeenWindow>,
    /// Maximum packets that can arrive with the same sequence before rejection.
    max_duplicates: u32,
}

/// Sliding-window replay detection for a single peer.
struct SeenWindow {
    /// Highest sequence seen so far.
    highest: u32,
    /// Bitset: bit i set means (highest - i) has been seen.
    bits: u64,
}

impl SeenWindow {
    fn new() -> Self {
        Self { highest: 0, bits: 0 }
    }

    /// Returns `true` if the sequence is new (not a duplicate/replay).
    fn check_and_insert(&mut self, seq: u32) -> bool {
        let diff = self.highest.wrapping_sub(seq);
        if seq == self.highest && self.bits & 1 != 0 {
            // exact duplicate of highest
            return false;
        }
        if diff < 64 && diff > 0 {
            // Older packet within window
            let mask = 1u64 << diff;
            if self.bits & mask != 0 {
                return false; // already seen
            }
            self.bits |= mask;
            return true;
        }
        if seq.wrapping_sub(self.highest) < 0x8000_0000 {
            // New highest
            let advance = seq.wrapping_sub(self.highest);
            if advance >= 64 {
                self.bits = 1;
            } else {
                self.bits = (self.bits << advance) | 1;
            }
            self.highest = seq;
            return true;
        }
        // Packet is too old (more than 64 below highest) — reject
        false
    }
}

impl PacketFilter {
    pub fn new() -> Self {
        Self {
            seen: std::collections::HashMap::new(),
            max_duplicates: 0,
        }
    }

    /// Register a peer and allow tracking for it.
    pub fn register_peer(&mut self, peer_id: u64) {
        self.seen.entry(peer_id).or_insert_with(SeenWindow::new);
    }

    /// Remove tracking for a disconnected peer.
    pub fn remove_peer(&mut self, peer_id: u64) {
        self.seen.remove(&peer_id);
    }

    /// Returns `Ok(())` if `packet` should be accepted, `Err` otherwise.
    pub fn check(&mut self, peer_id: u64, packet: &Packet) -> Result<(), ProtocolError> {
        // Basic sanity: payload size within declared limits
        if packet.payload.len() > MAX_PAYLOAD_LEN {
            return Err(ProtocolError::PayloadTooLarge(packet.payload.len()));
        }

        // Replay detection
        let window = self.seen.entry(peer_id).or_insert_with(SeenWindow::new);
        if !window.check_and_insert(packet.sequence) {
            return Err(ProtocolError::ReplayDetected { sequence: packet.sequence });
        }

        Ok(())
    }

    /// Resets all state (e.g. on reconnect).
    pub fn reset(&mut self) {
        self.seen.clear();
    }
}

impl Default for PacketFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ─── BandwidthTracker ─────────────────────────────────────────────────────────

/// Rolling-window bandwidth meter.
///
/// Call `record_send` / `record_recv` with byte counts and the current
/// millisecond timestamp.  Query `bytes_per_sec_up` / `bytes_per_sec_down`
/// to get the rolling rate.
pub struct BandwidthTracker {
    /// Length of the rolling window in milliseconds.
    pub window_ms: u64,
    send_buckets: std::collections::VecDeque<(u64, usize)>,
    recv_buckets: std::collections::VecDeque<(u64, usize)>,
    total_sent: u64,
    total_recv: u64,
}

impl BandwidthTracker {
    pub fn new(window_ms: u64) -> Self {
        Self {
            window_ms,
            send_buckets: std::collections::VecDeque::new(),
            recv_buckets: std::collections::VecDeque::new(),
            total_sent: 0,
            total_recv: 0,
        }
    }

    /// Default 1-second rolling window.
    pub fn default_window() -> Self {
        Self::new(1000)
    }

    /// Record `bytes` sent at time `now_ms`.
    pub fn record_send(&mut self, bytes: usize, now_ms: u64) {
        self.total_sent += bytes as u64;
        self.send_buckets.push_back((now_ms, bytes));
        self.evict_old(now_ms);
    }

    /// Record `bytes` received at time `now_ms`.
    pub fn record_recv(&mut self, bytes: usize, now_ms: u64) {
        self.total_recv += bytes as u64;
        self.recv_buckets.push_back((now_ms, bytes));
        self.evict_old(now_ms);
    }

    fn evict_old(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        while let Some(&(ts, _)) = self.send_buckets.front() {
            if ts < cutoff { self.send_buckets.pop_front(); } else { break; }
        }
        while let Some(&(ts, _)) = self.recv_buckets.front() {
            if ts < cutoff { self.recv_buckets.pop_front(); } else { break; }
        }
    }

    /// Bytes per second upload over the rolling window.
    pub fn bytes_per_sec_up(&self, now_ms: u64) -> f64 {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        let sum: usize = self.send_buckets.iter()
            .filter(|&&(ts, _)| ts >= cutoff)
            .map(|&(_, b)| b)
            .sum();
        (sum as f64) / (self.window_ms as f64 / 1000.0)
    }

    /// Bytes per second download over the rolling window.
    pub fn bytes_per_sec_down(&self, now_ms: u64) -> f64 {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        let sum: usize = self.recv_buckets.iter()
            .filter(|&&(ts, _)| ts >= cutoff)
            .map(|&(_, b)| b)
            .sum();
        (sum as f64) / (self.window_ms as f64 / 1000.0)
    }

    pub fn total_bytes_sent(&self) -> u64 { self.total_sent }
    pub fn total_bytes_recv(&self) -> u64 { self.total_recv }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_packet(kind: PacketKind, seq: u32, payload: Vec<u8>) -> Packet {
        Packet::new(kind, seq, 0, 0, payload)
    }

    // ── PacketKind round-trip ─────────────────────────────────────────────────

    #[test]
    fn test_packet_kind_roundtrip_well_known() {
        let kinds = [
            PacketKind::Connect, PacketKind::Disconnect, PacketKind::Heartbeat,
            PacketKind::StateUpdate, PacketKind::InputEvent, PacketKind::ChatMessage,
            PacketKind::SpawnEntity, PacketKind::DespawnEntity, PacketKind::UpdateTransform,
            PacketKind::AnimationState, PacketKind::SoundEvent, PacketKind::ParticleEvent,
            PacketKind::ForceFieldSync, PacketKind::CameraUpdate, PacketKind::ScriptCall,
            PacketKind::ScriptResult, PacketKind::Ack, PacketKind::Nack,
            PacketKind::Ping, PacketKind::Pong, PacketKind::LobbyJoin, PacketKind::LobbyLeave,
            PacketKind::GameStart, PacketKind::GameEnd, PacketKind::VoteKick, PacketKind::VoteBan,
            PacketKind::FileRequest, PacketKind::FileChunk, PacketKind::Error,
        ];
        for k in kinds {
            let v = k.to_u16();
            assert_eq!(PacketKind::from_u16(v).unwrap(), k, "round-trip for {k:?}");
        }
    }

    #[test]
    fn test_packet_kind_custom_roundtrip() {
        let k = PacketKind::Custom(42);
        assert_eq!(PacketKind::from_u16(k.to_u16()).unwrap(), k);
    }

    #[test]
    fn test_packet_kind_unknown_returns_err() {
        assert!(PacketKind::from_u16(0x1D).is_err());
    }

    // ── Encoder / Decoder round-trip ─────────────────────────────────────────

    #[test]
    fn test_encode_decode_roundtrip() {
        let enc = PacketEncoder::new();
        let dec = PacketDecoder::new();
        let pkt = make_packet(PacketKind::ChatMessage, 7, b"hello world".to_vec());
        let bytes = enc.encode(&pkt).unwrap();
        let (decoded, consumed) = dec.decode(&bytes).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.kind, pkt.kind);
        assert_eq!(decoded.sequence, pkt.sequence);
        assert_eq!(decoded.payload, pkt.payload);
    }

    #[test]
    fn test_decode_too_short_returns_err() {
        let dec = PacketDecoder::new();
        let short = [0u8; 5];
        assert!(matches!(dec.decode(&short), Err(ProtocolError::BufferTooShort { .. })));
    }

    #[test]
    fn test_decode_bad_magic() {
        let dec = PacketDecoder::new();
        let mut bytes = vec![0u8; PacketHeader::SIZE];
        // Don't write correct magic
        assert!(matches!(dec.decode(&bytes), Err(ProtocolError::BadMagic(_))));
        // Fix magic but bad version
        bytes[0..4].copy_from_slice(&MAGIC);
        bytes[4] = 99; // wrong version
        assert!(matches!(dec.decode(&bytes), Err(ProtocolError::VersionMismatch { .. })));
    }

    #[test]
    fn test_encode_batch_and_decode_all() {
        let enc = PacketEncoder::new();
        let dec = PacketDecoder::new();
        let pkts = vec![
            make_packet(PacketKind::Ping, 1, vec![1, 2, 3, 4, 5, 6, 7, 8]),
            make_packet(PacketKind::Pong, 2, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
            make_packet(PacketKind::Heartbeat, 3, vec![]),
        ];
        let bytes = enc.encode_batch(&pkts).unwrap();
        let decoded = dec.decode_all(&bytes).unwrap();
        assert_eq!(decoded.len(), pkts.len());
        for (a, b) in pkts.iter().zip(decoded.iter()) {
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.sequence, b.sequence);
            assert_eq!(a.payload, b.payload);
        }
    }

    // ── Varint ────────────────────────────────────────────────────────────────

    #[test]
    fn test_varint_roundtrip() {
        let values: &[u64] = &[0, 1, 127, 128, 255, 300, 16383, 16384, u32::MAX as u64, u64::MAX / 2];
        for &v in values {
            let mut buf = Vec::new();
            PacketEncoder::encode_varint(v, &mut buf);
            let (decoded, _) = PacketDecoder::decode_varint(&buf, 0).unwrap();
            assert_eq!(decoded, v, "varint roundtrip for {v}");
        }
    }

    // ── ConnectionToken ───────────────────────────────────────────────────────

    #[test]
    fn test_connection_token_roundtrip() {
        let tok = ConnectionToken::new(0xDEADBEEF_CAFEBABE, [0xAB; 16], 9999999999);
        let bytes = tok.to_bytes();
        let tok2 = ConnectionToken::from_bytes(&bytes).unwrap();
        assert_eq!(tok, tok2);
    }

    #[test]
    fn test_connection_token_validity() {
        let tok = ConnectionToken::new(1, [0u8; 16], 1000);
        assert!(tok.is_valid(999));
        assert!(!tok.is_valid(1000));
        assert!(!tok.is_valid(1001));
    }

    // ── PacketFilter ──────────────────────────────────────────────────────────

    #[test]
    fn test_packet_filter_accepts_new_sequences() {
        let mut filter = PacketFilter::new();
        let peer = 1u64;
        for seq in 0u32..10 {
            let pkt = make_packet(PacketKind::StateUpdate, seq, vec![]);
            assert!(filter.check(peer, &pkt).is_ok(), "seq {seq} should be accepted");
        }
    }

    #[test]
    fn test_packet_filter_rejects_replay() {
        let mut filter = PacketFilter::new();
        let peer = 42u64;
        let pkt = make_packet(PacketKind::StateUpdate, 5, vec![]);
        assert!(filter.check(peer, &pkt).is_ok());
        // Same sequence again — replay
        assert!(matches!(
            filter.check(peer, &pkt),
            Err(ProtocolError::ReplayDetected { sequence: 5 })
        ));
    }

    // ── BandwidthTracker ──────────────────────────────────────────────────────

    #[test]
    fn test_bandwidth_tracker_basic() {
        let mut bw = BandwidthTracker::new(1000);
        bw.record_send(500, 0);
        bw.record_send(500, 500);
        bw.record_recv(1024, 0);
        assert_eq!(bw.total_bytes_sent(), 1000);
        assert_eq!(bw.total_bytes_recv(), 1024);
        // Within window
        let up = bw.bytes_per_sec_up(999);
        assert!(up > 0.0);
    }

    #[test]
    fn test_bandwidth_tracker_evicts_old() {
        let mut bw = BandwidthTracker::new(1000);
        bw.record_send(9999, 0);
        // 2 seconds later — old bucket evicted
        bw.record_send(1, 2001);
        let up = bw.bytes_per_sec_up(2001);
        // Only the recent byte should be in the window
        assert!(up < 5.0, "old data should be evicted, up={up}");
    }

    // ── Position delta ────────────────────────────────────────────────────────

    #[test]
    fn test_position_delta_encoding() {
        let from = 10.0f32;
        let to = 10.5f32;
        let delta = PacketEncoder::encode_position_delta(from, to);
        let recovered = from + PacketDecoder::decode_position_delta(delta);
        assert!((recovered - to).abs() < 0.01, "recovered={recovered}, expected={to}");
    }

    // ── CompressionHint ───────────────────────────────────────────────────────

    #[test]
    fn test_compression_hint_roundtrip() {
        for hint in [CompressionHint::None, CompressionHint::Zlib, CompressionHint::Lz4] {
            assert_eq!(CompressionHint::from_u8(hint.to_u8()).unwrap(), hint);
        }
        assert!(CompressionHint::from_u8(99).is_err());
    }
}
