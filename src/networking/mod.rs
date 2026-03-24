//! Networking module: HTTP client, WebSocket client, connection management.
//!
//! Provides async-compatible networking primitives for leaderboards,
//! replay sharing, analytics, and live updates. Designed to work with
//! Rust's standard library plus minimal dependencies — uses non-blocking
//! TCP sockets under the hood with a simple state-machine event loop.
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

pub use http::{HttpClient, HttpRequest, HttpResponse, HttpEvent, Method};
pub use websocket::{WsClient, WsMessage, WsEvent, WsState};
pub use leaderboard::{LeaderboardClient, ScoreEntry, LeaderboardEvent};
pub use analytics::{Analytics, AnalyticsEvent, SessionStats};
