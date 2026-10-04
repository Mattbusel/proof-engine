//! WebSocket client with auto-reconnect, message queueing, and channel mux.
//!
//! Driven by `tick(dt)` each frame. Messages arrive as `WsEvent` values
//! polled from `drain_events()`. Send messages with `send()`.
//!
//! Needs the `websocket` cargo feature for a real connection (tungstenite,
//! rustls for `wss://`).
//!
//! Auto-reconnect: exponential backoff on disconnect.
//! Message queue: outgoing messages buffered during disconnection.
//! Channels: local labels on outgoing messages (not sent on the wire).

#![warn(missing_docs)]

use std::collections::{VecDeque, HashMap};
use std::time::{Duration, Instant};

// ── WsMessage ─────────────────────────────────────────────────────────────────

/// A WebSocket message.
#[derive(Debug, Clone)]
pub enum WsMessage {
    /// UTF-8 text frame.
    Text(String),
    /// Binary frame.
    Binary(Vec<u8>),
    /// Close with optional code and reason.
    Close {
        /// Close status code (1000 is a normal closure).
        code: u16,
        /// Human-readable reason.
        reason: String,
    },
    /// Ping with payload.
    Ping(Vec<u8>),
    /// Pong response to a Ping.
    Pong(Vec<u8>),
}

impl WsMessage {
    /// A text message.
    pub fn text(s: impl Into<String>) -> Self { Self::Text(s.into()) }
    /// A binary message.
    pub fn binary(v: Vec<u8>) -> Self { Self::Binary(v) }
    /// A close message with code 1000 (normal closure).
    pub fn close_normal() -> Self { Self::Close { code: 1000, reason: "Normal closure".into() } }

    /// True for text and binary messages (not control frames).
    pub fn is_data(&self) -> bool {
        matches!(self, Self::Text(_) | Self::Binary(_))
    }

    /// The text, if this is a text message.
    pub fn as_text(&self) -> Option<&str> {
        if let Self::Text(s) = self { Some(s) } else { None }
    }

    /// Payload length in bytes for text and binary messages; 0 for control frames.
    pub fn len(&self) -> usize {
        match self {
            Self::Text(s)   => s.len(),
            Self::Binary(v) => v.len(),
            _               => 0,
        }
    }

    /// True if `len()` is 0.
    pub fn is_empty(&self) -> bool { self.len() == 0 }
}

// ── WsState ───────────────────────────────────────────────────────────────────

/// Connection state of a `WsClient`.
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
    /// True when `Connected`.
    pub fn is_connected(self) -> bool { self == Self::Connected }
    /// True when `Connected` or `Handshaking`.
    pub fn is_live(self) -> bool { matches!(self, Self::Connected | Self::Handshaking) }
}

// ── WsEvent ───────────────────────────────────────────────────────────────────

/// Something that happened on the connection, from `WsClient::drain_events`.
#[derive(Debug, Clone)]
pub enum WsEvent {
    /// Successfully connected and handshaked.
    Connected {
        /// The server URL.
        url: String,
    },
    /// Connection closed by server or error.
    Disconnected {
        /// The server URL.
        url: String,
        /// Close code (1006 when the connection failed without a close frame).
        code: u16,
        /// Close reason or error text.
        reason: String,
        /// Whether the client will try to reconnect.
        will_reconnect: bool,
    },
    /// Received a message from the server.
    Message {
        /// The message.
        message: WsMessage,
        /// Always `None` for received messages: channels are local labels only.
        channel: Option<String>,
    },
    /// Reconnect attempt starting.
    Reconnecting {
        /// The server URL.
        url: String,
        /// Which reconnect attempt this is.
        attempt: u32,
        /// Delay used before the next attempt if this one fails, in milliseconds.
        backoff_ms: u64,
    },
    /// Error during connection or message.
    Error {
        /// What went wrong.
        description: String,
    },
    /// Ping round-trip time measured.
    PingRtt {
        /// Round-trip time in milliseconds.
        millis: f64,
    },
}

// ── ChannelMessage ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct OutboundMessage {
    message: WsMessage,
    channel: Option<String>,
    queued_at: Instant,
}

