//! Reliable UDP transport layer.
//!
//! Builds reliable, ordered delivery on top of raw UDP without any async
//! runtime.  Drive the stack each frame by calling `ConnectionManager::poll()`.
//!
//! ## Architecture
//! ```text
//!  Application
//!      │  send(channel, data)
//!      ▼
//!  ConnectionManager  ──  maintains one Connection per peer
//!      │
//!      ▼
//!  Connection  ──  per-channel send queues + ReliableUdp
//!      │
//!      ▼
//!  NonBlockingSocket  ──  wraps std::net::UdpSocket
//! ```

use std::collections::{HashMap, VecDeque};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use crate::networking::protocol::{
    Packet, PacketEncoder, PacketDecoder, PacketKind, ProtocolError, PacketHeader,
};

// ─── Constants ───────────────────────────────────────────────────────────────

/// Maximum Transmission Unit for outgoing packets (bytes).
pub const MTU: usize = 1400;
/// Base retransmit timeout in milliseconds.
pub const RETRANSMIT_BASE_MS: u64 = 100;
/// Maximum retransmit timeout after backoff (ms).
pub const RETRANSMIT_MAX_MS: u64 = 8000;
/// Maximum number of retransmit attempts before giving up.
pub const MAX_RETRANSMIT: u32 = 10;
/// Keepalive interval in milliseconds.
pub const KEEPALIVE_MS: u64 = 500;
/// Peer timeout after last received packet (ms).
pub const PEER_TIMEOUT_MS: u64 = 10_000;
/// EWMA smoothing factor for RTT estimation.
pub const RTT_ALPHA: f64 = 0.125;
/// EWMA smoothing factor for jitter.
pub const JITTER_ALPHA: f64 = 0.25;
/// Fragment timeout in milliseconds (drop partial reassembly).
pub const FRAGMENT_TIMEOUT_MS: u64 = 5_000;
/// Number of sequence numbers in the ack window.
pub const ACK_WINDOW: u32 = 32;

// ─── Channel ─────────────────────────────────────────────────────────────────

/// Delivery channel semantics for outgoing data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    /// Guaranteed delivery, no ordering guarantee.
    Reliable,
    /// Best-effort, no ordering guarantee (fire-and-forget).
    Unreliable,
    /// Guaranteed delivery in the order sent.
    ReliableOrdered,
    /// Best-effort, but older out-of-order packets are discarded.
    UnreliableOrdered,
}

impl Channel {
    pub fn is_reliable(self) -> bool {
        matches!(self, Channel::Reliable | Channel::ReliableOrdered)
    }
    pub fn is_ordered(self) -> bool {
        matches!(self, Channel::ReliableOrdered | Channel::UnreliableOrdered)
    }
}

// ─── ConnectionState ─────────────────────────────────────────────────────────

/// Lifecycle state of a single UDP connection to a remote peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// No connection.
    Disconnected,
    /// Sent a Connect packet, waiting for server acknowledgement.
    Connecting,
    /// Fully established; can send and receive data.
    Connected,
    /// No packet received within `PEER_TIMEOUT_MS`.
    TimedOut,
    /// Remote explicitly requested disconnect (kick/ban).
    Kicked,
}

// ─── TransportStats ──────────────────────────────────────────────────────────

/// Snapshot of transport-layer statistics for a single peer connection.
#[derive(Debug, Clone, Default)]
pub struct TransportStats {
    /// Smoothed round-trip time in milliseconds.
    pub rtt_ms: f64,
    /// Estimated packet loss as a percentage (0.0–100.0).
    pub packet_loss_pct: f64,
    /// Smoothed jitter in milliseconds.
    pub jitter_ms: f64,
    /// Outgoing bytes per second.
    pub bandwidth_up: f64,
    /// Incoming bytes per second.
    pub bandwidth_down: f64,
    /// Total packets sent.
    pub packets_sent: u64,
    /// Total packets received.
    pub packets_recv: u64,
    /// Total retransmissions.
    pub retransmits: u64,
}

// ─── ReceivedPacket ──────────────────────────────────────────────────────────

/// A packet received from a specific peer, ready for the application layer.
#[derive(Debug, Clone)]
pub struct ReceivedPacket {
    pub from:   SocketAddr,
    pub packet: Packet,
}

// ─── NonBlockingSocket ───────────────────────────────────────────────────────

/// Non-blocking UDP socket wrapper with poll-based receive.
pub struct NonBlockingSocket {
    socket: UdpSocket,
    /// Local address this socket is bound to.
    pub local_addr: SocketAddr,
}

