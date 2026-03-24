//! Network transport layer: reliable/unreliable channels, fragmentation,
//! bandwidth throttling, and connection state machine.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Configuration for the transport layer.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub max_packet_size: usize,
    pub fragment_size: usize,
    pub max_fragments_per_packet: usize,
    pub reliable_window_size: usize,
    pub max_retransmits: u32,
    pub retransmit_timeout_ms: u64,
    pub connection_timeout_ms: u64,
    pub keepalive_interval_ms: u64,
    pub max_bandwidth_bytes_per_sec: usize,
    pub ack_redundancy: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            max_packet_size: 1200,
            fragment_size: 1024,
            max_fragments_per_packet: 256,
            reliable_window_size: 256,
            max_retransmits: 10,
            retransmit_timeout_ms: 200,
            connection_timeout_ms: 10000,
            keepalive_interval_ms: 1000,
            max_bandwidth_bytes_per_sec: 65536,
            ack_redundancy: 3,
        }
    }
}

/// Transport-layer statistics.
#[derive(Debug, Clone, Default)]
pub struct TransportStats {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub packets_lost: u64,
    pub packets_acked: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub retransmissions: u64,
    pub rtt_ms: f64,
    pub rtt_variance_ms: f64,
    pub packet_loss_ratio: f64,
    pub bandwidth_used_bytes_per_sec: f64,
    pub fragments_sent: u64,
    pub fragments_reassembled: u64,
}

