//! Leaderboard protocol: submit scores, fetch boards, paginate results.
//!
//! Speaks JSON over [`HttpClient`] (needs the `http` feature for real
//! network access). The expected server API:
//!
//! - `POST {base}/scores` with `{"player_id", "name", "score", "metadata",
//!   "checksum"?, "replay_id"?}`, answered with `{"rank", "score"}`.
//! - `GET {base}/scores?period=&class=&page=&page_size=` answered with either
//!   a JSON array of entries or `{"entries": [...], "total": N}`.
//! - `GET {base}/scores/{player_id}` answered with one entry.
//!
//! An entry is `{"rank", "player_id" (or "id"), "name", "score",
//! "metadata"?, "timestamp"?, "replay_url"?}`; numbers may also be strings.

#![warn(missing_docs)]

use std::collections::VecDeque;
use crate::networking::http::{HttpClient, HttpRequest, HttpEvent, RequestId, Method};

// ── ScoreEntry ────────────────────────────────────────────────────────────────

/// One row of a leaderboard.
#[derive(Debug, Clone)]
pub struct ScoreEntry {
    /// Position on the board, 1 = best.
    pub rank:      u32,
    /// Stable player identifier.
    pub player_id: String,
    /// Display name.
    pub name:      String,
    /// The score.
    pub score:     i64,
    /// Arbitrary metadata: class, build, floor, kills, etc.
    pub metadata:  std::collections::HashMap<String, String>,
    /// ISO-8601 timestamp.
    pub timestamp: String,
    /// Optional replay URL.
    pub replay_url: Option<String>,
}

// ── LeaderboardFilter ─────────────────────────────────────────────────────────

/// Which part of the board to fetch.
#[derive(Debug, Clone, Default)]
pub struct LeaderboardFilter {
    /// Time window: `"daily"`, `"weekly"` or `"all_time"`; sent as `period=`.
    pub period:      Option<String>,
    /// Only this character class; sent as `class=`.
    pub class:       Option<String>,
    /// Lowest score wanted; sent as `min_score=`.
    pub min_score:   Option<i64>,
    /// Highest score wanted; sent as `max_score=`.
    pub max_score:   Option<i64>,
    /// Page number, starting at 0.
    pub page:        u32,
    /// Entries per page.
    pub page_size:   u32,
}

impl LeaderboardFilter {
    /// First page, 100 entries, no period or class.
    pub fn new() -> Self { Self { page_size: 100, ..Default::default() } }
    /// Today's scores.
    pub fn daily(mut self) -> Self { self.period = Some("daily".into()); self }
    /// This week's scores.
    pub fn weekly(mut self) -> Self { self.period = Some("weekly".into()); self }
    /// All scores ever.
    pub fn all_time(mut self) -> Self { self.period = Some("all_time".into()); self }
    /// Set the page number.
    pub fn page(mut self, p: u32) -> Self { self.page = p; self }
    /// Set the entries per page.
    pub fn page_size(mut self, n: u32) -> Self { self.page_size = n; self }
}

// ── LeaderboardEvent ──────────────────────────────────────────────────────────

/// Results from `LeaderboardClient::drain_events`.
#[derive(Debug, Clone)]
pub enum LeaderboardEvent {
    /// The server accepted a score.
    ScoreSubmitted {
        /// Rank the score reached.
        rank: u32,
        /// The score as recorded by the server.
        score: i64,
    },
    /// A submission failed (HTTP error, network error or timeout).
    ScoreRejected {
        /// Server response text or error description.
        reason: String,
    },
    /// A page of the board arrived.
    FetchSuccess {
        /// The entries on this page.
        entries: Vec<ScoreEntry>,
        /// Total entries on the board, if the server said; otherwise this page's count.
        total: u32,
        /// The page that was requested.
        page: u32,
    },
    /// A fetch or rank lookup failed.
    FetchFailed {
        /// What went wrong.
        reason: String,
    },
    /// A player's rank arrived.
    PlayerRank {
        /// The player's rank.
        rank: u32,
        /// The player's entry.
        entry: ScoreEntry,
    },
    /// The server had no entry for this player.
    RankNotFound {
        /// The player that was looked up.
        player_id: String,
    },
}

// ── ScoreSubmission ───────────────────────────────────────────────────────────