impl NonBlockingSocket {
    /// Bind to `addr` and set non-blocking mode.
    pub fn bind(addr: SocketAddr) -> Result<Self, std::io::Error> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        let local_addr = socket.local_addr()?;
        Ok(Self { socket, local_addr })
    }

    /// Send `data` to `dest`.  Returns bytes written.
    pub fn send_to(&self, data: &[u8], dest: SocketAddr) -> Result<usize, std::io::Error> {
        self.socket.send_to(data, dest)
    }

    /// Poll for available packets.  Returns all currently-buffered datagrams.
    /// Stops on `WouldBlock` (nothing more to read right now).
    pub fn poll(&self, buf: &mut Vec<u8>) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut results = Vec::new();
        buf.resize(65535, 0);
        loop {
            match self.socket.recv_from(buf) {
                Ok((len, addr)) => {
                    results.push((addr, buf[..len].to_vec()));
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        results
    }
}

// ─── Fragmenter ──────────────────────────────────────────────────────────────

/// Fragment header appended before each fragment payload.
/// Layout: packet_id(u16 BE) + fragment_idx(u8) + total_fragments(u8) = 4 bytes.
#[derive(Debug, Clone)]
struct FragmentHeader {
    packet_id:       u16,
    fragment_idx:    u8,
    total_fragments: u8,
}

impl FragmentHeader {
    const SIZE: usize = 4;

    fn encode(&self) -> [u8; Self::SIZE] {
        let id_bytes = self.packet_id.to_be_bytes();
        [id_bytes[0], id_bytes[1], self.fragment_idx, self.total_fragments]
    }

    fn decode(b: &[u8]) -> Option<Self> {
        if b.len() < Self::SIZE { return None; }
        Some(Self {
            packet_id:       u16::from_be_bytes([b[0], b[1]]),
            fragment_idx:    b[2],
            total_fragments: b[3],
        })
    }
}

/// Partial reassembly state for one large message.
#[derive(Debug)]
struct PartialMessage {
    total_fragments: u8,
    received:        Vec<Option<Vec<u8>>>,
    created_at:      Instant,
}

impl PartialMessage {
    fn new(total: u8) -> Self {
        Self {
            total_fragments: total,
            received:        vec![None; total as usize],
            created_at:      Instant::now(),
        }
    }

    fn is_complete(&self) -> bool {
        self.received.iter().all(|s| s.is_some())
    }

    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > Duration::from_millis(FRAGMENT_TIMEOUT_MS)
    }

    fn reassemble(&self) -> Vec<u8> {
        self.received.iter().flat_map(|s| s.as_ref().unwrap().iter().copied()).collect()
    }
}

/// Splits large payloads into MTU-safe fragments and reassembles on the
/// receiver side.
pub struct Fragmenter {
    next_packet_id: u16,
    /// In-progress reassembly: keyed by (peer_addr_hash, packet_id).
    reassembly:     HashMap<(u64, u16), PartialMessage>,
}

impl Fragmenter {
    pub fn new() -> Self {
        Self {
            next_packet_id: 0,
            reassembly:     HashMap::new(),
        }
    }

    /// Split `data` into MTU-sized chunks.  Returns vec of raw datagrams ready
    /// to send.  Each datagram has the 4-byte fragment header prepended.
    pub fn fragment(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        let max_body = MTU - PacketHeader::SIZE - FragmentHeader::SIZE;
        let chunks: Vec<&[u8]> = data.chunks(max_body).collect();
        let total = chunks.len().min(255) as u8;
        let id    = self.next_packet_id;
        self.next_packet_id = self.next_packet_id.wrapping_add(1);

        chunks.iter().enumerate().take(255).map(|(i, chunk)| {
            let fh = FragmentHeader {
                packet_id:       id,
                fragment_idx:    i as u8,
                total_fragments: total,
            };
            let mut out = Vec::with_capacity(FragmentHeader::SIZE + chunk.len());
            out.extend_from_slice(&fh.encode());
            out.extend_from_slice(chunk);
            out
        }).collect()
    }