impl TransportStats {
    pub fn update_loss_ratio(&mut self) {
        let total = self.packets_sent;
        if total > 0 {
            self.packet_loss_ratio = self.packets_lost as f64 / total as f64;
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Types of packets in the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    ConnectionRequest,
    ConnectionAccept,
    ConnectionDeny,
    Disconnect,
    Keepalive,
    Reliable,
    Unreliable,
    Fragment,
    Ack,
}

impl PacketType {
    pub fn to_u8(self) -> u8 {
        match self {
            PacketType::ConnectionRequest => 0,
            PacketType::ConnectionAccept => 1,
            PacketType::ConnectionDeny => 2,
            PacketType::Disconnect => 3,
            PacketType::Keepalive => 4,
            PacketType::Reliable => 5,
            PacketType::Unreliable => 6,
            PacketType::Fragment => 7,
            PacketType::Ack => 8,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(PacketType::ConnectionRequest),
            1 => Some(PacketType::ConnectionAccept),
            2 => Some(PacketType::ConnectionDeny),
            3 => Some(PacketType::Disconnect),
            4 => Some(PacketType::Keepalive),
            5 => Some(PacketType::Reliable),
            6 => Some(PacketType::Unreliable),
            7 => Some(PacketType::Fragment),
            8 => Some(PacketType::Ack),
            _ => None,
        }
    }
}

/// Header prepended to every packet.
#[derive(Debug, Clone)]
pub struct PacketHeader {
    pub protocol_id: u32,
    pub packet_type: PacketType,
    pub sequence: u16,
    pub ack: u16,
    pub ack_bits: u32,
    pub timestamp_ms: u64,
    pub payload_size: u16,
}

impl PacketHeader {
    pub const SERIALIZED_SIZE: usize = 4 + 1 + 2 + 2 + 4 + 8 + 2;

    pub fn new(packet_type: PacketType, sequence: u16) -> Self {
        Self {
            protocol_id: 0x50524F46, // "PROF"
            packet_type,
            sequence,
            ack: 0,
            ack_bits: 0,
            timestamp_ms: 0,
            payload_size: 0,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::SERIALIZED_SIZE);
        buf.extend_from_slice(&self.protocol_id.to_le_bytes());
        buf.push(self.packet_type.to_u8());
        buf.extend_from_slice(&self.sequence.to_le_bytes());
        buf.extend_from_slice(&self.ack.to_le_bytes());
        buf.extend_from_slice(&self.ack_bits.to_le_bytes());
        buf.extend_from_slice(&self.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&self.payload_size.to_le_bytes());
        buf
    }

    pub fn deserialize(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SERIALIZED_SIZE {
            return None;
        }
        let protocol_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let packet_type = PacketType::from_u8(data[4])?;
        let sequence = u16::from_le_bytes([data[5], data[6]]);
        let ack = u16::from_le_bytes([data[7], data[8]]);
        let ack_bits = u32::from_le_bytes([data[9], data[10], data[11], data[12]]);
        let timestamp_ms = u64::from_le_bytes([
            data[13], data[14], data[15], data[16],
            data[17], data[18], data[19], data[20],
        ]);
        let payload_size = u16::from_le_bytes([data[21], data[22]]);

        Some(Self {
            protocol_id,
            packet_type,
            sequence,
            ack,
            ack_bits,
            timestamp_ms,
            payload_size,
        })
    }

    pub fn validate_protocol(&self) -> bool {
        self.protocol_id == 0x50524F46
    }
}

/// Sequence number math: handles wrapping at u16 boundary.
fn sequence_greater_than(a: u16, b: u16) -> bool {
    ((a > b) && (a - b <= 32768)) || ((a < b) && (b - a > 32768))
}

fn sequence_difference(a: u16, b: u16) -> i32 {
    if sequence_greater_than(a, b) {
        if a >= b { (a - b) as i32 } else { (a as i32 + 65536) - b as i32 }
    } else if a == b {
        0
    } else {
        -(sequence_difference(b, a))
    }
}

/// An entry in the reliable send window.
#[derive(Debug, Clone)]
struct ReliableEntry {
    sequence: u16,
    data: Vec<u8>,
    send_time: Instant,
    retransmit_count: u32,
    acked: bool,
    next_retransmit: Instant,
    channel_id: u8,
}

/// Reliable delivery channel with retransmission and RTT estimation.
pub struct ReliableChannel {
    config: TransportConfig,
    local_sequence: u16,
    remote_sequence: u16,
    send_window: VecDeque<ReliableEntry>,
    receive_buffer: HashMap<u16, Vec<u8>>,
    next_deliver_sequence: u16,
    pending_acks: Vec<u16>,
    rtt_estimate_ms: f64,
    rtt_variance_ms: f64,
    smoothed_rtt_ms: f64,
    stats: TransportStats,
    channel_id: u8,
    ack_bitfield: u32,
    last_ack_sequence: u16,
    send_queue: VecDeque<Vec<u8>>,
}

impl ReliableChannel {
    pub fn new(channel_id: u8, config: TransportConfig) -> Self {
        Self {
            config,
            local_sequence: 0,
            remote_sequence: 0,
            send_window: VecDeque::new(),
            receive_buffer: HashMap::new(),
            next_deliver_sequence: 0,
            pending_acks: Vec::new(),
            rtt_estimate_ms: 100.0,
            rtt_variance_ms: 50.0,
            smoothed_rtt_ms: 100.0,
            stats: TransportStats::default(),
            channel_id,
            ack_bitfield: 0,
            last_ack_sequence: 0,
            send_queue: VecDeque::new(),
        }
    }

    pub fn channel_id(&self) -> u8 {
        self.channel_id
    }

    pub fn stats(&self) -> &TransportStats {
        &self.stats
    }

    pub fn rtt_ms(&self) -> f64 {
        self.smoothed_rtt_ms
    }

    pub fn local_sequence(&self) -> u16 {
        self.local_sequence
    }

    pub fn remote_sequence(&self) -> u16 {
        self.remote_sequence
    }

    /// Queue data for reliable sending. Returns the assigned sequence number.
    pub fn send(&mut self, data: Vec<u8>) -> u16 {
        let seq = self.local_sequence;
        self.local_sequence = self.local_sequence.wrapping_add(1);
        self.send_queue.push_back(data);
        seq
    }

    /// Flush the send queue, producing packets ready to send.
    pub fn flush(&mut self, now: Instant) -> Vec<(PacketHeader, Vec<u8>)> {
        let mut packets = Vec::new();

        while let Some(data) = self.send_queue.pop_front() {
            let seq = self.local_sequence.wrapping_sub(self.send_queue.len() as u16 + 1);
            let rto = self.compute_rto();

            let entry = ReliableEntry {
                sequence: seq,
                data: data.clone(),
                send_time: now,
                retransmit_count: 0,
                acked: false,
                next_retransmit: now + rto,
                channel_id: self.channel_id,
            };

            let mut header = PacketHeader::new(PacketType::Reliable, seq);
            header.ack = self.remote_sequence;
            header.ack_bits = self.ack_bitfield;
            header.payload_size = data.len() as u16;

            self.send_window.push_back(entry);
            self.stats.packets_sent += 1;
            self.stats.bytes_sent += (PacketHeader::SERIALIZED_SIZE + data.len()) as u64;

            packets.push((header, data));
        }

        // Check for retransmissions
        let rto = self.compute_rto();
        let max_retransmits = self.config.max_retransmits;
        let remote_seq = self.remote_sequence;
        let ack_bits = self.ack_bitfield;
        let mut retransmissions = 0u64;
        let mut pkts_sent = 0u64;
        let mut bytes_sent = 0u64;
        let mut pkts_lost = 0u64;
        for entry in self.send_window.iter_mut() {
            if !entry.acked && now >= entry.next_retransmit {
                if entry.retransmit_count < max_retransmits {
                    entry.retransmit_count += 1;
                    // Exponential backoff
                    let backoff = rto * (1 << entry.retransmit_count.min(5));
                    entry.next_retransmit = now + backoff;
                    entry.send_time = now;

                    let mut header = PacketHeader::new(PacketType::Reliable, entry.sequence);
                    header.ack = remote_seq;
                    header.ack_bits = ack_bits;
                    header.payload_size = entry.data.len() as u16;

                    retransmissions += 1;
                    pkts_sent += 1;
                    bytes_sent += (PacketHeader::SERIALIZED_SIZE + entry.data.len()) as u64;

                    packets.push((header, entry.data.clone()));
                } else {
                    pkts_lost += 1;
                }
            }
        }
        self.stats.retransmissions += retransmissions;
        self.stats.packets_sent += pkts_sent;
        self.stats.bytes_sent += bytes_sent;
        self.stats.packets_lost += pkts_lost;

        // Prune old acked entries from the window
        while let Some(front) = self.send_window.front() {
            if front.acked || front.retransmit_count >= self.config.max_retransmits {
                self.send_window.pop_front();
            } else {
                break;
            }
        }

        packets
    }

    fn compute_rto(&self) -> Duration {
        // Jacobson/Karels algorithm: RTO = SRTT + max(G, 4*RTTVAR)
        let rto_ms = self.smoothed_rtt_ms + 4.0 * self.rtt_variance_ms;
        let rto_ms = rto_ms.max(self.config.retransmit_timeout_ms as f64);
        Duration::from_millis(rto_ms as u64)
    }

    /// Process an incoming reliable packet.
    pub fn receive(&mut self, header: &PacketHeader, payload: Vec<u8>, now: Instant) {
        self.stats.packets_received += 1;
        self.stats.bytes_received += (PacketHeader::SERIALIZED_SIZE + payload.len()) as u64;

        let seq = header.sequence;

        // Update remote sequence tracking
        if sequence_greater_than(seq, self.remote_sequence) {
            let diff = sequence_difference(seq, self.remote_sequence);
            // Shift ack bitfield
            if diff > 0 && diff <= 32 {
                self.ack_bitfield <<= diff as u32;
                self.ack_bitfield |= 1 << (diff as u32 - 1);
            } else if diff > 32 {
                self.ack_bitfield = 0;
            }
            self.remote_sequence = seq;
        } else {
            let diff = sequence_difference(self.remote_sequence, seq);
            if diff > 0 && diff <= 32 {
                self.ack_bitfield |= 1 << (diff as u32 - 1);
            }
        }

        // Store in receive buffer for ordered delivery
        self.receive_buffer.insert(seq, payload);
        self.pending_acks.push(seq);

        // Process acks from the remote side
        self.process_acks(header.ack, header.ack_bits, now);
    }

    fn process_acks(&mut self, ack: u16, ack_bits: u32, now: Instant) {
        let mut acked_count = 0u64;
        let mut rtt_samples = Vec::new();
        for entry in self.send_window.iter_mut() {
            if entry.acked {
                continue;
            }
            let seq = entry.sequence;
            let is_acked = if seq == ack {
                true
            } else {
                let diff = sequence_difference(ack, seq);
                diff > 0 && diff <= 32 && (ack_bits & (1 << (diff - 1))) != 0
            };

            if is_acked {
                entry.acked = true;
                acked_count += 1;

                if entry.retransmit_count == 0 {
                    let rtt = now.duration_since(entry.send_time).as_secs_f64() * 1000.0;
                    rtt_samples.push(rtt);
                }
            }
        }
        self.stats.packets_acked += acked_count;
        for rtt in rtt_samples {
            self.update_rtt(rtt);
        }
    }

    fn update_rtt(&mut self, sample_ms: f64) {
        // Jacobson/Karels RTT estimation
        let alpha = 0.125;
        let beta = 0.25;

        let err = sample_ms - self.smoothed_rtt_ms;
        self.smoothed_rtt_ms += alpha * err;
        self.rtt_variance_ms += beta * (err.abs() - self.rtt_variance_ms);
        self.rtt_estimate_ms = sample_ms;
    }

    /// Drain delivered messages in order.
    pub fn drain_received(&mut self) -> Vec<Vec<u8>> {
        let mut messages = Vec::new();
        loop {
            if let Some(data) = self.receive_buffer.remove(&self.next_deliver_sequence) {
                messages.push(data);
                self.next_deliver_sequence = self.next_deliver_sequence.wrapping_add(1);
            } else {
                break;
            }
        }
        messages
    }

    /// Get pending acks to piggyback on outgoing packets.
    pub fn drain_pending_acks(&mut self) -> Vec<u16> {
        std::mem::take(&mut self.pending_acks)
    }

    /// Number of unacked packets in the send window.
    pub fn in_flight(&self) -> usize {
        self.send_window.iter().filter(|e| !e.acked).count()
    }

    /// Whether the send window is full.
    pub fn is_congested(&self) -> bool {
        self.in_flight() >= self.config.reliable_window_size
    }

    /// Reset the channel state.
    pub fn reset(&mut self) {
        self.local_sequence = 0;
        self.remote_sequence = 0;
        self.next_deliver_sequence = 0;
        self.send_window.clear();
        self.receive_buffer.clear();
        self.pending_acks.clear();
        self.send_queue.clear();
        self.smoothed_rtt_ms = 100.0;
        self.rtt_variance_ms = 50.0;
        self.stats.reset();
    }
}

/// Unreliable channel: fire-and-forget with sequence numbers for ordering.
pub struct UnreliableChannel {
    local_sequence: u16,
    remote_sequence: u16,
    stats: TransportStats,
    received_buffer: VecDeque<Vec<u8>>,
    max_buffer_size: usize,
    ack_bitfield: u32,
    drop_out_of_order: bool,
}

impl UnreliableChannel {
    pub fn new() -> Self {
        Self {
            local_sequence: 0,
            remote_sequence: 0,
            stats: TransportStats::default(),
            received_buffer: VecDeque::new(),
            max_buffer_size: 256,
            ack_bitfield: 0,
            drop_out_of_order: false,
        }
    }

    pub fn with_max_buffer(mut self, size: usize) -> Self {
        self.max_buffer_size = size;
        self
    }

    pub fn set_drop_out_of_order(&mut self, drop: bool) {
        self.drop_out_of_order = drop;
    }

    pub fn stats(&self) -> &TransportStats {
        &self.stats
    }

    pub fn local_sequence(&self) -> u16 {
        self.local_sequence
    }

    /// Prepare a packet for unreliable sending.
    pub fn send(&mut self, data: Vec<u8>) -> (PacketHeader, Vec<u8>) {
        let seq = self.local_sequence;
        self.local_sequence = self.local_sequence.wrapping_add(1);

        let mut header = PacketHeader::new(PacketType::Unreliable, seq);
        header.ack = self.remote_sequence;
        header.ack_bits = self.ack_bitfield;
        header.payload_size = data.len() as u16;

        self.stats.packets_sent += 1;
        self.stats.bytes_sent += (PacketHeader::SERIALIZED_SIZE + data.len()) as u64;

        (header, data)
    }

    /// Process an incoming unreliable packet.
    pub fn receive(&mut self, header: &PacketHeader, payload: Vec<u8>) {
        self.stats.packets_received += 1;
        self.stats.bytes_received += (PacketHeader::SERIALIZED_SIZE + payload.len()) as u64;

        let seq = header.sequence;

        // Check if this is newer than what we have
        if self.drop_out_of_order && !sequence_greater_than(seq, self.remote_sequence) && seq != self.remote_sequence {
            // Drop out-of-order packet
            return;
        }

        // Update remote sequence tracking
        if sequence_greater_than(seq, self.remote_sequence) {
            let diff = sequence_difference(seq, self.remote_sequence);
            if diff > 0 && diff <= 32 {
                self.ack_bitfield <<= diff as u32;
                self.ack_bitfield |= 1 << (diff as u32 - 1);
            } else if diff > 32 {
                self.ack_bitfield = 0;
            }
            self.remote_sequence = seq;
        } else {
            let diff = sequence_difference(self.remote_sequence, seq);
            if diff > 0 && diff <= 32 {
                self.ack_bitfield |= 1 << (diff as u32 - 1);
            }
        }

        // Buffer the payload
        self.received_buffer.push_back(payload);
        while self.received_buffer.len() > self.max_buffer_size {
            self.received_buffer.pop_front();
        }
    }

    /// Drain all received messages.
    pub fn drain_received(&mut self) -> Vec<Vec<u8>> {
        self.received_buffer.drain(..).collect()
    }

    pub fn reset(&mut self) {
        self.local_sequence = 0;
        self.remote_sequence = 0;
        self.received_buffer.clear();
        self.stats.reset();
    }
}

/// Header for a fragment of a larger packet.
#[derive(Debug, Clone)]
pub struct FragmentHeader {
    pub group_id: u16,
    pub fragment_index: u8,
    pub total_fragments: u8,
    pub fragment_size: u16,
}

impl FragmentHeader {
    pub const SERIALIZED_SIZE: usize = 2 + 1 + 1 + 2;

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::SERIALIZED_SIZE);
        buf.extend_from_slice(&self.group_id.to_le_bytes());
        buf.push(self.fragment_index);
        buf.push(self.total_fragments);
        buf.extend_from_slice(&self.fragment_size.to_le_bytes());
        buf
    }

