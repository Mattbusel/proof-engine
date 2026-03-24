//! Rollback networking support — deterministic state snapshots, input prediction,
//! and reconciliation for peer-to-peer fighting/action games.
//!
//! ## Architecture
//! - `GameState` trait — implement this for your game state to enable rollback
//! - `RollbackBuffer` — ring buffer of state snapshots indexed by frame
//! - `InputPredictor` — predicts missing remote inputs based on last known
//! - `RollbackSession` — orchestrates rollback, save/load, re-simulation
//! - `NetworkStats` — RTT, packet loss, and frame delay tracking

use std::collections::{HashMap, VecDeque};

// ── Frame numbering ────────────────────────────────────────────────────────────

pub type Frame = u64;
pub type PlayerId = u8;

pub const MAX_PLAYERS: usize = 4;
pub const MAX_ROLLBACK_FRAMES: usize = 8;
pub const INPUT_DELAY: usize = 2;

// ── PlayerInput ───────────────────────────────────────────────────────────────

/// Serializable snapshot of a single player's input for one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerInput {
    pub buttons:   u32,     // bitmask of held buttons
    pub buttons_pressed:  u32,  // newly pressed this frame
    pub buttons_released: u32,  // newly released this frame
    pub axis_x:    i16,     // left stick X, scaled -32768..32767
    pub axis_y:    i16,     // left stick Y
    pub axis_rx:   i16,     // right stick X
    pub axis_ry:   i16,     // right stick Y
    pub frame:     Frame,
}

impl PlayerInput {
    pub fn is_held(&self, btn: u32) -> bool     { self.buttons & btn != 0 }
    pub fn is_pressed(&self, btn: u32) -> bool  { self.buttons_pressed & btn != 0 }
    pub fn is_released(&self, btn: u32) -> bool { self.buttons_released & btn != 0 }

    pub fn direction(&self) -> (f32, f32) {
        (self.axis_x as f32 / 32767.0, self.axis_y as f32 / 32767.0)
    }

    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..4].copy_from_slice(&self.buttons.to_le_bytes());
        buf[4..8].copy_from_slice(&self.buttons_pressed.to_le_bytes());
        buf[8..10].copy_from_slice(&self.axis_x.to_le_bytes());
        buf[10..12].copy_from_slice(&self.axis_y.to_le_bytes());
        buf[12..14].copy_from_slice(&self.axis_rx.to_le_bytes());
        buf[14..16].copy_from_slice(&self.axis_ry.to_le_bytes());
        buf
    }

    pub fn from_bytes(bytes: &[u8; 16]) -> Self {
        let buttons          = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let buttons_pressed  = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let axis_x  = i16::from_le_bytes([bytes[8],  bytes[9]]);
        let axis_y  = i16::from_le_bytes([bytes[10], bytes[11]]);
        let axis_rx = i16::from_le_bytes([bytes[12], bytes[13]]);
        let axis_ry = i16::from_le_bytes([bytes[14], bytes[15]]);
        Self { buttons, buttons_pressed, buttons_released: 0, axis_x, axis_y, axis_rx, axis_ry, frame: 0 }
    }

    /// Returns true if this input has no active button presses and zero stick.
    pub fn is_neutral(&self) -> bool {
        self.buttons == 0 && self.axis_x == 0 && self.axis_y == 0
    }
}

/// All player inputs for one frame.
#[derive(Debug, Clone, Default)]
pub struct FrameInput {
    pub frame:   Frame,
    pub inputs:  [PlayerInput; MAX_PLAYERS],
    pub confirmed: [bool; MAX_PLAYERS],
}

impl FrameInput {
    pub fn new(frame: Frame) -> Self {
        let mut fi = Self::default();
        fi.frame = frame;
        fi
    }

    pub fn set_input(&mut self, player: PlayerId, input: PlayerInput) {
        let idx = player as usize;
        if idx < MAX_PLAYERS {
            self.inputs[idx] = input;
            self.confirmed[idx] = true;
        }
    }

