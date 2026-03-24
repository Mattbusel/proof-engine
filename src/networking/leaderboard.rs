//! Leaderboard protocol: submit scores, fetch boards, paginate results.

use std::collections::VecDeque;
use crate::networking::http::{HttpClient, HttpRequest, HttpEvent, RequestId, Method};

// ── ScoreEntry ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ScoreEntry {
    pub rank:      u32,
    pub player_id: String,
    pub name:      String,
    pub score:     i64,
    /// Arbitrary metadata: class, build, floor, kills, etc.
    pub metadata:  std::collections::HashMap<String, String>,
    /// ISO-8601 timestamp.
    pub timestamp: String,
    /// Optional replay URL.
    pub replay_url: Option<String>,
}

// ── LeaderboardFilter ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct LeaderboardFilter {
    pub period:      Option<String>,   // "daily", "weekly", "all_time"
    pub class:       Option<String>,
    pub min_score:   Option<i64>,
    pub max_score:   Option<i64>,
    pub page:        u32,
    pub page_size:   u32,
}

impl LeaderboardFilter {
    pub fn new() -> Self { Self { page_size: 100, ..Default::default() } }
    pub fn daily(mut self) -> Self { self.period = Some("daily".into()); self }
    pub fn weekly(mut self) -> Self { self.period = Some("weekly".into()); self }
    pub fn all_time(mut self) -> Self { self.period = Some("all_time".into()); self }
    pub fn page(mut self, p: u32) -> Self { self.page = p; self }
    pub fn page_size(mut self, n: u32) -> Self { self.page_size = n; self }
}

// ── LeaderboardEvent ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum LeaderboardEvent {
    ScoreSubmitted { rank: u32, score: i64 },
    ScoreRejected  { reason: String },
    FetchSuccess   { entries: Vec<ScoreEntry>, total: u32, page: u32 },
    FetchFailed    { reason: String },
    PlayerRank     { rank: u32, entry: ScoreEntry },
    RankNotFound   { player_id: String },
}

// ── ScoreSubmission ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ScoreSubmission {
    pub player_id:   String,
    pub name:        String,
    pub score:       i64,
    pub metadata:    std::collections::HashMap<String, String>,
    /// Optional anti-cheat checksum (SHA-256 of score + secret).
    pub checksum:    Option<String>,
    /// Optional replay data attachment.
    pub replay_id:   Option<String>,
}

impl ScoreSubmission {
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

    pub fn with_meta(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), val.into());
        self
    }

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
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http:     HttpClient::new(),
            base_url: base_url.into(),
            api_key:  None,
            events:   VecDeque::new(),
            pending:  Vec::new(),
        }
    }

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
                _ => {}
            }
        }
    }

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
                // Stub: parse entries from JSON (in real impl: full serde)
                self.events.push_back(LeaderboardEvent::FetchSuccess {
                    entries: Vec::new(),
                    total: 0,
                    page,
                });
            }
            LeaderboardOp::GetRank { player_id } => {
                if response.is_success() {
                    let rank = response.json_field("rank").and_then(|s| s.parse().ok()).unwrap_or(0);
                    let score_val = response.json_field("score").and_then(|s| s.parse().ok()).unwrap_or(0);
                    self.events.push_back(LeaderboardEvent::PlayerRank {
                        rank,
                        entry: ScoreEntry {
                            rank,
                            player_id: player_id.clone(),
                            name: response.json_field("name").unwrap_or_default(),
                            score: score_val,
                            metadata: std::collections::HashMap::new(),
                            timestamp: response.json_field("timestamp").unwrap_or_default(),
                            replay_url: response.json_field("replay_url"),
                        },
                    });
                } else {
                    self.events.push_back(LeaderboardEvent::RankNotFound { player_id });
                }
            }
        }
    }

    fn serialize_submission(&self, s: &ScoreSubmission) -> String {
        let meta_pairs: String = s.metadata.iter()
            .map(|(k, v)| format!("\"{}\":\"{}\"", k, v))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"player_id":"{}","name":"{}","score":{},"metadata":{{{}}}}}"#,
            s.player_id, s.name, s.score, meta_pairs
        )
    }

    fn build_fetch_url(&self, f: &LeaderboardFilter) -> String {
        let mut params = Vec::new();
        if let Some(ref p) = f.period   { params.push(format!("period={}", p)); }
        if let Some(ref c) = f.class    { params.push(format!("class={}", c)); }
        params.push(format!("page={}", f.page));
        params.push(format!("page_size={}", f.page_size));
        if params.is_empty() {
            format!("{}/scores", self.base_url)
        } else {
            format!("{}/scores?{}", self.base_url, params.join("&"))
        }
    }
}
