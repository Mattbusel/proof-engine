//! Opt-in analytics and telemetry.
//!
//! Records gameplay events, performance stats, and session data.
//! All data is opt-in, aggregated, and contains no PII.
//! Players can view their own data or export it to CSV.

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use crate::networking::http::{HttpClient, HttpRequest};

// ── AnalyticsEvent ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum AnalyticsEvent {
    SessionStart   { build: String, platform: String },
    SessionEnd     { duration_secs: f32, outcome: String },
    Death          { floor: u32, cause: String, score: i64, time_secs: f32 },
    BossEncounter  { name: String, floor: u32, defeated: bool, attempts: u32 },
    Achievement    { id: String, name: String },
    ItemPickup     { item: String, rarity: String, floor: u32 },
    FloorComplete  { floor: u32, time_secs: f32, score: i64 },
    FrameTime      { avg_ms: f32, p99_ms: f32, min_ms: f32, max_ms: f32 },
    MemoryUsage    { bytes: u64, peak_bytes: u64 },
    Custom         { name: String, value: f64, tags: Vec<String> },
}

impl AnalyticsEvent {
    pub fn name(&self) -> &str {
        match self {
            Self::SessionStart  { .. } => "session_start",
            Self::SessionEnd    { .. } => "session_end",
            Self::Death         { .. } => "death",
            Self::BossEncounter { .. } => "boss_encounter",
            Self::Achievement   { .. } => "achievement",
            Self::ItemPickup    { .. } => "item_pickup",
            Self::FloorComplete { .. } => "floor_complete",
            Self::FrameTime     { .. } => "frame_time",
            Self::MemoryUsage   { .. } => "memory_usage",
            Self::Custom { name, .. } => name.as_str(),
        }
    }

    pub fn to_json(&self) -> String {
        match self {
            Self::SessionStart { build, platform } =>
                format!(r#"{{"event":"session_start","build":"{}","platform":"{}"}}"#, build, platform),
            Self::SessionEnd { duration_secs, outcome } =>
                format!(r#"{{"event":"session_end","duration":{:.2},"outcome":"{}"}}"#, duration_secs, outcome),
            Self::Death { floor, cause, score, time_secs } =>
                format!(r#"{{"event":"death","floor":{},"cause":"{}","score":{},"time":{:.2}}}"#,
                    floor, cause, score, time_secs),
            Self::BossEncounter { name, floor, defeated, attempts } =>
                format!(r#"{{"event":"boss","name":"{}","floor":{},"defeated":{},"attempts":{}}}"#,
                    name, floor, defeated, attempts),
            Self::FloorComplete { floor, time_secs, score } =>
                format!(r#"{{"event":"floor_complete","floor":{},"time":{:.2},"score":{}}}"#,
                    floor, time_secs, score),
            Self::FrameTime { avg_ms, p99_ms, min_ms, max_ms } =>
                format!(r#"{{"event":"frame_time","avg":{:.2},"p99":{:.2},"min":{:.2},"max":{:.2}}}"#,
                    avg_ms, p99_ms, min_ms, max_ms),
            Self::Custom { name, value, tags } =>
                format!(r#"{{"event":"{}","value":{:.6},"tags":[{}]}}"#,
                    name, value,
                    tags.iter().map(|t| format!("\"{}\"", t)).collect::<Vec<_>>().join(",")),
            _ => format!(r#"{{"event":"{}"}}"#, self.name()),
        }
    }
}

// ── SessionStats ──────────────────────────────────────────────────────────────

/// Rolling statistics for the current session.
#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    pub deaths:       u32,
    pub kills:        u32,
    pub floors:       u32,
    pub items:        u32,
    pub score:        i64,
    pub play_time:    f32,
    pub bosses_seen:  u32,
    pub bosses_killed: u32,
    pub max_floor:    u32,
    pub total_damage_dealt: f64,
    pub total_damage_taken: f64,
    pub frame_times:  Vec<f32>,
}

impl SessionStats {
    pub fn avg_frame_ms(&self) -> f32 {
        if self.frame_times.is_empty() { return 0.0; }
        self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32
    }

    pub fn p99_frame_ms(&self) -> f32 {
        if self.frame_times.is_empty() { return 0.0; }
        let mut sorted = self.frame_times.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((sorted.len() as f32 * 0.99) as usize).min(sorted.len()-1);
        sorted[idx]
    }

    pub fn to_csv_row(&self) -> String {
        format!("{},{},{},{},{},{:.2},{},{},{:.2},{:.2},{:.2},{:.2}",
            self.deaths, self.kills, self.floors, self.items, self.score,
            self.play_time, self.bosses_seen, self.bosses_killed,
            self.total_damage_dealt, self.total_damage_taken,
            self.avg_frame_ms(), self.p99_frame_ms())
    }

    pub fn csv_header() -> &'static str {
        "deaths,kills,floors,items,score,play_time,bosses_seen,bosses_killed,damage_dealt,damage_taken,avg_frame_ms,p99_frame_ms"
    }
}

// ── Analytics ─────────────────────────────────────────────────────────────────

/// Opt-in analytics system. Call `record()` to log events.
/// `tick()` batches them and uploads when enough accumulate or on flush.
pub struct Analytics {
    /// Whether telemetry is enabled (user opt-in).
    pub enabled:         bool,
    pub endpoint:        Option<String>,
    pub build_version:   String,
    pub platform:        String,
    pub session_id:      String,
    pub stats:           SessionStats,

