//! WebSocket client with auto-reconnect, message queueing, and channel mux.
//!
//! Driven by `tick(dt)` each frame. Messages arrive as `WsEvent` values
//! polled from `drain_events()`. Send messages with `send()`.
//!
//! Auto-reconnect: exponential backoff on disconnect.
//! Message queue: outgoing messages buffered during disconnection.
//! Channel mux: multiple logical channels on one connection.

use std::collections::{VecDeque, HashMap};
use std::time::{Duration, Instant};

// ── WsMessage ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum WsMessage {
    Text(String),
    Binary(Vec<u8>),
    /// Close with optional code and reason.
    Close { code: u16, reason: String },
    /// Ping with payload.
    Ping(Vec<u8>),
    /// Pong response to a Ping.
    Pong(Vec<u8>),
}

impl WsMessage {
    pub fn text(s: impl Into<String>) -> Self { Self::Text(s.into()) }
    pub fn binary(v: Vec<u8>) -> Self { Self::Binary(v) }
    pub fn close_normal() -> Self { Self::Close { code: 1000, reason: "Normal closure".into() } }

    pub fn is_data(&self) -> bool {
        matches!(self, Self::Text(_) | Self::Binary(_))
    }

    pub fn as_text(&self) -> Option<&str> {
        if let Self::Text(s) = self { Some(s) } else { None }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Text(s)   => s.len(),
            Self::Binary(v) => v.len(),
            _               => 0,
        }
    }

    pub fn is_empty(&self) -> bool { self.len() == 0 }
}

// ── WsState ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsState {
    /// Not connected. Awaiting connect() or auto-reconnect.
    Disconnected,
    /// TCP connection in progress.
    Connecting,
    /// WebSocket handshake in progress.
    Handshaking,
    /// Fully connected and ready.
    Connected,
    /// Waiting before reconnect attempt (backoff).
    ReconnectBackoff,
    /// Intentionally closed, will not reconnect.
    Closed,
}

impl WsState {
    pub fn is_connected(self) -> bool { self == Self::Connected }
    pub fn is_live(self) -> bool { matches!(self, Self::Connected | Self::Handshaking) }
}

// ── WsEvent ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum WsEvent {
    /// Successfully connected and handshaked.
    Connected { url: String },
    /// Connection closed by server or error.
    Disconnected { url: String, code: u16, reason: String, will_reconnect: bool },
    /// Received a message from the server.
    Message { message: WsMessage, channel: Option<String> },
    /// Reconnect attempt starting.
    Reconnecting { url: String, attempt: u32, backoff_ms: u64 },
    /// Error during connection or message.
    Error { description: String },
    /// Ping round-trip time measured.
    PingRtt { millis: f64 },
}

// ── ChannelMessage ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct OutboundMessage {
    message: WsMessage,
    channel: Option<String>,
    queued_at: Instant,
}

// ── WsClient ─────────────────────────────────────────────────────────────────

/// WebSocket client driven by `tick()`.
pub struct WsClient {
    pub url:             String,
    pub state:           WsState,
    /// Auto-reconnect on unexpected disconnect.
    pub auto_reconnect:  bool,
    /// Max reconnect attempts before giving up (0 = unlimited).
    pub max_reconnects:  u32,
    /// Keepalive ping interval.
    pub ping_interval:   Duration,
    /// Close connection if no pong received within this time.
    pub pong_timeout:    Duration,
    /// Max size of outbound queue.
    pub max_queue_size:  usize,