    pub fn all_confirmed(&self, player_count: u8) -> bool {
        (0..player_count as usize).all(|i| self.confirmed[i])
    }

    pub fn checksum(&self) -> u32 {
        let mut h = 0u32;
        for inp in &self.inputs {
            h ^= inp.buttons;
            h = h.wrapping_add(inp.axis_x as u32).wrapping_mul(0x9e3779b9);
        }
        h
    }
}

// ── InputPredictor ────────────────────────────────────────────────────────────

/// Predicts missing remote player inputs by repeating the last known input.
pub struct InputPredictor {
    last_confirmed: [PlayerInput; MAX_PLAYERS],
    last_confirmed_frame: [Frame; MAX_PLAYERS],
    prediction_streak: [u32; MAX_PLAYERS],
}

impl InputPredictor {
    pub fn new() -> Self {
        Self {
            last_confirmed: [PlayerInput::default(); MAX_PLAYERS],
            last_confirmed_frame: [0; MAX_PLAYERS],
            prediction_streak: [0; MAX_PLAYERS],
        }
    }

    pub fn confirm_input(&mut self, player: PlayerId, input: PlayerInput) {
        let idx = player as usize;
        if idx < MAX_PLAYERS {
            self.last_confirmed[idx] = input;
            self.last_confirmed_frame[idx] = input.frame;
            self.prediction_streak[idx] = 0;
        }
    }

    pub fn predict(&mut self, player: PlayerId, frame: Frame) -> PlayerInput {
        let idx = player as usize;
        if idx >= MAX_PLAYERS { return PlayerInput::default(); }
        self.prediction_streak[idx] += 1;

        let mut predicted = self.last_confirmed[idx];
        // After many frames without confirmation, assume neutral input
        if self.prediction_streak[idx] > 6 {
            predicted.buttons = 0;
            predicted.axis_x  = 0;
            predicted.axis_y  = 0;
        }
        predicted.frame = frame;
        predicted.buttons_pressed = 0;  // don't re-trigger presses
        predicted.buttons_released = 0;
        predicted
    }

    pub fn prediction_error(&self, player: PlayerId, actual: &PlayerInput) -> bool {
        let idx = player as usize;
        if idx >= MAX_PLAYERS { return false; }
        let predicted = self.last_confirmed[idx];
        predicted.buttons != actual.buttons || predicted.axis_x != actual.axis_x
    }

    pub fn streak(&self, player: PlayerId) -> u32 {
        self.prediction_streak.get(player as usize).copied().unwrap_or(0)
    }
}

// ── GameState trait ───────────────────────────────────────────────────────────

/// Implement this on your game state to enable rollback.
pub trait GameState: Clone + Send + 'static {
    /// Advance simulation by one frame using the given inputs.
    fn advance(&mut self, inputs: &FrameInput);

    /// Compute a checksum of the game state (for desync detection).
    fn checksum(&self) -> u64;

    /// Memory size hint for the snapshot (used for buffer sizing).
    fn snapshot_size_hint() -> usize { 4096 }
}

// ── StateSnapshot ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct StateSnapshot<S: GameState> {
    pub frame:    Frame,
    pub state:    S,
    pub checksum: u64,
}

impl<S: GameState> StateSnapshot<S> {
    pub fn capture(frame: Frame, state: &S) -> Self {
        let cs = state.checksum();
        Self { frame, state: state.clone(), checksum: cs }
    }
}

// ── RollbackBuffer ────────────────────────────────────────────────────────────

/// Ring buffer of state snapshots and frame inputs for rollback.
pub struct RollbackBuffer<S: GameState> {
    snapshots:   VecDeque<StateSnapshot<S>>,
    frame_inputs: VecDeque<FrameInput>,
    capacity:    usize,
}

impl<S: GameState> RollbackBuffer<S> {
    pub fn new(capacity: usize) -> Self {
        Self {
            snapshots: VecDeque::with_capacity(capacity),
            frame_inputs: VecDeque::with_capacity(capacity * 2),
            capacity,
        }
    }