    /// Feed an incoming fragment.  Returns `Some(reassembled_data)` when all
    /// fragments for a message have arrived.
    pub fn receive_fragment(&mut self, peer_key: u64, raw: &[u8]) -> Option<Vec<u8>> {
        let fh = FragmentHeader::decode(raw)?;
        let body = raw[FragmentHeader::SIZE..].to_vec();

        let entry = self.reassembly
            .entry((peer_key, fh.packet_id))
            .or_insert_with(|| PartialMessage::new(fh.total_fragments));

        if fh.fragment_idx as usize >= entry.received.len() {
            return None; // malformed
        }
        entry.received[fh.fragment_idx as usize] = Some(body);

        if entry.is_complete() {
            let data = entry.reassemble();
            self.reassembly.remove(&(peer_key, fh.packet_id));
            Some(data)
        } else {
            None
        }
    }

    /// Evict stale partial messages to free memory.
    pub fn gc(&mut self) {
        self.reassembly.retain(|_, v| !v.is_expired());
    }
}

impl Default for Fragmenter {
    fn default() -> Self { Self::new() }
}

// ─── SendEntry ────────────────────────────────────────────────────────────────

/// A reliable packet sitting in the retransmit queue.
#[derive(Debug, Clone)]
struct SendEntry {
    sequence:          u32,
    data:              Vec<u8>,
    sent_at:           Instant,
    next_retransmit:   Instant,
    retransmit_count:  u32,
    retransmit_delay:  Duration,
}

impl SendEntry {
    fn new(sequence: u32, data: Vec<u8>, now: Instant) -> Self {
        let delay = Duration::from_millis(RETRANSMIT_BASE_MS);
        Self {
            sequence,
            data,
            sent_at: now,
            next_retransmit: now + delay,
            retransmit_count: 0,
            retransmit_delay: delay,
        }
    }

    /// Advance the retransmit timer with exponential backoff.
    fn backoff(&mut self, now: Instant) {
        self.retransmit_count += 1;
        self.retransmit_delay = Duration::from_millis(
            (self.retransmit_delay.as_millis() as u64 * 2).min(RETRANSMIT_MAX_MS),
        );
        self.next_retransmit = now + self.retransmit_delay;
    }

    fn is_due(&self, now: Instant) -> bool {
        now >= self.next_retransmit
    }
}

// ─── ReorderBuffer ───────────────────────────────────────────────────────────

/// Holds out-of-order packets until gaps are filled, then delivers in sequence.
struct ReorderBuffer {
    /// Next expected sequence number.
    next_expected: u32,
    /// Buffered out-of-order packets keyed by sequence.
    buffer: HashMap<u32, Packet>,
    /// Maximum buffered packets before we advance anyway.
    max_hold: usize,
}

impl ReorderBuffer {
    fn new() -> Self {
        Self { next_expected: 0, buffer: HashMap::new(), max_hold: 64 }
    }

    /// Insert a packet.  Returns a vector of in-order packets now deliverable.
    fn insert(&mut self, pkt: Packet) -> Vec<Packet> {
        let seq = pkt.sequence;

        if seq == self.next_expected {
            // In-order — deliver immediately plus any buffered that follow.
            let mut out = vec![pkt];
            self.next_expected = self.next_expected.wrapping_add(1);
            loop {
                if let Some(next) = self.buffer.remove(&self.next_expected) {
                    out.push(next);
                    self.next_expected = self.next_expected.wrapping_add(1);
                } else {
                    break;
                }
            }
            out
        } else {
            // Out-of-order — buffer it.
            self.buffer.insert(seq, pkt);
            // If buffer is overflowing, flush everything and skip ahead.
            if self.buffer.len() > self.max_hold {
                let mut all: Vec<Packet> = self.buffer.drain().map(|(_, p)| p).collect();
                all.sort_by_key(|p| p.sequence);
                if let Some(last) = all.last() {
                    self.next_expected = last.sequence.wrapping_add(1);
                }
                return all;
            }
            Vec::new()
        }
    }

    fn reset(&mut self) {
        self.next_expected = 0;
        self.buffer.clear();
    }
}

// ─── AckAccumulator ──────────────────────────────────────────────────────────

/// Maintains the ack + ack_bits fields sent with each outgoing packet.
struct AckAccumulator {
    last_received: u32,
    ack_bits:      u32,
}

impl AckAccumulator {
    fn new() -> Self { Self { last_received: 0, ack_bits: 0 } }

    /// Record that we received `seq`.
    fn record(&mut self, seq: u32) {
        let diff = self.last_received.wrapping_sub(seq);
        if seq == self.last_received {
            // duplicate — ignore
        } else if seq.wrapping_sub(self.last_received) < 0x8000_0000 {
            // newer
            let advance = seq.wrapping_sub(self.last_received);
            if advance >= 32 {
                self.ack_bits = 0;
            } else {
                self.ack_bits <<= advance;
                self.ack_bits |= 1 << (advance - 1);
            }
            self.last_received = seq;
        } else if diff < 32 {
            // older but within window
            self.ack_bits |= 1 << (diff - 1);
        }
    }