    pub fn deserialize(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SERIALIZED_SIZE {
            return None;
        }
        Some(Self {
            group_id: u16::from_le_bytes([data[0], data[1]]),
            fragment_index: data[2],
            total_fragments: data[3],
            fragment_size: u16::from_le_bytes([data[4], data[5]]),
        })
    }
}

/// Reassembly state for a group of fragments.
struct ReassemblyGroup {
    group_id: u16,
    total_fragments: u8,
    received_mask: u64,
    fragments: Vec<Option<Vec<u8>>>,
    creation_time: Instant,
    total_size: usize,
}

impl ReassemblyGroup {
    fn new(group_id: u16, total_fragments: u8, now: Instant) -> Self {
        let mut fragments = Vec::with_capacity(total_fragments as usize);
        for _ in 0..total_fragments {
            fragments.push(None);
        }
        Self {
            group_id,
            total_fragments,
            received_mask: 0,
            fragments,
            creation_time: now,
            total_size: 0,
        }
    }

    fn insert(&mut self, index: u8, data: Vec<u8>) -> bool {
        if index >= self.total_fragments {
            return false;
        }
        let bit = 1u64 << index;
        if self.received_mask & bit != 0 {
            return false; // duplicate
        }
        self.received_mask |= bit;
        self.total_size += data.len();
        self.fragments[index as usize] = Some(data);
        self.is_complete()
    }