    pub fn save_snapshot(&mut self, frame: Frame, state: &S) {
        let snapshot = StateSnapshot::capture(frame, state);
        if self.snapshots.len() >= self.capacity {
            self.snapshots.pop_front();
        }
        self.snapshots.push_back(snapshot);
    }

    pub fn save_inputs(&mut self, inputs: FrameInput) {
        if self.frame_inputs.len() >= self.capacity * 2 {
            self.frame_inputs.pop_front();
        }
        self.frame_inputs.push_back(inputs);
    }

    pub fn get_snapshot(&self, frame: Frame) -> Option<&StateSnapshot<S>> {
        self.snapshots.iter().rfind(|s| s.frame == frame)
    }

    pub fn latest_snapshot(&self) -> Option<&StateSnapshot<S>> {
        self.snapshots.back()
    }

    pub fn get_inputs_from(&self, start_frame: Frame) -> Vec<&FrameInput> {
        self.frame_inputs.iter()
            .filter(|fi| fi.frame >= start_frame)
            .collect()
    }

    pub fn oldest_snapshot_frame(&self) -> Option<Frame> {
        self.snapshots.front().map(|s| s.frame)
    }

    pub fn len(&self) -> usize { self.snapshots.len() }
}

// ── DesyncDetector ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DesyncEvent {
    pub frame:       Frame,
    pub local_checksum:  u64,
    pub remote_checksum: u64,
    pub player_id:   PlayerId,
}

pub struct DesyncDetector {
    local_checksums:  HashMap<Frame, u64>,
    remote_checksums: HashMap<(Frame, PlayerId), u64>,
    desyncs:          Vec<DesyncEvent>,
    check_interval:   u32,
}

impl DesyncDetector {
    pub fn new(check_interval: u32) -> Self {
        Self {
            local_checksums: HashMap::new(),
            remote_checksums: HashMap::new(),
            desyncs: Vec::new(),
            check_interval,
        }
    }

    pub fn record_local(&mut self, frame: Frame, checksum: u64) {
        self.local_checksums.insert(frame, checksum);
    }

    pub fn record_remote(&mut self, frame: Frame, player: PlayerId, checksum: u64) {
        self.remote_checksums.insert((frame, player), checksum);
        // Check for desync
        if let Some(&local) = self.local_checksums.get(&frame) {
            if local != checksum {
                self.desyncs.push(DesyncEvent { frame, local_checksum: local, remote_checksum: checksum, player_id: player });
            }
        }
    }

    pub fn has_desync(&self) -> bool { !self.desyncs.is_empty() }

    pub fn drain_desyncs(&mut self) -> Vec<DesyncEvent> {
        std::mem::take(&mut self.desyncs)
    }

    pub fn cleanup_old(&mut self, oldest_frame: Frame) {
        self.local_checksums.retain(|&f, _| f >= oldest_frame);
        self.remote_checksums.retain(|(f, _), _| *f >= oldest_frame);
    }
}

// ── NetworkStats ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PeerStats {
    pub player_id:        PlayerId,
    pub rtt_ms:           f32,
    pub rtt_variance_ms:  f32,
    pub packet_loss_pct:  f32,
    pub frames_ahead:     i32,   // positive = remote is ahead
    pub last_recv_frame:  Frame,
    pub predicted_frames: u32,
}

pub struct NetworkStats {
    peers:           HashMap<PlayerId, PeerStats>,
    local_frame:     Frame,
    rtt_samples:     VecDeque<f32>,
    max_rtt_samples: usize,
}