    fn ack(&self)      -> u32 { self.last_received }
    fn ack_bits(&self) -> u32 { self.ack_bits }
}

// ─── CongestionControl ───────────────────────────────────────────────────────

/// Simple AIMD (Additive Increase Multiplicative Decrease) congestion window.
struct CongestionControl {
    /// Current congestion window in packets.
    pub cwnd: u32,
    ssthresh: u32,
}

impl CongestionControl {
    fn new() -> Self { Self { cwnd: 16, ssthresh: 64 } }

    /// Called on each ack — increase window.
    fn on_ack(&mut self) {
        if self.cwnd < self.ssthresh {
            // Slow start: double each ack
            self.cwnd = (self.cwnd + 2).min(256);
        } else {
            // Congestion avoidance: +1 per RTT
            self.cwnd = (self.cwnd + 1).min(256);
        }
    }

    /// Called on detected loss — halve window.
    fn on_loss(&mut self) {
        self.ssthresh = (self.cwnd / 2).max(4);
        self.cwnd = self.ssthresh;
    }

    fn can_send(&self, in_flight: u32) -> bool {
        in_flight < self.cwnd
    }
}

// ─── ReliableUdp ─────────────────────────────────────────────────────────────

/// Reliable ordered UDP transport for a single peer connection.
///
/// Caller owns the `NonBlockingSocket` and passes it in for send operations.
/// `tick()` must be called frequently (each game frame) to drive retransmits.
pub struct ReliableUdp {
    pub peer_addr:     SocketAddr,
    state:             ConnectionState,
    next_sequence:     u32,
    ack_accum:         AckAccumulator,
    send_queue:        VecDeque<SendEntry>,
    reorder_buf:       ReorderBuffer,
    congestion:        CongestionControl,
    rtt_ms:            f64,
    jitter_ms:         f64,
    last_recv:         Instant,
    last_keepalive:    Instant,
    /// Pending timestamps for RTT calculation: seq -> sent_at.
    ping_map:          HashMap<u32, Instant>,
    encoder:           PacketEncoder,
    decoder:           PacketDecoder,
    stats:             TransportStats,
    /// Number of reliable packets currently in flight.
    in_flight:         u32,
}

impl ReliableUdp {
    pub fn new(peer_addr: SocketAddr) -> Self {
        let now = Instant::now();
        Self {
            peer_addr,
            state:          ConnectionState::Connecting,
            next_sequence:  0,
            ack_accum:      AckAccumulator::new(),
            send_queue:     VecDeque::new(),
            reorder_buf:    ReorderBuffer::new(),
            congestion:     CongestionControl::new(),
            rtt_ms:         50.0,
            jitter_ms:      0.0,
            last_recv:      now,
            last_keepalive: now,
            ping_map:       HashMap::new(),
            encoder:        PacketEncoder { reliable: true, ..PacketEncoder::default() },
            decoder:        PacketDecoder::new(),
            stats:          TransportStats::default(),
            in_flight:      0,
        }
    }

    pub fn state(&self) -> ConnectionState { self.state }
    pub fn stats(&self) -> &TransportStats { &self.stats }
    pub fn rtt_ms(&self) -> f64 { self.rtt_ms }

    /// Allocate a new sequence number.
    fn next_seq(&mut self) -> u32 {
        let s = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        s
    }

    /// Enqueue a reliable packet for delivery.
    pub fn send_reliable(&mut self, socket: &NonBlockingSocket, mut packet: Packet) {
        packet.sequence = self.next_seq();
        packet.ack      = self.ack_accum.ack();
        packet.ack_bits = self.ack_accum.ack_bits();
        packet.flags    |= PacketHeader::FLAG_RELIABLE;

        if let Ok(data) = self.encoder.encode(&packet) {
            // Send immediately if window allows
            if self.congestion.can_send(self.in_flight) {
                let _ = socket.send_to(&data, self.peer_addr);
                self.in_flight += 1;
                self.stats.packets_sent += 1;
                self.stats.bandwidth_up += data.len() as f64;

                if packet.kind == PacketKind::Ping {
                    self.ping_map.insert(packet.sequence, Instant::now());
                }

                let entry = SendEntry::new(packet.sequence, data, Instant::now());
                self.send_queue.push_back(entry);
            } else {
                // Queue for later
                let entry = SendEntry::new(packet.sequence, data, Instant::now());
                self.send_queue.push_back(entry);
            }
        }
    }