    fn is_complete(&self) -> bool {
        let expected = if self.total_fragments >= 64 {
            u64::MAX
        } else {
            (1u64 << self.total_fragments) - 1
        };
        self.received_mask == expected
    }

    fn assemble(&self) -> Option<Vec<u8>> {
        if !self.is_complete() {
            return None;
        }
        let mut result = Vec::with_capacity(self.total_size);
        for frag in &self.fragments {
            if let Some(data) = frag {
                result.extend_from_slice(data);
            } else {
                return None;
            }
        }
        Some(result)
    }

    fn received_count(&self) -> u8 {
        self.received_mask.count_ones() as u8
    }

    fn age(&self, now: Instant) -> Duration {
        now.duration_since(self.creation_time)
    }
}

/// Buffer managing reassembly of fragmented packets.
pub struct ReassemblyBuffer {
    groups: HashMap<u16, ReassemblyGroup>,
    timeout: Duration,
    max_groups: usize,
}

impl ReassemblyBuffer {
    pub fn new(timeout_ms: u64, max_groups: usize) -> Self {
        Self {
            groups: HashMap::new(),
            timeout: Duration::from_millis(timeout_ms),
            max_groups,
        }
    }

    pub fn insert(&mut self, header: &FragmentHeader, data: Vec<u8>, now: Instant) -> Option<Vec<u8>> {
        // Clean up expired groups
        self.cleanup(now);

        let group = self.groups
            .entry(header.group_id)
            .or_insert_with(|| ReassemblyGroup::new(header.group_id, header.total_fragments, now));

        if group.insert(header.fragment_index, data) {
            let assembled = group.assemble();
            self.groups.remove(&header.group_id);
            assembled
        } else {
            None
        }
    }