impl NetworkStats {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
            local_frame: 0,
            rtt_samples: VecDeque::with_capacity(64),
            max_rtt_samples: 64,
        }
    }

    pub fn record_rtt(&mut self, player: PlayerId, rtt_ms: f32) {
        if self.rtt_samples.len() >= self.max_rtt_samples {
            self.rtt_samples.pop_front();
        }
        self.rtt_samples.push_back(rtt_ms);
        let entry = self.peers.entry(player).or_insert_with(|| PeerStats {
            player_id: player, rtt_ms: 0.0, rtt_variance_ms: 0.0,
            packet_loss_pct: 0.0, frames_ahead: 0, last_recv_frame: 0,
            predicted_frames: 0,
        });
        let sum: f32 = self.rtt_samples.iter().sum();
        entry.rtt_ms = sum / self.rtt_samples.len() as f32;
        let variance: f32 = self.rtt_samples.iter()
            .map(|&r| (r - entry.rtt_ms).powi(2))
            .sum::<f32>() / self.rtt_samples.len() as f32;
        entry.rtt_variance_ms = variance.sqrt();
    }

    pub fn recommended_input_delay(&self) -> usize {
        let max_rtt = self.peers.values()
            .map(|p| p.rtt_ms)
            .fold(0.0f32, f32::max);
        let frames_per_ms = 1000.0 / 60.0;  // assuming 60fps
        let delay = (max_rtt / (2.0 * frames_per_ms)).ceil() as usize;
        delay.clamp(1, 6)
    }

    pub fn peer(&self, player: PlayerId) -> Option<&PeerStats> {
        self.peers.get(&player)
    }

    pub fn average_rtt(&self) -> f32 {
        if self.rtt_samples.is_empty() { return 0.0; }
        self.rtt_samples.iter().sum::<f32>() / self.rtt_samples.len() as f32
    }

    pub fn update_frame(&mut self, frame: Frame) { self.local_frame = frame; }
}

// ── RollbackSession ────────────────────────────────────────────────────────────

/// Orchestrates frame advance, rollback, and re-simulation.
pub struct RollbackSession<S: GameState> {
    pub current_frame:    Frame,
    pub confirmed_frame:  Frame,
    pub buffer:           RollbackBuffer<S>,
    pub predictor:        InputPredictor,
    pub desync_detector:  DesyncDetector,
    pub net_stats:        NetworkStats,
    pub player_count:     u8,
    pub local_player_id:  PlayerId,
    pub input_delay:      usize,
    local_input_queue:    VecDeque<PlayerInput>,
    pending_remote:       HashMap<(Frame, PlayerId), PlayerInput>,
    rollback_count:       u64,
}

impl<S: GameState> RollbackSession<S> {
    pub fn new(player_count: u8, local_player_id: PlayerId) -> Self {
        Self {
            current_frame: 0,
            confirmed_frame: 0,
            buffer: RollbackBuffer::new(MAX_ROLLBACK_FRAMES * 4),
            predictor: InputPredictor::new(),
            desync_detector: DesyncDetector::new(8),
            net_stats: NetworkStats::new(),
            player_count,
            local_player_id,
            input_delay: INPUT_DELAY,
            local_input_queue: VecDeque::new(),
            pending_remote: HashMap::new(),
            rollback_count: 0,
        }
    }

    /// Queue local player's input for a future frame (after input delay).
    pub fn queue_local_input(&mut self, input: PlayerInput) {
        self.local_input_queue.push_back(input);
    }

    /// Receive a remote player's confirmed input.
    pub fn receive_remote_input(&mut self, player: PlayerId, frame: Frame, input: PlayerInput) {
        self.pending_remote.insert((frame, player), input);
        self.predictor.confirm_input(player, input);
    }

    /// Build a FrameInput for the current frame, predicting any missing remotes.
    pub fn build_frame_input(&mut self, state: &S) -> FrameInput {
        let frame = self.current_frame;
        let mut fi = FrameInput::new(frame);

        // Local input (with delay)
        let local_input = self.local_input_queue.pop_front().unwrap_or_default();
        fi.set_input(self.local_player_id, local_input);

        // Remote inputs
        for player in 0..self.player_count {
            if player == self.local_player_id { continue; }
            let key = (frame, player);
            let input = if let Some(&remote) = self.pending_remote.get(&key) {
                self.pending_remote.remove(&key);
                fi.confirmed[player as usize] = true;
                remote
            } else {
                self.predictor.predict(player, frame)
            };
            fi.inputs[player as usize] = input;
        }

        // Record checksum for desync detection
        let cs = state.checksum();
        self.desync_detector.record_local(frame, cs);

        fi
    }