// ── WsClient ─────────────────────────────────────────────────────────────────

/// What the connection thread reports to the client.
enum WorkerEvent {
    Connected,
    Message(WsMessage),
    Pong,
    Closed { code: u16, reason: String },
    Failed(String),
}

/// Handle to the background thread that owns the socket.
struct Worker {
    outbound: std::sync::mpsc::Sender<WsMessage>,
    inbound: std::sync::mpsc::Receiver<WorkerEvent>,
}

/// WebSocket client driven by `tick()`.
///
/// With the `websocket` feature the connection is real: a background thread
/// runs [`tungstenite`](https://crates.io/crates/tungstenite) (`ws://`, and
/// `wss://` through rustls) and `tick()` exchanges messages with it without
/// blocking. Without the feature, `connect` reports an error event and the
/// client stays disconnected; before 0.3.0 it pretended to connect to any URL
/// and silently dropped every message.
///
/// Channel names given to `send_on_channel` and `subscribe` are local labels
/// for your own bookkeeping; they are not sent on the wire.
pub struct WsClient {
    /// Server URL, `ws://` or `wss://`.
    pub url:             String,
    /// Current connection state.
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
    awaiting_pong:       bool,
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
    worker:              Option<Worker>,
}

/// A logical channel registered with [`WsClient::subscribe`].
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    /// Channel name.
    pub name:    String,
    /// Optional filter string (application defined).
    pub filter:  Option<String>,
    /// Whether the channel is active.
    pub active:  bool,
}