/// A score to send to the board.
#[derive(Debug, Clone)]
pub struct ScoreSubmission {
    /// Stable player identifier.
    pub player_id:   String,
    /// Display name.
    pub name:        String,
    /// The score.
    pub score:       i64,
    /// Extra fields sent as a JSON object (class, floor, kills and so on).
    pub metadata:    std::collections::HashMap<String, String>,
    /// Optional anti-cheat checksum (SHA-256 of score + secret).
    pub checksum:    Option<String>,
    /// Optional replay data attachment.
    pub replay_id:   Option<String>,
}

impl ScoreSubmission {
    /// A submission with no metadata, checksum or replay.
    pub fn new(player_id: impl Into<String>, name: impl Into<String>, score: i64) -> Self {
        Self {
            player_id: player_id.into(),
            name:      name.into(),
            score,
            metadata:  std::collections::HashMap::new(),
            checksum:  None,
            replay_id: None,
        }
    }

    /// Add a metadata field.
    pub fn with_meta(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), val.into());
        self
    }

    /// Attach a replay id.
    pub fn with_replay(mut self, id: impl Into<String>) -> Self {
        self.replay_id = Some(id.into());
        self
    }
}

// ── LeaderboardClient ────────────────────────────────────────────────────────

/// High-level leaderboard API built on top of HttpClient.
pub struct LeaderboardClient {
    http:           HttpClient,
    base_url:       String,
    api_key:        Option<String>,
    events:         VecDeque<LeaderboardEvent>,
    pending:        Vec<(RequestId, LeaderboardOp)>,
}

#[derive(Debug, Clone)]
enum LeaderboardOp {
    Submit,
    Fetch { page: u32 },
    GetRank { player_id: String },
}