    /// Advance one frame.
    pub fn advance(&mut self, state: &mut S) -> FrameInput {
        // Save snapshot before advancing
        self.buffer.save_snapshot(self.current_frame, state);

        let fi = self.build_frame_input(state);
        self.buffer.save_inputs(fi.clone());

        state.advance(&fi);
        self.net_stats.update_frame(self.current_frame);
        self.current_frame += 1;
        fi
    }

    /// Check if rollback is needed due to late-arriving remote inputs.
    /// Returns the frame to roll back to, if any.
    pub fn check_rollback(&mut self) -> Option<Frame> {
        let earliest_incorrect = self.pending_remote.keys()
            .filter(|(frame, _)| *frame < self.current_frame)
            .map(|(frame, _)| *frame)
            .min()?;

        if earliest_incorrect < self.current_frame {
            Some(earliest_incorrect)
        } else {
            None
        }
    }

    /// Perform rollback and re-simulation to a specific frame.
    /// Returns the new current state after re-simulation.
    pub fn rollback_to(&mut self, target_frame: Frame, state: &mut S) -> bool {
        let snapshot = match self.buffer.get_snapshot(target_frame) {
            Some(s) => s.clone(),
            None    => return false,
        };

        *state = snapshot.state;
        let resim_start = target_frame;

        // Collect inputs from target frame onward
        let inputs: Vec<FrameInput> = self.buffer
            .get_inputs_from(resim_start)
            .iter()
            .map(|fi| (*fi).clone())
            .collect();

        // Update any confirmed remote inputs in those frames
        let inputs_len = inputs.len();
        for mut fi in inputs {
            for player in 0..self.player_count {
                let key = (fi.frame, player);
                if let Some(&confirmed) = self.pending_remote.get(&key) {
                    fi.inputs[player as usize] = confirmed;
                    fi.confirmed[player as usize] = true;
                    self.pending_remote.remove(&key);
                    self.predictor.confirm_input(player, confirmed);
                }
            }
            state.advance(&fi);
        }

        self.current_frame = target_frame + inputs_len as Frame;
        self.rollback_count += 1;
        true
    }

    pub fn rollback_count(&self) -> u64 { self.rollback_count }
    pub fn frames_behind(&self) -> u64 {
        self.current_frame.saturating_sub(self.confirmed_frame)
    }
}

// ── InputPacket (wire format) ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InputPacket {
    pub from_player: PlayerId,
    pub frame:       Frame,
    pub inputs:      Vec<(Frame, PlayerInput)>,  // (frame, input) pairs
    pub checksum:    u32,
    pub ack_frame:   Frame,  // highest frame we've confirmed from them
}

impl InputPacket {
    pub fn new(player: PlayerId, frame: Frame) -> Self {
        Self { from_player: player, frame, inputs: Vec::new(), checksum: 0, ack_frame: 0 }
    }