impl WsClient {
    /// A disconnected client for `url`. Call [`WsClient::connect`] to start.
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
            awaiting_pong:     false,
            ping_payload:      vec![1, 2, 3, 4],
            outbound:          VecDeque::new(),
            events:            VecDeque::new(),
            channels:          HashMap::new(),
            connect_time:      None,
            messages_sent:     0,
            messages_received: 0,
            bytes_sent:        0,
            bytes_received:    0,
            worker:            None,
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
            if let Some(w) = &self.worker {
                let _ = w.outbound.send(WsMessage::close_normal());
            }
            self.worker = None;
            self.state = WsState::Closed;
        }
    }

    /// Drop the connection; it reconnects on the next tick if
    /// `auto_reconnect` is on.
    pub fn disconnect(&mut self) {
        self.worker = None;
        self.state = WsState::Disconnected;
        self.reconnect_attempt = self.reconnect_attempt.max(1);
        self.reconnect_timer = 0.0;
    }

    // ── Sending ───────────────────────────────────────────────────────────────

    /// Send a message. Queued if not currently connected.
    pub fn send(&mut self, message: WsMessage) -> bool {
        self.send_raw(message, None)
    }

    /// Send a message labelled with a local channel name.
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
        self.outbound.push_back(OutboundMessage {
            message,
            channel,
            queued_at: Instant::now(),
        });
        true
    }

    // ── Channels ──────────────────────────────────────────────────────────────

    /// Register a named logical channel.
    pub fn subscribe(&mut self, channel: impl Into<String>) {
        let name = channel.into();
        self.channels.insert(name.clone(), ChannelConfig {
            name,
            filter: None,
            active: true,
        });
    }

    /// Remove a logical channel.
    pub fn unsubscribe(&mut self, channel: &str) {
        self.channels.remove(channel);
    }

    // ── Tick ──────────────────────────────────────────────────────────────────

    /// Drive the WebSocket state machine. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        match self.state {
            WsState::Disconnected => {
                if self.auto_reconnect
                    && self.reconnect_attempt > 0
                    && (self.max_reconnects == 0 || self.reconnect_attempt < self.max_reconnects)
                {
                    self.reconnect_timer -= dt;
                    if self.reconnect_timer <= 0.0 {
                        self.state = WsState::Connecting;
                    }
                }
            }

            WsState::Connecting => match spawn_worker(&self.url) {
                Ok(worker) => {
                    self.worker = Some(worker);
                    self.state = WsState::Handshaking;
                }
                Err(description) => {
                    self.events.push_back(WsEvent::Error { description: description.clone() });
                    if cfg!(feature = "websocket") {
                        self.handle_disconnect(1006, description);
                    } else {
                        // No backend: retrying cannot help.
                        self.state = WsState::Closed;
                    }
                }
            },

            WsState::Handshaking | WsState::Connected => {
                self.poll_worker();
                if self.state != WsState::Connected {
                    return;
                }
                // Flush the outbound queue to the socket thread.
                if let Some(w) = &self.worker {
                    while let Some(msg) = self.outbound.pop_front() {
                        let len = msg.message.len() as u64;
                        if w.outbound.send(msg.message).is_err() {
                            break;
                        }
                        self.messages_sent += 1;
                        self.bytes_sent += len;
                    }
                }
                // Keepalive ping, and a disconnect if the pong does not come.
                let due = self.last_ping.is_none_or(|t| t.elapsed() >= self.ping_interval);
                if due && !self.awaiting_pong {
                    if let Some(w) = &self.worker {
                        let _ = w.outbound.send(WsMessage::Ping(self.ping_payload.clone()));
                    }
                    self.last_ping = Some(Instant::now());
                    self.awaiting_pong = true;
                }
                if self.awaiting_pong
                    && self.last_ping.is_some_and(|t| t.elapsed() > self.pong_timeout)
                {
                    self.handle_disconnect(1001, "Pong timeout".into());
                }
            }

            WsState::ReconnectBackoff => {
                self.reconnect_timer -= dt;
                if self.reconnect_timer <= 0.0 {
                    self.events.push_back(WsEvent::Reconnecting {
                        url: self.url.clone(),
                        attempt: self.reconnect_attempt,
                        backoff_ms: (self.reconnect_backoff * 1000.0) as u64,
                    });
                    self.state = WsState::Connecting;
                }
            }

            WsState::Closed => {}
        }
    }

    fn poll_worker(&mut self) {
        loop {
            let Some(w) = &self.worker else { return };
            let ev = match w.inbound.try_recv() {
                Ok(ev) => ev,
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    WorkerEvent::Failed("connection thread ended".into())
                }
            };
            match ev {
                WorkerEvent::Connected => {
                    self.state = WsState::Connected;
                    self.connect_time = Some(Instant::now());
                    self.reconnect_attempt = 0;
                    self.reconnect_backoff = 1.0;
                    self.last_ping = Some(Instant::now());
                    self.awaiting_pong = false;
                    self.events.push_back(WsEvent::Connected { url: self.url.clone() });
                }
                WorkerEvent::Message(message) => {
                    self.messages_received += 1;
                    self.bytes_received += message.len() as u64;
                    self.events.push_back(WsEvent::Message { message, channel: None });
                }
                WorkerEvent::Pong => {
                    if let Some(t) = self.last_ping {
                        self.events.push_back(WsEvent::PingRtt {
                            millis: t.elapsed().as_secs_f64() * 1000.0,
                        });
                    }
                    self.awaiting_pong = false;
                }
                WorkerEvent::Closed { code, reason } => {
                    self.handle_disconnect(code, reason);
                    return;
                }
                WorkerEvent::Failed(reason) => {
                    self.events.push_back(WsEvent::Error { description: reason.clone() });
                    self.handle_disconnect(1006, reason);
                    return;
                }
            }
        }
    }

    fn handle_disconnect(&mut self, code: u16, reason: String) {
        self.worker = None;
        self.connect_time = None;
        self.awaiting_pong = false;
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
            self.reconnect_timer = self.reconnect_backoff;
            // Exponential backoff with cap at 60s
            self.reconnect_backoff = (self.reconnect_backoff * 2.0).min(60.0);
            self.state = WsState::ReconnectBackoff;
        } else {
            self.state = WsState::Closed;
        }
    }

    // ── Stats ─────────────────────────────────────────────────────────────────

    /// Take all events produced since the last call.
    pub fn drain_events(&mut self) -> impl Iterator<Item = WsEvent> + '_ {
        self.events.drain(..)
    }

    /// Whether the connection is open.
    pub fn is_connected(&self) -> bool { self.state.is_connected() }
    /// Messages handed to the socket so far.
    pub fn messages_sent(&self) -> u64 { self.messages_sent }
    /// Messages received so far.
    pub fn messages_received(&self) -> u64 { self.messages_received }
    /// Payload bytes handed to the socket so far.
    pub fn bytes_sent(&self) -> u64 { self.bytes_sent }
    /// Payload bytes received so far.
    pub fn bytes_received(&self) -> u64 { self.bytes_received }
    /// Time since the current connection opened.
    pub fn uptime(&self) -> Option<Duration> { self.connect_time.map(|t| t.elapsed()) }
    /// Messages waiting to be sent.
    pub fn pending_outbound(&self) -> usize { self.outbound.len() }
}