    fn cleanup(&mut self, now: Instant) {
        self.groups.retain(|_, group| group.age(now) < self.timeout);

        // If still over capacity, remove oldest
        while self.groups.len() > self.max_groups {
            let oldest = self.groups.iter()
                .min_by_key(|(_, g)| g.creation_time)
                .map(|(&id, _)| id);
            if let Some(id) = oldest {
                self.groups.remove(&id);
            } else {
                break;
            }
        }
    }

    pub fn pending_groups(&self) -> usize {
        self.groups.len()
    }

    pub fn clear(&mut self) {
        self.groups.clear();
    }
}

/// Splits large payloads into fragments for transmission.
pub struct PacketFragmenter {
    fragment_size: usize,
    max_fragments: usize,
    next_group_id: u16,
}

impl PacketFragmenter {
    pub fn new(fragment_size: usize, max_fragments: usize) -> Self {
        Self {
            fragment_size: fragment_size.max(64),
            max_fragments: max_fragments.max(1),
            next_group_id: 0,
        }
    }

    /// Check if data needs fragmentation.
    pub fn needs_fragmentation(&self, data_len: usize) -> bool {
        data_len > self.fragment_size
    }

    /// Fragment a payload into multiple pieces with headers.
    pub fn fragment(&mut self, data: &[u8]) -> Vec<(FragmentHeader, Vec<u8>)> {
        if data.is_empty() {
            return Vec::new();
        }

        let total_fragments = ((data.len() + self.fragment_size - 1) / self.fragment_size).min(self.max_fragments);
        let group_id = self.next_group_id;
        self.next_group_id = self.next_group_id.wrapping_add(1);

        let mut fragments = Vec::with_capacity(total_fragments);
        let mut offset = 0;

        for i in 0..total_fragments {
            let end = (offset + self.fragment_size).min(data.len());
            let fragment_data = data[offset..end].to_vec();

            let header = FragmentHeader {
                group_id,
                fragment_index: i as u8,
                total_fragments: total_fragments as u8,
                fragment_size: fragment_data.len() as u16,
            };

            fragments.push((header, fragment_data));
            offset = end;

            if offset >= data.len() {
                break;
            }
        }

        fragments
    }

    pub fn max_payload_size(&self) -> usize {
        self.fragment_size * self.max_fragments
    }
}

/// Bandwidth throttle using a token bucket algorithm.
pub struct BandwidthThrottle {
    max_bytes_per_sec: f64,
    tokens: f64,
    max_tokens: f64,
    last_refill: Instant,
    bytes_sent_this_second: usize,
    second_start: Instant,
    enabled: bool,
}