    pub fn add_input(&mut self, frame: Frame, input: PlayerInput) {
        self.inputs.push((frame, input));
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.from_player);
        buf.extend_from_slice(&self.frame.to_le_bytes());
        buf.extend_from_slice(&self.ack_frame.to_le_bytes());
        buf.push(self.inputs.len() as u8);
        for (frame, input) in &self.inputs {
            buf.extend_from_slice(&frame.to_le_bytes());
            buf.extend_from_slice(&input.to_bytes());
        }
        buf.extend_from_slice(&self.checksum.to_le_bytes());
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 18 { return None; }
        let from_player = bytes[0];
        let frame = Frame::from_le_bytes(bytes[1..9].try_into().ok()?);
        let ack_frame = Frame::from_le_bytes(bytes[9..17].try_into().ok()?);
        let count = bytes[17] as usize;
        let mut inputs = Vec::new();
        let mut offset = 18;
        for _ in 0..count {
            if offset + 24 > bytes.len() { break; }
            let f = Frame::from_le_bytes(bytes[offset..offset+8].try_into().ok()?);
            let inp_bytes: &[u8; 16] = bytes[offset+8..offset+24].try_into().ok()?;
            let inp = PlayerInput::from_bytes(inp_bytes);
            inputs.push((f, inp));
            offset += 24;
        }
        let checksum = if offset + 4 <= bytes.len() {
            u32::from_le_bytes(bytes[offset..offset+4].try_into().unwrap_or([0;4]))
        } else { 0 };
        Some(Self { from_player, frame, inputs, checksum, ack_frame })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestState {
        frame: Frame,
        value: i64,
    }

    impl GameState for TestState {
        fn advance(&mut self, inputs: &FrameInput) {
            self.frame += 1;
            if inputs.inputs[0].is_held(1) { self.value += 1; }
        }
        fn checksum(&self) -> u64 { self.value as u64 ^ (self.frame << 32) }
    }

    #[test]
    fn test_player_input_roundtrip() {
        let inp = PlayerInput { buttons: 0b1010, axis_x: 1000, axis_y: -500, frame: 42, ..Default::default() };
        let bytes = inp.to_bytes();
        let rt = PlayerInput::from_bytes(&bytes);
        assert_eq!(rt.buttons, inp.buttons);
        assert_eq!(rt.axis_x, inp.axis_x);
        assert_eq!(rt.axis_y, inp.axis_y);
    }

    #[test]
    fn test_predictor_streak() {
        let mut pred = InputPredictor::new();
        let inp = PlayerInput { buttons: 0b0001, frame: 5, ..Default::default() };
        pred.confirm_input(1, inp);
        let p = pred.predict(1, 6);
        assert_eq!(p.buttons, 0b0001);
        assert_eq!(pred.streak(1), 1);
        // After many predictions, should go neutral
        for i in 7..20 { pred.predict(1, i); }
        let neutral = pred.predict(1, 20);
        assert_eq!(neutral.buttons, 0);
    }

    #[test]
    fn test_rollback_session_advance() {
        let mut session: RollbackSession<TestState> = RollbackSession::new(1, 0);
        let mut state = TestState { frame: 0, value: 0 };

        // Queue inputs
        for _ in 0..5 {
            session.queue_local_input(PlayerInput { buttons: 1, ..Default::default() });
        }
        for _ in 0..5 {
            session.advance(&mut state);
        }
        assert_eq!(session.current_frame, 5);
    }

    #[test]
    fn test_network_stats_rtt() {
        let mut stats = NetworkStats::new();
        for rtt in [20.0, 24.0, 22.0, 18.0, 21.0] {
            stats.record_rtt(0, rtt);
        }
        let avg = stats.average_rtt();
        assert!(avg > 18.0 && avg < 25.0);
    }

    #[test]
    fn test_input_packet_roundtrip() {
        let mut pkt = InputPacket::new(0, 100);
        pkt.add_input(100, PlayerInput { buttons: 3, ..Default::default() });
        let bytes = pkt.to_bytes();
        let rt = InputPacket::from_bytes(&bytes).unwrap();
        assert_eq!(rt.from_player, 0);
        assert_eq!(rt.frame, 100);
        assert_eq!(rt.inputs.len(), 1);
    }

    #[test]
    fn test_desync_detector() {
        let mut dd = DesyncDetector::new(4);
        dd.record_local(10, 0xABCDEF);
        dd.record_remote(10, 1, 0xABCDEF);
        assert!(!dd.has_desync());
        dd.record_remote(10, 1, 0x000000);  // wrong!
        // checksum was already recorded wrong from first record
        // Try a new frame
        dd.record_local(11, 0x111111);
        dd.record_remote(11, 1, 0x222222);
        assert!(dd.has_desync());
    }
}