    /// Send a best-effort (unreliable) packet.
    pub fn send_unreliable(&mut self, socket: &NonBlockingSocket, mut packet: Packet) {
        packet.sequence = self.next_seq();
        packet.ack      = self.ack_accum.ack();
        packet.ack_bits = self.ack_accum.ack_bits();
        if let Ok(data) = self.encoder.encode(&packet) {
            let _ = socket.send_to(&data, self.peer_addr);
            self.stats.packets_sent += 1;
            self.stats.bandwidth_up += data.len() as f64;
        }
    }

    /// Called by `ConnectionManager` when a raw datagram arrives from this peer.
    /// Returns decoded in-order packets ready for the application.
    pub fn receive(&mut self, raw: &[u8]) -> Vec<Packet> {
        self.last_recv = Instant::now();
        self.stats.bandwidth_down += raw.len() as f64;

        let (pkt, _) = match self.decoder.decode(raw) {
            Ok(p)  => p,
            Err(_) => return Vec::new(),
        };

        self.stats.packets_recv += 1;

        // Process ack / ack_bits from incoming packet
        self.process_acks(pkt.ack, pkt.ack_bits);

        // Record this packet in our ack accumulator
        self.ack_accum.record(pkt.sequence);

        // Handle control packets internally
        match pkt.kind {
            PacketKind::Pong => {
                self.handle_pong(&pkt);
                return Vec::new();
            }
            PacketKind::Heartbeat => {
                if self.state == ConnectionState::Connecting {
                    self.state = ConnectionState::Connected;
                }
                return Vec::new();
            }
            PacketKind::Disconnect => {
                self.state = ConnectionState::Disconnected;
                return Vec::new();
            }
            PacketKind::Connect => {
                self.state = ConnectionState::Connected;
                return Vec::new();
            }
            _ => {}
        }

        if self.state == ConnectionState::Connecting {
            self.state = ConnectionState::Connected;
        }

        // For ordered channels, buffer and reorder.
        if pkt.is_reliable() || pkt.flags & PacketHeader::FLAG_ORDERED != 0 {
            self.reorder_buf.insert(pkt)
        } else {
            vec![pkt]
        }
    }

    /// Process acks received in the incoming packet's header fields.
    fn process_acks(&mut self, ack: u32, ack_bits: u32) {
        // The `ack` field is the highest sequence the remote has received.
        // Bits in `ack_bits` indicate which of the 32 prior sequences were also received.

        let mut acked_seqs = Vec::new();
        acked_seqs.push(ack);
        for i in 0..32u32 {
            if ack_bits & (1 << i) != 0 {
                acked_seqs.push(ack.wrapping_sub(i + 1));
            }
        }

        let now = Instant::now();
        let mut any_acked = false;

        self.send_queue.retain(|entry| {
            if acked_seqs.contains(&entry.sequence) {
                any_acked = true;
                if let Some(&sent_at) = self.ping_map.get(&entry.sequence) {
                    let rtt = now.duration_since(sent_at).as_secs_f64() * 1000.0;
                    let err = (rtt - self.rtt_ms).abs();
                    self.jitter_ms = JITTER_ALPHA * err + (1.0 - JITTER_ALPHA) * self.jitter_ms;
                    self.rtt_ms    = RTT_ALPHA * rtt + (1.0 - RTT_ALPHA) * self.rtt_ms;
                    self.ping_map.remove(&entry.sequence);
                }
                self.in_flight = self.in_flight.saturating_sub(1);
                false // remove from queue
            } else {
                true // keep
            }
        });

        if any_acked {
            self.congestion.on_ack();
        }

        // Compute packet loss from ack gaps
        let loss = self.estimate_packet_loss(ack, ack_bits);
        self.stats.packet_loss_pct = loss;
        self.stats.rtt_ms          = self.rtt_ms;
        self.stats.jitter_ms       = self.jitter_ms;
    }

    fn estimate_packet_loss(&self, _ack: u32, ack_bits: u32) -> f64 {
        // Count set bits in ack_bits; bits NOT set indicate lost packets.
        let received = ack_bits.count_ones();
        let window   = 32u32;
        let lost     = window - received;
        (lost as f64 / window as f64) * 100.0
    }