impl BandwidthThrottle {
    pub fn new(max_bytes_per_sec: usize) -> Self {
        let now = Instant::now();
        Self {
            max_bytes_per_sec: max_bytes_per_sec as f64,
            tokens: max_bytes_per_sec as f64,
            max_tokens: max_bytes_per_sec as f64 * 1.5,
            last_refill: now,
            bytes_sent_this_second: 0,
            second_start: now,
            enabled: true,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_max_bytes_per_sec(&mut self, max: usize) {
        self.max_bytes_per_sec = max as f64;
        self.max_tokens = max as f64 * 1.5;
    }

    pub fn max_bytes_per_sec(&self) -> usize {
        self.max_bytes_per_sec as usize
    }

    /// Refill tokens based on elapsed time.
    pub fn refill(&mut self, now: Instant) {
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens += self.max_bytes_per_sec * elapsed;
        if self.tokens > self.max_tokens {
            self.tokens = self.max_tokens;
        }
        self.last_refill = now;

        // Reset per-second counter
        if now.duration_since(self.second_start).as_secs_f64() >= 1.0 {
            self.bytes_sent_this_second = 0;
            self.second_start = now;
        }
    }

    /// Check if we can send `bytes` right now.
    pub fn can_send(&self, bytes: usize) -> bool {
        if !self.enabled {
            return true;
        }
        self.tokens >= bytes as f64
    }

    /// Consume tokens for sending.
    pub fn consume(&mut self, bytes: usize) {
        self.tokens -= bytes as f64;
        self.bytes_sent_this_second += bytes;
    }

    /// Try to consume tokens; returns true if allowed.
    pub fn try_send(&mut self, bytes: usize, now: Instant) -> bool {
        self.refill(now);
        if self.can_send(bytes) {
            self.consume(bytes);
            true
        } else {
            false
        }
    }

    pub fn available_bytes(&self) -> usize {
        if !self.enabled {
            return usize::MAX;
        }
        self.tokens.max(0.0) as usize
    }

    pub fn utilization(&self) -> f64 {
        if self.max_bytes_per_sec <= 0.0 {
            return 0.0;
        }
        self.bytes_sent_this_second as f64 / self.max_bytes_per_sec
    }

    pub fn reset(&mut self) {
        self.tokens = self.max_bytes_per_sec;
        self.bytes_sent_this_second = 0;
        self.last_refill = Instant::now();
        self.second_start = self.last_refill;
    }
}

/// Connection states in the state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
}

/// Events emitted by the connection state machine.
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    Connected,
    Disconnected { reason: DisconnectReason },
    ConnectionFailed { reason: String },
    TimedOut,
}

/// Reasons for disconnection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    Graceful,
    Timeout,
    Kicked,
    ProtocolError,
    Full,
}

/// Connection state machine managing the lifecycle of a network connection.
pub struct ConnectionStateMachine {
    state: ConnectionState,
    config: TransportConfig,
    connect_started: Option<Instant>,
    last_packet_received: Option<Instant>,
    last_packet_sent: Option<Instant>,
    connect_attempts: u32,
    max_connect_attempts: u32,
    connect_retry_interval: Duration,
    events: VecDeque<ConnectionEvent>,
    disconnect_reason: Option<DisconnectReason>,
    session_id: u64,
    keepalive_due: bool,
}

impl ConnectionStateMachine {
    pub fn new(config: TransportConfig) -> Self {
        Self {
            state: ConnectionState::Disconnected,
            config,
            connect_started: None,
            last_packet_received: None,
            last_packet_sent: None,
            connect_attempts: 0,
            max_connect_attempts: 5,
            connect_retry_interval: Duration::from_millis(500),
            events: VecDeque::new(),
            disconnect_reason: None,
            session_id: 0,
            keepalive_due: false,
        }
    }

    pub fn state(&self) -> ConnectionState {
        self.state
    }

    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    pub fn session_id(&self) -> u64 {
        self.session_id
    }

    /// Begin connecting.
    pub fn connect(&mut self, now: Instant) {
        if self.state != ConnectionState::Disconnected {
            return;
        }
        self.state = ConnectionState::Connecting;
        self.connect_started = Some(now);
        self.connect_attempts = 0;
        self.session_id = generate_session_id(now);
    }

    /// Handle acceptance of our connection.
    pub fn on_accepted(&mut self, now: Instant) {
        if self.state == ConnectionState::Connecting {
            self.state = ConnectionState::Connected;
            self.last_packet_received = Some(now);
            self.events.push_back(ConnectionEvent::Connected);
        }
    }

    /// Handle denial of our connection.
    pub fn on_denied(&mut self, reason: String) {
        if self.state == ConnectionState::Connecting {
            self.state = ConnectionState::Disconnected;
            self.events.push_back(ConnectionEvent::ConnectionFailed { reason });
        }
    }

    /// Handle an incoming packet (any type) to update keep-alive tracking.
    pub fn on_packet_received(&mut self, now: Instant) {
        self.last_packet_received = Some(now);
    }

    /// Mark that we sent a packet.
    pub fn on_packet_sent(&mut self, now: Instant) {
        self.last_packet_sent = Some(now);
        self.keepalive_due = false;
    }

    /// Initiate graceful disconnection.
    pub fn disconnect(&mut self) {
        if self.state == ConnectionState::Connected {
            self.state = ConnectionState::Disconnecting;
            self.disconnect_reason = Some(DisconnectReason::Graceful);
        }
    }

    /// Complete disconnection (after sending disconnect packet).
    pub fn on_disconnect_complete(&mut self) {
        let reason = self.disconnect_reason.unwrap_or(DisconnectReason::Graceful);
        self.state = ConnectionState::Disconnected;
        self.events.push_back(ConnectionEvent::Disconnected { reason });
        self.connect_started = None;
        self.last_packet_received = None;
    }

    /// Handle remote disconnect.
    pub fn on_remote_disconnect(&mut self, reason: DisconnectReason) {
        if self.state == ConnectionState::Connected || self.state == ConnectionState::Connecting {
            self.state = ConnectionState::Disconnected;
            self.disconnect_reason = Some(reason);
            self.events.push_back(ConnectionEvent::Disconnected { reason });
        }
    }