// ── Connection thread ─────────────────────────────────────────────────────────

#[cfg(feature = "websocket")]
fn spawn_worker(url: &str) -> Result<Worker, String> {
    use std::sync::mpsc::{channel, TryRecvError};
    use tungstenite::{stream::MaybeTlsStream, Message};

    let (out_tx, out_rx) = channel::<WsMessage>();
    let (in_tx, in_rx) = channel::<WorkerEvent>();
    let url = url.to_owned();
    std::thread::Builder::new()
        .name("proof-websocket".into())
        .spawn(move || {
            let (mut socket, _resp) = match tungstenite::connect(url.as_str()) {
                Ok(s) => s,
                Err(e) => {
                    let _ = in_tx.send(WorkerEvent::Failed(format!("connect {url}: {e}")));
                    return;
                }
            };
            // Short read timeout so the thread also gets to send.
            let timeout = Some(Duration::from_millis(15));
            match socket.get_mut() {
                MaybeTlsStream::Plain(s) => { let _ = s.set_read_timeout(timeout); }
                MaybeTlsStream::Rustls(s) => { let _ = s.get_mut().set_read_timeout(timeout); }
                _ => {}
            }
            if in_tx.send(WorkerEvent::Connected).is_err() {
                return;
            }
            loop {
                // Outgoing.
                loop {
                    match out_rx.try_recv() {
                        Ok(msg) => {
                            let m = match msg {
                                WsMessage::Text(t) => Message::text(t),
                                WsMessage::Binary(b) => Message::binary(b),
                                WsMessage::Ping(p) => Message::Ping(p.into()),
                                WsMessage::Pong(p) => Message::Pong(p.into()),
                                WsMessage::Close { code, reason } => {
                                    let _ = socket.close(Some(tungstenite::protocol::CloseFrame {
                                        code: code.into(),
                                        reason: reason.into(),
                                    }));
                                    let _ = socket.flush();
                                    return;
                                }
                            };
                            if let Err(e) = socket.send(m) {
                                let _ = in_tx.send(WorkerEvent::Failed(format!("send: {e}")));
                                return;
                            }
                        }
                        Err(TryRecvError::Empty) => break,
                        // The client was dropped or disconnected: close politely.
                        Err(TryRecvError::Disconnected) => {
                            let _ = socket.close(None);
                            let _ = socket.flush();
                            return;
                        }
                    }
                }
                // Incoming.
                match socket.read() {
                    Ok(Message::Text(t)) => {
                        let _ = in_tx.send(WorkerEvent::Message(WsMessage::Text(t.to_string())));
                    }
                    Ok(Message::Binary(b)) => {
                        let _ = in_tx.send(WorkerEvent::Message(WsMessage::Binary(b.to_vec())));
                    }
                    Ok(Message::Pong(_)) => {
                        let _ = in_tx.send(WorkerEvent::Pong);
                    }
                    Ok(Message::Ping(_)) | Ok(Message::Frame(_)) => {} // tungstenite answers pings
                    Ok(Message::Close(frame)) => {
                        let (code, reason) = frame
                            .map(|f| (u16::from(f.code), f.reason.to_string()))
                            .unwrap_or((1005, String::new()));
                        let _ = in_tx.send(WorkerEvent::Closed { code, reason });
                        return;
                    }
                    Err(tungstenite::Error::Io(e))
                        if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                    Err(tungstenite::Error::ConnectionClosed) | Err(tungstenite::Error::AlreadyClosed) => {
                        let _ = in_tx.send(WorkerEvent::Closed { code: 1000, reason: String::new() });
                        return;
                    }
                    Err(e) => {
                        let _ = in_tx.send(WorkerEvent::Failed(format!("read: {e}")));
                        return;
                    }
                }
            }
        })
        .map_err(|e| format!("could not start the websocket thread: {e}"))?;
    Ok(Worker { outbound: out_tx, inbound: in_rx })
}

