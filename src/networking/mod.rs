//! Networking module: HTTP client, WebSocket client, connection management.
//!
//! Networking primitives for leaderboards, replay sharing, analytics and
//! live updates. Real network I/O needs cargo features: `http` (ureq) for
//! [`http`], [`leaderboard`] and [`analytics`], `websocket` (tungstenite) for
//! [`websocket`], or `net` for both. Without them the clients report an error
//! instead of talking to the network. Requests and connections run on
//! background threads; nothing blocks the frame.
//!
//! ## Modules
//! - `http`      — HTTP/HTTPS request/response with retry, caching, rate limiting
//! - `websocket` — WebSocket client with auto-reconnect and message queueing
//! - `leaderboard` — Leaderboard protocol: submit, fetch, paginate
//! - `analytics` — Opt-in telemetry: session, deaths, performance
//!
//! ## Design
//! All network operations are non-blocking. `tick(dt)` drives the state
//! machines each frame. Results are delivered through `Event` queues that
//! the game polls each frame — no async runtime required.

pub mod http;
pub mod websocket;
pub mod leaderboard;
pub mod analytics;

pub mod protocol;
pub mod transport;
pub mod sync;
pub mod lobby;
pub mod rpc;

pub use http::{HttpClient, HttpRequest, HttpResponse, HttpEvent, Method};
pub use websocket::{WsClient, WsMessage, WsEvent, WsState};
pub use leaderboard::{LeaderboardClient, ScoreEntry, LeaderboardEvent};
pub use analytics::{Analytics, AnalyticsEvent, SessionStats};