impl LeaderboardClient {
    /// A client for the board at `base_url` (no trailing slash).
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http:     HttpClient::new(),
            base_url: base_url.into(),
            api_key:  None,
            events:   VecDeque::new(),
            pending:  Vec::new(),
        }
    }

    /// Send `X-API-Key: <key>` with every request.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Submit a score.
    pub fn submit(&mut self, submission: ScoreSubmission) {
        let url  = format!("{}/scores", self.base_url);
        let body = self.serialize_submission(&submission);
        let mut req = HttpRequest::post_json(url, body);
        if let Some(ref key) = self.api_key {
            req = req.with_header("X-API-Key", key.clone());
        }
        let id = self.http.send(req);
        self.pending.push((id, LeaderboardOp::Submit));
    }

    /// Fetch leaderboard entries.
    pub fn fetch(&mut self, filter: LeaderboardFilter) {
        let url = self.build_fetch_url(&filter);
        let mut req = HttpRequest::get(url);
        if let Some(ref key) = self.api_key {
            req = req.with_header("X-API-Key", key.clone());
        }
        let page = filter.page;
        let id   = self.http.send(req);
        self.pending.push((id, LeaderboardOp::Fetch { page }));
    }

    /// Get a specific player's rank.
    pub fn get_rank(&mut self, player_id: impl Into<String>) {
        let pid = player_id.into();
        let url = format!("{}/scores/{}", self.base_url, pid);
        let mut req = HttpRequest::get(url);
        if let Some(ref key) = self.api_key {
            req = req.with_header("X-API-Key", key.clone());
        }
        let id = self.http.send(req);
        self.pending.push((id, LeaderboardOp::GetRank { player_id: pid }));
    }

    /// Drive the client. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.http.tick(dt);

        let http_events: Vec<HttpEvent> = self.http.drain_events().collect();
        for event in http_events {
            match event {
                HttpEvent::Success { id, response } => {
                    if let Some(pos) = self.pending.iter().position(|(rid, _)| *rid == id) {
                        let (_, op) = self.pending.remove(pos);
                        self.process_response(op, &response);
                    }
                }
                HttpEvent::Failure { id, error, .. } => {
                    if let Some(pos) = self.pending.iter().position(|(rid, _)| *rid == id) {
                        let (_, op) = self.pending.remove(pos);
                        match op {
                            LeaderboardOp::Submit => {
                                self.events.push_back(LeaderboardEvent::ScoreRejected {
                                    reason: error.to_string(),
                                });
                            }
                            LeaderboardOp::Fetch { .. } | LeaderboardOp::GetRank { .. } => {
                                self.events.push_back(LeaderboardEvent::FetchFailed {
                                    reason: error.to_string(),
                                });
                            }
                        }
                    }
                }
                HttpEvent::Timeout { id, url } => {
                    self.fail(id, format!("timed out: {url}"));
                }
                HttpEvent::Cancelled { id } => {
                    self.fail(id, "cancelled".into());
                }
                HttpEvent::RateLimited { .. } => {}
            }
        }
    }

    /// Resolve a pending operation as failed. Timeouts and cancellations used
    /// to be ignored, so the caller never heard back.
    fn fail(&mut self, id: RequestId, reason: String) {
        if let Some(pos) = self.pending.iter().position(|(rid, _)| *rid == id) {
            let (_, op) = self.pending.remove(pos);
            self.events.push_back(match op {
                LeaderboardOp::Submit => LeaderboardEvent::ScoreRejected { reason },
                LeaderboardOp::Fetch { .. } | LeaderboardOp::GetRank { .. } => {
                    LeaderboardEvent::FetchFailed { reason }
                }
            });
        }
    }

    /// Take all events produced since the last call.
    pub fn drain_events(&mut self) -> impl Iterator<Item = LeaderboardEvent> + '_ {
        self.events.drain(..)
    }

    fn process_response(
        &mut self,
        op: LeaderboardOp,
        response: &crate::networking::http::HttpResponse,
    ) {
        match op {
            LeaderboardOp::Submit => {
                if response.is_success() {
                    let rank  = response.json_field("rank").and_then(|s| s.parse().ok()).unwrap_or(0);
                    let score = response.json_field("score").and_then(|s| s.parse().ok()).unwrap_or(0);
                    self.events.push_back(LeaderboardEvent::ScoreSubmitted { rank, score });
                } else {
                    self.events.push_back(LeaderboardEvent::ScoreRejected {
                        reason: response.text_body().chars().take(200).collect(),
                    });
                }
            }
            LeaderboardOp::Fetch { page } => {
                if !response.is_success() {
                    self.events.push_back(LeaderboardEvent::FetchFailed {
                        reason: format!("HTTP {}: {}", response.status,
                            response.text_body().chars().take(200).collect::<String>()),
                    });
                    return;
                }
                match response.json().map_err(|e| e.to_string()).and_then(|v| parse_board(&v)) {
                    Ok((entries, total)) => self.events.push_back(LeaderboardEvent::FetchSuccess {
                        entries,
                        total,
                        page,
                    }),
                    Err(reason) => self.events.push_back(LeaderboardEvent::FetchFailed { reason }),
                }
            }
            LeaderboardOp::GetRank { player_id } => {
                let parsed = response.json().ok().and_then(|v| parse_entry(&v));
                if let (true, Some(mut entry)) = (response.is_success(), parsed) {
                    if entry.player_id.is_empty() {
                        entry.player_id = player_id.clone();
                    }
                    self.events.push_back(LeaderboardEvent::PlayerRank { rank: entry.rank, entry });
                } else {
                    self.events.push_back(LeaderboardEvent::RankNotFound { player_id });
                }
            }
        }
    }

    /// JSON body for a submission. Built with serde_json, so names with
    /// quotes or backslashes stay valid JSON (the old `format!` did not
    /// escape them), and `checksum` / `replay_id` are actually sent.
    fn serialize_submission(&self, s: &ScoreSubmission) -> String {
        let mut body = serde_json::json!({
            "player_id": s.player_id,
            "name": s.name,
            "score": s.score,
            "metadata": s.metadata,
        });
        if let Some(c) = &s.checksum {
            body["checksum"] = serde_json::Value::String(c.clone());
        }
        if let Some(r) = &s.replay_id {
            body["replay_id"] = serde_json::Value::String(r.clone());
        }
        body.to_string()
    }

    fn build_fetch_url(&self, f: &LeaderboardFilter) -> String {
        let mut params = Vec::new();
        if let Some(ref p) = f.period   { params.push(format!("period={}", url_encode(p))); }
        if let Some(ref c) = f.class    { params.push(format!("class={}", url_encode(c))); }
        if let Some(v) = f.min_score    { params.push(format!("min_score={v}")); }
        if let Some(v) = f.max_score    { params.push(format!("max_score={v}")); }
        params.push(format!("page={}", f.page));
        params.push(format!("page_size={}", f.page_size));
        if params.is_empty() {
            format!("{}/scores", self.base_url)
        } else {
            format!("{}/scores?{}", self.base_url, params.join("&"))
        }
    }
}

// ── JSON parsing ──────────────────────────────────────────────────────────────