    /// Tick the state machine. Returns true if a connect retry is needed.
    pub fn update(&mut self, now: Instant) -> bool {
        let mut needs_retry = false;

        match self.state {
            ConnectionState::Connecting => {
                if let Some(started) = self.connect_started {
                    let elapsed = now.duration_since(started);
                    if elapsed >= Duration::from_millis(self.config.connection_timeout_ms) {
                        self.state = ConnectionState::Disconnected;
                        self.events.push_back(ConnectionEvent::TimedOut);
                        return false;
                    }

                    // Check if we need to retry
                    let expected_attempts = (elapsed.as_millis() / self.connect_retry_interval.as_millis()).max(1) as u32;
                    if expected_attempts > self.connect_attempts && self.connect_attempts < self.max_connect_attempts {
                        self.connect_attempts += 1;
                        needs_retry = true;
                    }
                }
            }
            ConnectionState::Connected => {
                // Check for timeout
                if let Some(last_recv) = self.last_packet_received {
                    let since_recv = now.duration_since(last_recv);
                    if since_recv >= Duration::from_millis(self.config.connection_timeout_ms) {
                        self.state = ConnectionState::Disconnected;
                        self.disconnect_reason = Some(DisconnectReason::Timeout);
                        self.events.push_back(ConnectionEvent::TimedOut);
                        return false;
                    }
                }

                // Check if keepalive is needed
                if let Some(last_sent) = self.last_packet_sent {
                    let since_sent = now.duration_since(last_sent);
                    if since_sent >= Duration::from_millis(self.config.keepalive_interval_ms) {
                        self.keepalive_due = true;
                    }
                } else {
                    self.keepalive_due = true;
                }
            }
            ConnectionState::Disconnecting => {
                // Just transition to disconnected
                self.on_disconnect_complete();
            }
            ConnectionState::Disconnected => {}
        }

        needs_retry
    }

    /// Whether a keepalive should be sent.
    pub fn needs_keepalive(&self) -> bool {
        self.keepalive_due && self.state == ConnectionState::Connected
    }

    /// Drain pending events.
    pub fn drain_events(&mut self) -> Vec<ConnectionEvent> {
        self.events.drain(..).collect()
    }

    /// Force transition to disconnected.
    pub fn force_disconnect(&mut self, reason: DisconnectReason) {
        self.state = ConnectionState::Disconnected;
        self.disconnect_reason = Some(reason);
        self.events.push_back(ConnectionEvent::Disconnected { reason });
    }

    pub fn reset(&mut self) {
        self.state = ConnectionState::Disconnected;
        self.connect_started = None;
        self.last_packet_received = None;
        self.last_packet_sent = None;
        self.connect_attempts = 0;
        self.events.clear();
        self.disconnect_reason = None;
        self.keepalive_due = false;
    }

    /// Time since last received packet.
    pub fn time_since_last_received(&self, now: Instant) -> Option<Duration> {
        self.last_packet_received.map(|t| now.duration_since(t))
    }
}