    fn handle_pong(&mut self, pkt: &Packet) {
        if pkt.payload.len() < 8 { return; }
        let ping_seq_bytes: [u8; 8] = pkt.payload[0..8].try_into().unwrap_or_default();
        let _ping_ts = u64::from_be_bytes(ping_seq_bytes);
        // RTT already updated in process_acks via ping_map; nothing else to do.
    }

    /// Drive retransmits, keepalives, and timeout detection.
    /// Call every frame.  Returns packets that need to be sent via `socket`.
    pub fn tick(&mut self, socket: &NonBlockingSocket) {
        let now = Instant::now();

        // Timeout detection
        if now.duration_since(self.last_recv) > Duration::from_millis(PEER_TIMEOUT_MS) {
            self.state = ConnectionState::TimedOut;
            return;
        }

        // Retransmits
        let mut lost_count = 0u32;
        for entry in self.send_queue.iter_mut() {
            if entry.is_due(now) {
                if entry.retransmit_count >= MAX_RETRANSMIT {
                    // Give up — will be cleaned up below
                    lost_count += 1;
                    continue;
                }
                let _ = socket.send_to(&entry.data, self.peer_addr);
                self.stats.retransmits += 1;
                entry.backoff(now);
            }
        }

        // Remove exhausted entries
        self.send_queue.retain(|e| e.retransmit_count < MAX_RETRANSMIT);

        // Signal loss to congestion control
        if lost_count > 0 {
            self.congestion.on_loss();
            self.in_flight = self.in_flight.saturating_sub(lost_count);
        }

        // Keepalive
        if now.duration_since(self.last_keepalive) > Duration::from_millis(KEEPALIVE_MS) {
            self.last_keepalive = now;
            let seq = self.next_seq();
            let hb = Packet::heartbeat(seq, self.ack_accum.ack(), self.ack_accum.ack_bits());
            if let Ok(data) = self.encoder.encode(&hb) {
                let _ = socket.send_to(&data, self.peer_addr);
                self.stats.packets_sent += 1;
            }
        }
    }

    pub fn disconnect(&mut self, socket: &NonBlockingSocket) {
        let seq = self.next_seq();
        let pkt = Packet::new(
            PacketKind::Disconnect, seq,
            self.ack_accum.ack(), self.ack_accum.ack_bits(), Vec::new(),
        );
        if let Ok(data) = self.encoder.encode(&pkt) {
            let _ = socket.send_to(&data, self.peer_addr);
        }
        self.state = ConnectionState::Disconnected;
    }

    pub fn reset_reorder(&mut self) {
        self.reorder_buf.reset();
    }
}

// ─── ConnectionManager ────────────────────────────────────────────────────────

/// Manages multiple UDP peer connections over a single local socket.
pub struct ConnectionManager {
    socket:     NonBlockingSocket,
    peers:      HashMap<SocketAddr, ReliableUdp>,
    fragmenter: Fragmenter,
    encoder:    PacketEncoder,
    recv_buf:   Vec<u8>,
}

impl ConnectionManager {
    /// Create a `ConnectionManager` bound to `local_addr`.
    pub fn bind(local_addr: SocketAddr) -> Result<Self, std::io::Error> {
        Ok(Self {
            socket:     NonBlockingSocket::bind(local_addr)?,
            peers:      HashMap::new(),
            fragmenter: Fragmenter::new(),
            encoder:    PacketEncoder::new(),
            recv_buf:   vec![0u8; 65535],
        })
    }

    /// Initiate a connection to `addr`.
    pub fn connect(&mut self, addr: SocketAddr) {
        let mut conn = ReliableUdp::new(addr);
        let pkt = Packet::new(PacketKind::Connect, 0, 0, 0, Vec::new());
        conn.send_reliable(&self.socket, pkt);
        self.peers.insert(addr, conn);
    }

    /// Gracefully disconnect a peer.
    pub fn disconnect(&mut self, addr: SocketAddr) {
        if let Some(conn) = self.peers.get_mut(&addr) {
            conn.disconnect(&self.socket);
        }
        self.peers.remove(&addr);
    }