/// Percent-encode a query value (RFC 3986 unreserved characters pass through).
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn as_i64(v: &serde_json::Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_f64().map(|f| f as i64))
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

fn as_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// One leaderboard entry from a JSON object. `None` if it is not an object
/// or has no usable score.
fn parse_entry(v: &serde_json::Value) -> Option<ScoreEntry> {
    let o = v.as_object()?;
    let score = o.get("score").and_then(as_i64)?;
    let metadata = o
        .get("metadata")
        .and_then(|m| m.as_object())
        .map(|m| m.iter().map(|(k, v)| (k.clone(), as_text(v))).collect())
        .unwrap_or_default();
    Some(ScoreEntry {
        rank: o.get("rank").and_then(as_i64).unwrap_or(0).clamp(0, u32::MAX as i64) as u32,
        player_id: o.get("player_id").or_else(|| o.get("id")).map(as_text).unwrap_or_default(),
        name: o.get("name").map(as_text).unwrap_or_default(),
        score,
        metadata,
        timestamp: o.get("timestamp").map(as_text).unwrap_or_default(),
        replay_url: o.get("replay_url").map(as_text).filter(|s| !s.is_empty()),
    })
}

/// A page of entries: a bare array, or `{"entries": [...], "total": N}`.
/// Missing ranks are filled from the position on the page.
fn parse_board(v: &serde_json::Value) -> Result<(Vec<ScoreEntry>, u32), String> {
    let (list, total) = match v {
        serde_json::Value::Array(a) => (a, None),
        serde_json::Value::Object(o) => (
            o.get("entries")
                .and_then(|e| e.as_array())
                .ok_or("expected an \"entries\" array")?,
            o.get("total").and_then(as_i64),
        ),
        _ => return Err("expected a JSON array or object".into()),
    };
    let mut entries: Vec<ScoreEntry> = list.iter().filter_map(parse_entry).collect();
    for (i, e) in entries.iter_mut().enumerate() {
        if e.rank == 0 {
            e.rank = i as u32 + 1;
        }
    }
    let total = total.map(|t| t.max(0) as u32).unwrap_or(entries.len() as u32);
    Ok((entries, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_array_and_wrapped_pages() {
        let v: serde_json::Value = serde_json::from_str(
            r#"[{"name":"Ada","player_id":"p1","score":"4200","rank":1,"seed":"123","ts":1},
                {"name":"Bo","id":"p2","score":900,"metadata":{"class":"Rogue","floor":3}}]"#,
        ).unwrap();
        let (entries, total) = parse_board(&v).unwrap();
        assert_eq!(total, 2);
        assert_eq!(entries[0].score, 4200);
        assert_eq!(entries[1].player_id, "p2");
        assert_eq!(entries[1].rank, 2);
        assert_eq!(entries[1].metadata["floor"], "3");

        let v: serde_json::Value =
            serde_json::from_str(r#"{"entries":[{"name":"C","score":5}],"total":77}"#).unwrap();
        let (entries, total) = parse_board(&v).unwrap();
        assert_eq!((entries.len(), total), (1, 77));
        assert!(parse_board(&serde_json::json!("nope")).is_err());
    }

    #[test]
    fn submission_json_is_escaped_and_complete() {
        let client = LeaderboardClient::new("http://x");
        let mut s = ScoreSubmission::new("p\"1", "Name \"with\" quotes\\", 10).with_replay("r9");
        s.checksum = Some("abc".into());
        let v: serde_json::Value = serde_json::from_str(&client.serialize_submission(&s)).unwrap();
        assert_eq!(v["name"], "Name \"with\" quotes\\");
        assert_eq!(v["player_id"], "p\"1");
        assert_eq!(v["replay_id"], "r9");
        assert_eq!(v["checksum"], "abc");
    }

    #[test]
    fn query_values_are_encoded() {
        let client = LeaderboardClient::new("http://x");
        let url = client.build_fetch_url(&LeaderboardFilter::new().daily());
        assert!(url.ends_with("/scores?period=daily&page=0&page_size=100"), "{url}");
        let mut f = LeaderboardFilter::new();
        f.class = Some("Void Walker&x=1".into());
        assert!(client.build_fetch_url(&f).contains("class=Void%20Walker%26x%3D1"));
        f.min_score = Some(100);
        f.max_score = Some(-5);
        let url = client.build_fetch_url(&f);
        assert!(url.contains("min_score=100") && url.contains("max_score=-5"), "{url}");
    }
}