/// Generate a pseudo-unique session ID from an Instant.
fn generate_session_id(now: Instant) -> u64 {
    let elapsed = now.elapsed();
    let nanos = elapsed.as_nanos() as u64;
    // FNV-1a hash for spreading bits
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in nanos.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// A complete outgoing packet ready for serialization.
#[derive(Debug, Clone)]
pub struct OutgoingPacket {
    pub header: PacketHeader,
    pub payload: Vec<u8>,
    pub fragment_header: Option<FragmentHeader>,
}

impl OutgoingPacket {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = self.header.serialize();
        if let Some(ref fh) = self.fragment_header {
            buf.extend_from_slice(&fh.serialize());
        }
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn total_size(&self) -> usize {
        PacketHeader::SERIALIZED_SIZE
            + self.fragment_header.as_ref().map_or(0, |_| FragmentHeader::SERIALIZED_SIZE)
            + self.payload.len()
    }
}

/// Deserialize an incoming raw packet into its components.
pub fn deserialize_packet(data: &[u8]) -> Option<(PacketHeader, Option<FragmentHeader>, Vec<u8>)> {
    let header = PacketHeader::deserialize(data)?;
    if !header.validate_protocol() {
        return None;
    }

    let mut offset = PacketHeader::SERIALIZED_SIZE;

    let fragment_header = if header.packet_type == PacketType::Fragment {
        if data.len() < offset + FragmentHeader::SERIALIZED_SIZE {
            return None;
        }
        let fh = FragmentHeader::deserialize(&data[offset..])?;
        offset += FragmentHeader::SERIALIZED_SIZE;
        Some(fh)
    } else {
        None
    };

    let payload = if offset < data.len() {
        data[offset..].to_vec()
    } else {
        Vec::new()
    };

    Some((header, fragment_header, payload))
}

/// Jitter buffer for smoothing network packet delivery timing.
pub struct JitterBuffer {
    buffer: VecDeque<(u64, Vec<u8>)>,
    delay_ms: u64,
    max_size: usize,
}

impl JitterBuffer {
    pub fn new(delay_ms: u64, max_size: usize) -> Self {
        Self {
            buffer: VecDeque::new(),
            delay_ms,
            max_size,
        }
    }

    pub fn push(&mut self, timestamp_ms: u64, data: Vec<u8>) {
        // Insert in sorted order by timestamp
        let pos = self.buffer.iter().position(|(ts, _)| *ts > timestamp_ms);
        match pos {
            Some(idx) => self.buffer.insert(idx, (timestamp_ms, data)),
            None => self.buffer.push_back((timestamp_ms, data)),
        }

        // Trim excess
        while self.buffer.len() > self.max_size {
            self.buffer.pop_front();
        }
    }

    pub fn drain_ready(&mut self, current_time_ms: u64) -> Vec<Vec<u8>> {
        let threshold = current_time_ms.saturating_sub(self.delay_ms);
        let mut ready = Vec::new();
        while let Some(&(ts, _)) = self.buffer.front() {
            if ts <= threshold {
                if let Some((_, data)) = self.buffer.pop_front() {
                    ready.push(data);
                }
            } else {
                break;
            }
        }
        ready
    }

    pub fn set_delay(&mut self, delay_ms: u64) {
        self.delay_ms = delay_ms;
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_header_roundtrip() {
        let mut h = PacketHeader::new(PacketType::Reliable, 42);
        h.ack = 10;
        h.ack_bits = 0xFF00FF00;
        h.timestamp_ms = 123456789;
        h.payload_size = 512;

        let data = h.serialize();
        let h2 = PacketHeader::deserialize(&data).unwrap();
        assert_eq!(h2.sequence, 42);
        assert_eq!(h2.ack, 10);
        assert_eq!(h2.ack_bits, 0xFF00FF00);
        assert_eq!(h2.timestamp_ms, 123456789);
        assert_eq!(h2.payload_size, 512);
        assert!(h2.validate_protocol());
    }

    #[test]
    fn test_sequence_greater_than() {
        assert!(sequence_greater_than(1, 0));
        assert!(sequence_greater_than(100, 99));
        // Wraparound
        assert!(sequence_greater_than(0, 65535));
        assert!(!sequence_greater_than(65535, 0));
    }

    #[test]
    fn test_fragment_header_roundtrip() {
        let fh = FragmentHeader {
            group_id: 7,
            fragment_index: 3,
            total_fragments: 10,
            fragment_size: 1024,
        };
        let data = fh.serialize();
        let fh2 = FragmentHeader::deserialize(&data).unwrap();
        assert_eq!(fh2.group_id, 7);
        assert_eq!(fh2.fragment_index, 3);
        assert_eq!(fh2.total_fragments, 10);
        assert_eq!(fh2.fragment_size, 1024);
    }

    #[test]
    fn test_fragmentation_and_reassembly() {
        let mut fragmenter = PacketFragmenter::new(100, 256);
        let data: Vec<u8> = (0..350).map(|i| (i % 256) as u8).collect();

        let fragments = fragmenter.fragment(&data);
        assert_eq!(fragments.len(), 4);

        let now = Instant::now();
        let mut reassembly = ReassemblyBuffer::new(5000, 32);

        let mut result = None;
        for (fh, fdata) in &fragments {
            result = reassembly.insert(fh, fdata.clone(), now);
        }

        let assembled = result.unwrap();
        assert_eq!(assembled, data);
    }

    #[test]
    fn test_bandwidth_throttle() {
        let mut throttle = BandwidthThrottle::new(1000);
        let now = Instant::now();
        throttle.refill(now);

        assert!(throttle.can_send(500));
        throttle.consume(500);
        assert!(throttle.can_send(500));
        throttle.consume(500);
        assert!(!throttle.can_send(100));
    }

    #[test]
    fn test_connection_state_machine() {
        let config = TransportConfig::default();
        let mut csm = ConnectionStateMachine::new(config);
        let now = Instant::now();

        assert_eq!(csm.state(), ConnectionState::Disconnected);
        csm.connect(now);
        assert_eq!(csm.state(), ConnectionState::Connecting);
        csm.on_accepted(now);
        assert_eq!(csm.state(), ConnectionState::Connected);

        let events = csm.drain_events();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_unreliable_channel() {
        let mut ch = UnreliableChannel::new();
        let (header, payload) = ch.send(vec![1, 2, 3]);
        assert_eq!(header.sequence, 0);

        ch.receive(&header, payload);
        let msgs = ch.drain_received();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0], vec![1, 2, 3]);
    }

    #[test]
    fn test_jitter_buffer() {
        let mut jb = JitterBuffer::new(50, 100);
        jb.push(100, vec![1]);
        jb.push(120, vec![2]);
        jb.push(90, vec![0]);

        let ready = jb.drain_ready(140);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], vec![0]);

        let ready2 = jb.drain_ready(160);
        assert_eq!(ready2.len(), 1);
        assert_eq!(ready2[0], vec![1]);
    }

    #[test]
    fn test_deserialize_packet() {
        let mut header = PacketHeader::new(PacketType::Unreliable, 5);
        header.payload_size = 3;
        let mut data = header.serialize();
        data.extend_from_slice(&[10, 20, 30]);

        let (h, fh, payload) = deserialize_packet(&data).unwrap();
        assert_eq!(h.sequence, 5);
        assert!(fh.is_none());
        assert_eq!(payload, vec![10, 20, 30]);
    }
}