    pending:             Vec<AnalyticsEvent>,
    local_log:           VecDeque<(String, String)>, // (timestamp, json)
    http:                Option<HttpClient>,
    session_start:       Instant,
    flush_interval:      f32,
    flush_timer:         f32,
    batch_size:          usize,
    max_local_log:       usize,
    uploads_sent:        u32,
}

impl Analytics {
    pub fn new(build: impl Into<String>, platform: impl Into<String>) -> Self {
        Self {
            enabled:        false, // Opt-in by default
            endpoint:       None,
            build_version:  build.into(),
            platform:       platform.into(),
            session_id:     Self::generate_session_id(),
            stats:          SessionStats::default(),
            pending:        Vec::new(),
            local_log:      VecDeque::new(),
            http:           None,
            session_start:  Instant::now(),
            flush_interval: 60.0, // flush every 60s
            flush_timer:    0.0,
            batch_size:     100,
            max_local_log:  10_000,
            uploads_sent:   0,
        }
    }

    pub fn enable(&mut self, endpoint: impl Into<String>) {
        self.enabled  = true;
        self.endpoint = Some(endpoint.into());
        self.http     = Some(HttpClient::new());
        self.record(AnalyticsEvent::SessionStart {
            build:    self.build_version.clone(),
            platform: self.platform.clone(),
        });
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Record an analytics event. No-op if disabled.
    pub fn record(&mut self, event: AnalyticsEvent) {
        if !self.enabled { return; }

        // Update session stats
        match &event {
            AnalyticsEvent::Death { .. } => self.stats.deaths += 1,
            AnalyticsEvent::BossEncounter { defeated, .. } => {
                self.stats.bosses_seen += 1;
                if *defeated { self.stats.bosses_killed += 1; }
            }
            AnalyticsEvent::ItemPickup { .. } => self.stats.items += 1,
            AnalyticsEvent::FloorComplete { floor, .. } => {
                self.stats.floors += 1;
                self.stats.max_floor = self.stats.max_floor.max(*floor);
            }
            AnalyticsEvent::FrameTime { avg_ms, .. } => {
                self.stats.frame_times.push(*avg_ms);
                // Keep last 1000 frame time samples
                if self.stats.frame_times.len() > 1000 {
                    self.stats.frame_times.remove(0);
                }
            }
            _ => {}
        }

        let ts  = format!("{}", self.session_start.elapsed().as_secs());
        let json = event.to_json();

        // Add session_id to every event
        let full_json = format!(r#"{{"session":"{}","t":{},"data":{}}}"#,
            self.session_id, ts, json);

        if self.local_log.len() < self.max_local_log {
            self.local_log.push_back((ts, full_json.clone()));
        }
        self.pending.push(event);
    }

    /// Drive uploads. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled { return; }

        self.stats.play_time += dt;
        self.flush_timer += dt;

        let should_flush = self.flush_timer >= self.flush_interval
            || self.pending.len() >= self.batch_size;

        if should_flush && !self.pending.is_empty() {
            self.flush();
        }

        if let Some(ref mut http) = self.http {
            http.tick(dt);
            // Drain http events (fire-and-forget analytics)
            let _: Vec<_> = http.drain_events().collect();
        }
    }

    /// Force-upload all pending events.
    pub fn flush(&mut self) {
        if self.pending.is_empty() { return; }

        let batch: Vec<String> = self.local_log.iter()
            .rev()
            .take(self.pending.len())
            .map(|(_, j)| j.clone())
            .collect();

        if let (Some(ref endpoint), Some(ref mut http)) = (self.endpoint.clone(), self.http.as_mut()) {
            let url  = format!("{}/events", endpoint);
            let body = format!("[{}]", batch.join(","));
            let req  = HttpRequest::post_json(url, body)
                .with_header("X-Session-Id", self.session_id.clone());
            http.send(req);
            self.uploads_sent += 1;
        }

        self.pending.clear();
        self.flush_timer = 0.0;
    }

    /// Export the local event log to CSV.
    pub fn export_csv(&self) -> String {
        let mut csv = String::from("timestamp,event_json\n");
        for (ts, json) in &self.local_log {
            csv.push_str(&format!("{},{}\n", ts, json.replace(',', ";")));
        }
        csv
    }

    /// Get the local log as a string slice view (newest last).
    pub fn local_log_entries(&self) -> impl Iterator<Item = &(String, String)> {
        self.local_log.iter()
    }

    /// Clear the local log.
    pub fn clear_local_log(&mut self) { self.local_log.clear(); }

    pub fn session_duration(&self) -> Duration { self.session_start.elapsed() }
    pub fn uploads_sent(&self) -> u32 { self.uploads_sent }

    fn generate_session_id() -> String {
        // Simple deterministic ID from process start time + counter
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        // Poor man's UUID-ish hex
        format!("{:016x}{:016x}", n, n.wrapping_mul(0xdeadbeef_cafebabe))
    }
}