#[cfg(not(feature = "websocket"))]
fn spawn_worker(_url: &str) -> Result<Worker, String> {
    Err("proof-engine was built without the `websocket` feature, so it cannot open connections".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pump(c: &mut WsClient, until: impl Fn(&[WsEvent]) -> bool) -> Vec<WsEvent> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut seen = Vec::new();
        while Instant::now() < deadline {
            c.tick(0.016);
            seen.extend(c.drain_events());
            if until(&seen) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        seen
    }

    #[cfg(not(feature = "websocket"))]
    #[test]
    fn without_feature_connect_reports_an_error_instead_of_pretending() {
        let mut c = WsClient::new("ws://127.0.0.1:9/");
        c.connect();
        let ev = pump(&mut c, |e| !e.is_empty());
        assert!(matches!(ev.first(), Some(WsEvent::Error { .. })), "{ev:?}");
        assert_eq!(c.state, WsState::Closed);
        assert!(!c.is_connected());
    }

    #[test]
    fn new_client_does_not_connect_by_itself() {
        let mut c = WsClient::new("ws://127.0.0.1:9/");
        for _ in 0..5 {
            c.tick(1.0);
        }
        assert_eq!(c.state, WsState::Disconnected);
    }

    #[cfg(feature = "websocket")]
    #[test]
    fn echo_round_trip_against_a_local_server() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            loop {
                match ws.read() {
                    Ok(m) if m.is_text() || m.is_binary() => {
                        ws.send(m).unwrap();
                    }
                    Ok(tungstenite::Message::Close(_)) | Err(_) => break,
                    Ok(_) => {}
                }
            }
        });

        let mut c = WsClient::new(format!("ws://{addr}/"));
        c.ping_interval = Duration::from_millis(50);
        c.connect();
        c.send(WsMessage::text("hello"));
        c.send(WsMessage::binary(vec![1, 2, 3]));
        let ev = pump(&mut c, |e| {
            e.iter().filter(|x| matches!(x, WsEvent::Message { .. })).count() >= 2
                && e.iter().any(|x| matches!(x, WsEvent::PingRtt { .. }))
        });
        assert!(matches!(ev.first(), Some(WsEvent::Connected { .. })), "{ev:?}");
        let texts: Vec<_> = ev
            .iter()
            .filter_map(|x| match x {
                WsEvent::Message { message, .. } => Some(format!("{message:?}")),
                _ => None,
            })
            .collect();
        assert_eq!(texts, vec!["Text(\"hello\")".to_string(), "Binary([1, 2, 3])".to_string()]);
        assert!(ev.iter().any(|x| matches!(x, WsEvent::PingRtt { .. })), "pong received");
        assert_eq!(c.messages_sent(), 2);
        assert_eq!(c.messages_received(), 2);
        c.close();
        assert_eq!(c.state, WsState::Closed);
    }

    #[cfg(feature = "websocket")]
    #[test]
    fn refused_connection_backs_off_and_gives_up() {
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let mut c = WsClient::new(format!("ws://127.0.0.1:{port}/"));
        c.max_reconnects = 1;
        c.connect();
        let ev = pump(&mut c, |e| {
            e.iter().any(|x| matches!(x, WsEvent::Disconnected { will_reconnect: true, .. }))
        });
        assert!(ev.iter().any(|x| matches!(x, WsEvent::Error { .. })), "{ev:?}");
        assert_eq!(c.state, WsState::ReconnectBackoff);
    }
}