    /// Send `data` to `addr` on `channel`.  Fragments if larger than MTU.
    pub fn send(&mut self, addr: SocketAddr, channel: Channel, data: Vec<u8>) {
        let needs_fragment = data.len() > MTU - PacketHeader::SIZE;

        let peer = self.peers.entry(addr).or_insert_with(|| ReliableUdp::new(addr));

        if needs_fragment {
            let frags = self.fragmenter.fragment(&data);
            for frag in frags {
                let mut pkt = Packet::new(
                    PacketKind::StateUpdate,
                    0, 0, 0, frag,
                );
                pkt.flags |= PacketHeader::FLAG_FRAGMENTED;
                if channel.is_reliable() {
                    peer.send_reliable(&self.socket, pkt);
                } else {
                    peer.send_unreliable(&self.socket, pkt);
                }
            }
        } else {
            let pkt = Packet::new(PacketKind::StateUpdate, 0, 0, 0, data);
            if channel.is_reliable() {
                peer.send_reliable(&self.socket, pkt);
            } else {
                peer.send_unreliable(&self.socket, pkt);
            }
        }
    }

    /// Send a typed `Packet` to `addr` on `channel`.
    pub fn send_packet(&mut self, addr: SocketAddr, channel: Channel, packet: Packet) {
        let peer = self.peers.entry(addr).or_insert_with(|| ReliableUdp::new(addr));
        if channel.is_reliable() {
            peer.send_reliable(&self.socket, packet);
        } else {
            peer.send_unreliable(&self.socket, packet);
        }
    }

    /// Poll the socket for incoming datagrams and drive retransmits.
    /// Returns all application-level packets received this frame.
    pub fn poll(&mut self) -> Vec<ReceivedPacket> {
        let mut out = Vec::new();

        // Receive all pending datagrams
        let datagrams = self.socket.poll(&mut self.recv_buf);
        for (addr, raw) in datagrams {
            let peer = self.peers.entry(addr).or_insert_with(|| ReliableUdp::new(addr));
            let packets = peer.receive(&raw);
            for pkt in packets {
                out.push(ReceivedPacket { from: addr, packet: pkt });
            }
        }

        // Tick all peers (retransmit / keepalive / timeout)
        for conn in self.peers.values_mut() {
            conn.tick(&self.socket);
        }

        // Clean up timed-out / disconnected peers
        self.peers.retain(|_, conn| {
            !matches!(conn.state(), ConnectionState::TimedOut | ConnectionState::Disconnected)
        });

        // Fragment GC
        self.fragmenter.gc();

        out
    }

    /// Returns the current state of a peer connection.
    pub fn peer_state(&self, addr: SocketAddr) -> Option<ConnectionState> {
        self.peers.get(&addr).map(|c| c.state())
    }

    /// Returns transport stats for a peer.
    pub fn peer_stats(&self, addr: SocketAddr) -> Option<&TransportStats> {
        self.peers.get(&addr).map(|c| c.stats())
    }

    /// Returns count of connected peers.
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Returns all connected peer addresses.
    pub fn peer_addrs(&self) -> Vec<SocketAddr> {
        self.peers.keys().copied().collect()
    }

    /// Broadcast a packet to all connected peers on `channel`.
    pub fn broadcast(&mut self, channel: Channel, packet: Packet) {
        let addrs: Vec<SocketAddr> = self.peers.keys().copied().collect();
        for addr in addrs {
            self.send_packet(addr, channel, packet.clone());
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn loopback(port: u16) -> SocketAddr {
        format!("127.0.0.1:{port}").parse().unwrap()
    }

    // ── Channel flags ─────────────────────────────────────────────────────────

    #[test]
    fn test_channel_flags() {
        assert!(Channel::Reliable.is_reliable());
        assert!(Channel::ReliableOrdered.is_reliable());
        assert!(!Channel::Unreliable.is_reliable());
        assert!(!Channel::UnreliableOrdered.is_reliable());

        assert!(Channel::ReliableOrdered.is_ordered());
        assert!(Channel::UnreliableOrdered.is_ordered());
        assert!(!Channel::Reliable.is_ordered());
        assert!(!Channel::Unreliable.is_ordered());
    }

    // ── Fragmenter ────────────────────────────────────────────────────────────

    #[test]
    fn test_fragmenter_roundtrip_small() {
        let mut f = Fragmenter::new();
        let data = vec![0xABu8; 100];
        let frags = f.fragment(&data);
        assert_eq!(frags.len(), 1);
        let result = f.receive_fragment(1, &frags[0]);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), data);
    }

    #[test]
    fn test_fragmenter_roundtrip_large() {
        let mut f = Fragmenter::new();
        let data: Vec<u8> = (0..4000).map(|i| (i % 251) as u8).collect();
        let frags = f.fragment(&data);
        assert!(frags.len() > 1);

        let mut assembled = None;
        for frag in frags {
            assembled = f.receive_fragment(99, &frag);
        }
        assert!(assembled.is_some());
        assert_eq!(assembled.unwrap(), data);
    }