    reconnect_attempt:   u32,
    reconnect_timer:     f32,
    reconnect_backoff:   f32,
    last_ping:           Option<Instant>,
    last_pong:           Option<Instant>,
    ping_payload:        Vec<u8>,
    outbound:            VecDeque<OutboundMessage>,
    events:              VecDeque<WsEvent>,
    /// Logical channels: name -> subscription filter
    channels:            HashMap<String, ChannelConfig>,
    connect_time:        Option<Instant>,
    messages_sent:       u64,
    messages_received:   u64,
    bytes_sent:          u64,
    bytes_received:      u64,
    /// Simulated state (real impl would use TCP stream).
    sim_connected_at:    Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct ChannelConfig {
    pub name:    String,
    pub filter:  Option<String>,
    pub active:  bool,
}

impl WsClient {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url:               url.into(),
            state:             WsState::Disconnected,
            auto_reconnect:    true,
            max_reconnects:    10,
            ping_interval:     Duration::from_secs(30),
            pong_timeout:      Duration::from_secs(10),
            max_queue_size:    1024,
            reconnect_attempt: 0,
            reconnect_timer:   0.0,
            reconnect_backoff: 1.0,
            last_ping:         None,
            last_pong:         None,
            ping_payload:      vec![1, 2, 3, 4],
            outbound:          VecDeque::new(),
            events:            VecDeque::new(),
            channels:          HashMap::new(),
            connect_time:      None,
            messages_sent:     0,
            messages_received: 0,
            bytes_sent:        0,
            bytes_received:    0,
            sim_connected_at:  None,
        }
    }

    // ── Control ───────────────────────────────────────────────────────────────

    /// Initiate a connection. No-op if already connected.
    pub fn connect(&mut self) {
        if matches!(self.state, WsState::Disconnected | WsState::Closed) {
            self.state = WsState::Connecting;
            self.reconnect_attempt = 0;
        }
    }

    /// Close the connection intentionally (will not auto-reconnect).
    pub fn close(&mut self) {
        if self.state != WsState::Closed {
            self.send_raw(WsMessage::close_normal(), None);
            self.state = WsState::Closed;
        }
    }

    /// Disconnect but allow auto-reconnect.
    pub fn disconnect(&mut self) {
        self.state = WsState::Disconnected;
        self.sim_connected_at = None;
    }

    // ── Sending ───────────────────────────────────────────────────────────────

    /// Send a message. Queued if not currently connected.
    pub fn send(&mut self, message: WsMessage) -> bool {
        self.send_raw(message, None)
    }

    /// Send a message on a named channel.
    pub fn send_on_channel(&mut self, channel: &str, message: WsMessage) -> bool {
        self.send_raw(message, Some(channel.to_owned()))
    }

    fn send_raw(&mut self, message: WsMessage, channel: Option<String>) -> bool {
        if self.outbound.len() >= self.max_queue_size {
            self.events.push_back(WsEvent::Error {
                description: "Outbound queue full, message dropped".into(),
            });
            return false;
        }
        let len = message.len();
        self.outbound.push_back(OutboundMessage {
            message,
            channel,
            queued_at: Instant::now(),
        });
        self.bytes_sent += len as u64;
        true
    }

    // ── Channels ──────────────────────────────────────────────────────────────

    /// Subscribe to a named logical channel.
    pub fn subscribe(&mut self, channel: impl Into<String>) {
        let name = channel.into();
        self.channels.insert(name.clone(), ChannelConfig {
            name,
            filter: None,
            active: true,
        });
    }

    pub fn unsubscribe(&mut self, channel: &str) {
        self.channels.remove(channel);
    }

    // ── Tick ──────────────────────────────────────────────────────────────────

    /// Drive the WebSocket state machine. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        match self.state {
            WsState::Disconnected => {
                if self.auto_reconnect
                    && (self.max_reconnects == 0 || self.reconnect_attempt < self.max_reconnects)
                {
                    self.reconnect_timer -= dt;
                    if self.reconnect_timer <= 0.0 {
                        self.state = WsState::Connecting;
                        self.events.push_back(WsEvent::Reconnecting {
                            url: self.url.clone(),
                            attempt: self.reconnect_attempt,
                            backoff_ms: (self.reconnect_backoff * 1000.0) as u64,
                        });
                    }
                }
            }

            WsState::Connecting => {
                // Simulate connection establishment
                self.state = WsState::Handshaking;
                self.sim_connected_at = Some(Instant::now());
            }

            WsState::Handshaking => {
                // Simulate handshake completion after brief delay
                if let Some(t) = self.sim_connected_at {
                    if t.elapsed() >= Duration::from_millis(10) {
                        self.state = WsState::Connected;
                        self.connect_time = Some(Instant::now());
                        self.reconnect_attempt = 0;
                        self.reconnect_backoff = 1.0;
                        self.events.push_back(WsEvent::Connected { url: self.url.clone() });
                    }
                }
            }

            WsState::Connected => {
                // Flush outbound queue
                while let Some(msg) = self.outbound.pop_front() {
                    self.messages_sent += 1;
                    // In the real impl, write to the TCP stream here
                }

                // Keepalive ping
                let should_ping = match self.last_ping {
                    None    => true,
                    Some(t) => t.elapsed() >= self.ping_interval,
                };
                if should_ping {
                    self.send_raw(WsMessage::Ping(self.ping_payload.clone()), None);
                    self.last_ping = Some(Instant::now());
                }

                // Pong timeout check
                if let (Some(ping_t), None) = (self.last_ping, self.last_pong) {
                    if ping_t.elapsed() > self.pong_timeout {
                        self.handle_disconnect(1001, "Pong timeout".into());
                    }
                }
            }

            WsState::ReconnectBackoff => {
                self.reconnect_timer -= dt;
                if self.reconnect_timer <= 0.0 {
                    self.state = WsState::Connecting;
                }
            }

            WsState::Closed => {}
        }
    }

    fn handle_disconnect(&mut self, code: u16, reason: String) {
        let will_reconnect = self.auto_reconnect
            && (self.max_reconnects == 0 || self.reconnect_attempt < self.max_reconnects);

        self.events.push_back(WsEvent::Disconnected {
            url: self.url.clone(),
            code,
            reason,
            will_reconnect,
        });

        if will_reconnect {
            self.reconnect_attempt += 1;
            // Exponential backoff with cap at 60s
            self.reconnect_backoff = (self.reconnect_backoff * 2.0).min(60.0);
            self.reconnect_timer = self.reconnect_backoff;
            self.state = WsState::ReconnectBackoff;
        } else {
            self.state = WsState::Closed;
        }
        self.sim_connected_at = None;
    }

    // ── Stats ─────────────────────────────────────────────────────────────────

    pub fn drain_events(&mut self) -> impl Iterator<Item = WsEvent> + '_ {
        self.events.drain(..)
    }

    pub fn is_connected(&self) -> bool { self.state.is_connected() }
    pub fn messages_sent(&self) -> u64 { self.messages_sent }
    pub fn messages_received(&self) -> u64 { self.messages_received }
    pub fn bytes_sent(&self) -> u64 { self.bytes_sent }
    pub fn bytes_received(&self) -> u64 { self.bytes_received }
    pub fn uptime(&self) -> Option<Duration> { self.connect_time.map(|t| t.elapsed()) }
    pub fn pending_outbound(&self) -> usize { self.outbound.len() }
}