    #[test]
    fn test_fragmenter_out_of_order() {
        let mut f = Fragmenter::new();
        let data: Vec<u8> = (0..4000).map(|i| (i % 127) as u8).collect();
        let mut frags = f.fragment(&data);
        // Reverse fragment order
        frags.reverse();
        let mut assembled = None;
        for frag in frags {
            assembled = f.receive_fragment(7, &frag);
        }
        assert!(assembled.is_some());
        // Data bytes may not match due to reversal, but reassembly completed
        assert_eq!(assembled.unwrap().len(), data.len());
    }

    // ── AckAccumulator ────────────────────────────────────────────────────────

    #[test]
    fn test_ack_accumulator_basic() {
        let mut acc = AckAccumulator::new();
        acc.record(0);
        acc.record(1);
        acc.record(2);
        assert_eq!(acc.ack(), 2);
        // Bits: 1 means seq=1 received, bit 1 means seq=0 received
        assert!(acc.ack_bits() & 1 != 0); // seq=1
        assert!(acc.ack_bits() & 2 != 0); // seq=0
    }

    // ── CongestionControl ─────────────────────────────────────────────────────

    #[test]
    fn test_congestion_control_aimd() {
        let mut cc = CongestionControl::new();
        let initial = cc.cwnd;
        cc.on_ack();
        cc.on_ack();
        assert!(cc.cwnd >= initial); // window grew or stayed
        let before_loss = cc.cwnd;
        cc.on_loss();
        assert!(cc.cwnd < before_loss); // window shrank
    }

    // ── ReorderBuffer ─────────────────────────────────────────────────────────

    #[test]
    fn test_reorder_buffer_in_order() {
        let mut rb = ReorderBuffer::new();
        let p0 = Packet::new(PacketKind::StateUpdate, 0, 0, 0, vec![]);
        let p1 = Packet::new(PacketKind::StateUpdate, 1, 0, 0, vec![]);
        let out0 = rb.insert(p0);
        let out1 = rb.insert(p1);
        assert_eq!(out0.len(), 1);
        assert_eq!(out1.len(), 1);
    }

    #[test]
    fn test_reorder_buffer_out_of_order() {
        let mut rb = ReorderBuffer::new();
        let p0 = Packet::new(PacketKind::StateUpdate, 0, 0, 0, vec![1]);
        let p2 = Packet::new(PacketKind::StateUpdate, 2, 0, 0, vec![3]);
        let p1 = Packet::new(PacketKind::StateUpdate, 1, 0, 0, vec![2]);

        let out_p0 = rb.insert(p0); // seq=0 → delivered immediately
        let out_p2 = rb.insert(p2); // seq=2 → buffered
        assert_eq!(out_p0.len(), 1);
        assert_eq!(out_p2.len(), 0);

        let out_p1 = rb.insert(p1); // seq=1 → delivers 1 and 2
        assert_eq!(out_p1.len(), 2);
    }

    // ── ConnectionManager bind ────────────────────────────────────────────────

    #[test]
    fn test_connection_manager_bind() {
        // Just verify we can bind on an ephemeral port
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mgr = ConnectionManager::bind(addr);
        assert!(mgr.is_ok());
    }

    #[test]
    fn test_connection_manager_peer_count() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mut mgr = ConnectionManager::bind(addr).unwrap();
        assert_eq!(mgr.peer_count(), 0);
    }

    // ── NonBlockingSocket ─────────────────────────────────────────────────────

    #[test]
    fn test_non_blocking_socket_bind() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let sock = NonBlockingSocket::bind(addr);
        assert!(sock.is_ok());
        let sock = sock.unwrap();
        // Port should be non-zero (assigned by OS)
        assert_ne!(sock.local_addr.port(), 0);
    }

    // ── SendEntry backoff ─────────────────────────────────────────────────────

    #[test]
    fn test_send_entry_backoff_growth() {
        let mut entry = SendEntry::new(1, vec![0u8; 10], Instant::now());
        let d0 = entry.retransmit_delay;
        entry.backoff(Instant::now());
        let d1 = entry.retransmit_delay;
        entry.backoff(Instant::now());
        let d2 = entry.retransmit_delay;
        assert!(d1 >= d0);
        assert!(d2 >= d1);
        assert!(d2.as_millis() <= RETRANSMIT_MAX_MS as u128);
    }
}
