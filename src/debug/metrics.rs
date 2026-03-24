//! Metrics collection and performance instrumentation.
//!
//! Provides counters, gauges, histograms, rolling rates, exponential moving
//! averages, a Prometheus-compatible text exporter, and a performance dashboard
//! that aggregates engine-level statistics into a formatted table.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// ── helpers ───────────────────────────────────────────────────────────────────

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

// ── MetricKind ────────────────────────────────────────────────────────────────

/// The kind of a metric, determining how its value is interpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetricKind {
    /// Monotonically increasing count.
    Counter,
    /// Current instantaneous value (can go up or down).
    Gauge,
    /// Distribution of observed values across configurable buckets.
    Histogram,
    /// Percentile summary over a sliding window.
    Summary,
}

// ── MetricValue ───────────────────────────────────────────────────────────────

/// The actual numeric value stored by a `Metric`.
#[derive(Debug, Clone)]
pub enum MetricValue {
    /// Integer value used for counters and integer gauges.
    Int(i64),
    /// Floating-point value for gauges, rates, etc.
    Float(f64),
    /// Histogram distribution: (upper_bound, cumulative_count) pairs plus aggregate stats.
    Histogram {
        buckets: Vec<(f64, u64)>,
        sum:     f64,
        count:   u64,
    },
    /// Percentile summary.
    Summary {
        p50:   f64,
        p90:   f64,
        p95:   f64,
        p99:   f64,
        count: u64,
    },
}

impl Default for MetricValue {
    fn default() -> Self { MetricValue::Int(0) }
}

// ── Metric ────────────────────────────────────────────────────────────────────

/// A single named metric with labels and a current value.
#[derive(Debug, Clone)]
pub struct Metric {
    pub name:        String,
    pub kind:        MetricKind,
    pub value:       MetricValue,
    pub labels:      HashMap<String, String>,
    /// Unix millisecond timestamp of the last update.
    pub last_update: u64,
}

impl Metric {
    fn new(name: impl Into<String>, kind: MetricKind, labels: HashMap<String, String>) -> Self {
        let value = match kind {
            MetricKind::Counter   => MetricValue::Int(0),
            MetricKind::Gauge     => MetricValue::Float(0.0),
            MetricKind::Histogram => MetricValue::Histogram { buckets: Vec::new(), sum: 0.0, count: 0 },
            MetricKind::Summary   => MetricValue::Summary { p50: 0.0, p90: 0.0, p95: 0.0, p99: 0.0, count: 0 },
        };
        Self { name: name.into(), kind, value, labels, last_update: now_ms() }
    }
}

// ── MetricKey ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MetricKey {
    name:        String,
    sorted_labels: Vec<(String, String)>,
}

impl MetricKey {
    fn new(name: &str, labels: &HashMap<String, String>) -> Self {
        let mut sorted_labels: Vec<(String, String)> = labels
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        sorted_labels.sort_by(|a, b| a.0.cmp(&b.0));
        Self { name: name.to_owned(), sorted_labels }
    }
}

// ── HistogramBuckets ──────────────────────────────────────────────────────────

/// Configurable histogram bucket boundaries with statistical helpers.
#[derive(Debug, Clone)]
pub struct HistogramBuckets {
    /// Upper bounds of each bucket (must be sorted ascending).
    boundaries: Vec<f64>,
    /// Count of observations falling into each bucket (cumulative).
    counts:     Vec<u64>,
    /// All raw observed values (for exact percentile computation).
    observations: Vec<f64>,
    sum:         f64,
    count:       u64,
}

impl HistogramBuckets {
    /// Create buckets from explicit sorted upper bounds.
    pub fn new(boundaries: Vec<f64>) -> Self {
        let n = boundaries.len();
        Self {
            boundaries,
            counts: vec![0; n],
            observations: Vec::new(),
            sum: 0.0,
            count: 0,
        }
    }

    /// Standard latency buckets in milliseconds: 1, 5, 10, 25, 50, 100, 250, 500, 1000, 5000.
    pub fn latency_ms() -> Self {
        Self::new(vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 5000.0])
    }

    /// Exponential buckets: start * factor^i for i in 0..count.
    pub fn exponential(start: f64, factor: f64, count: usize) -> Self {
        let mut b = Vec::with_capacity(count);
        let mut v = start;
        for _ in 0..count {
            b.push(v);
            v *= factor;
        }
        Self::new(b)
    }

    /// Record an observed value.
    pub fn observe(&mut self, value: f64) {
        self.sum   += value;
        self.count += 1;
        self.observations.push(value);
        for (i, &bound) in self.boundaries.iter().enumerate() {
            if value <= bound {
                self.counts[i] += 1;
            }
        }
    }

    /// Estimate the p-th percentile (0.0–1.0) via linear interpolation.
    pub fn percentile(&self, p: f64) -> f64 {
        if self.observations.is_empty() { return 0.0; }
        let mut sorted = self.observations.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let rank = p * (sorted.len() - 1) as f64;
        let lo   = rank.floor() as usize;
        let hi   = rank.ceil() as usize;
        let frac = rank - lo as f64;
        if lo == hi { return sorted[lo]; }
        sorted[lo] * (1.0 - frac) + sorted[hi] * frac
    }

    /// Arithmetic mean of all observations.
    pub fn mean(&self) -> f64 {
        if self.count == 0 { return 0.0; }
        self.sum / self.count as f64
    }

    /// Population standard deviation of all observations.
    pub fn std_dev(&self) -> f64 {
        if self.count < 2 { return 0.0; }
        let mean = self.mean();
        let var  = self.observations.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / self.count as f64;
        var.sqrt()
    }

    /// Total count of observations.
    pub fn count(&self) -> u64 { self.count }

    /// Sum of all observations.
    pub fn sum(&self) -> f64 { self.sum }

    /// Returns (upper_bound, cumulative_count) pairs for Prometheus exposition.
    pub fn bucket_pairs(&self) -> Vec<(f64, u64)> {
        self.boundaries.iter().cloned().zip(self.counts.iter().cloned()).collect()
    }

    /// Reset all observations.
    pub fn reset(&mut self) {
        self.counts      = vec![0; self.boundaries.len()];
        self.observations.clear();
        self.sum   = 0.0;
        self.count = 0;
    }
}

// ── InternalHistogram ─────────────────────────────────────────────────────────

/// Internal storage for a histogram metric in the registry.
#[derive(Debug, Clone)]
struct InternalHistogram {
    buckets: HistogramBuckets,
}

impl InternalHistogram {
    fn new(boundaries: Vec<f64>) -> Self {
        Self { buckets: HistogramBuckets::new(boundaries) }
    }

    fn observe(&mut self, value: f64) {
        self.buckets.observe(value);
    }

    fn to_metric_value(&self) -> MetricValue {
        MetricValue::Histogram {
            buckets: self.buckets.bucket_pairs(),
            sum:     self.buckets.sum(),
            count:   self.buckets.count(),
        }
    }

    fn to_summary_value(&self) -> MetricValue {
        MetricValue::Summary {
            p50:   self.buckets.percentile(0.50),
            p90:   self.buckets.percentile(0.90),
            p95:   self.buckets.percentile(0.95),
            p99:   self.buckets.percentile(0.99),
            count: self.buckets.count(),
        }
    }
}

// ── RegistryEntry ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum RegistryEntry {
    Counter(i64),
    Gauge(f64),
    Histogram(InternalHistogram),
}

// ── MetricsRegistry ───────────────────────────────────────────────────────────

/// Thread-safe registry for creating and updating metrics.
///
/// All operations are guarded by an internal `Mutex`, making the registry safe
/// to share across threads via `Arc<MetricsRegistry>`.
pub struct MetricsRegistry {
    inner: Mutex<RegistryInner>,
}

#[derive(Debug, Default)]
struct RegistryInner {
    entries: HashMap<MetricKey, (MetricKind, RegistryEntry, HashMap<String, String>)>,
    /// Default histogram buckets for new histogram metrics.
    default_buckets: Vec<f64>,
}

impl RegistryInner {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            default_buckets: vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 5.0, 10.0],
        }
    }
}

impl MetricsRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self { inner: Mutex::new(RegistryInner::new()) }
    }

    /// Create a registry wrapped in an `Arc` for sharing.
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// Increment a counter by `delta` (default 1).
    pub fn counter(&self, name: &str, labels: HashMap<String, String>) -> i64 {
        self.counter_by(name, labels, 1)
    }

    /// Increment a counter by a specific amount.
    pub fn counter_by(&self, name: &str, labels: HashMap<String, String>, delta: i64) -> i64 {
        let key = MetricKey::new(name, &labels);
        let mut inner = self.inner.lock().unwrap();
        let entry = inner.entries.entry(key).or_insert_with(|| {
            (MetricKind::Counter, RegistryEntry::Counter(0), labels.clone())
        });
        if let RegistryEntry::Counter(ref mut v) = entry.1 {
            *v += delta;
            *v
        } else {
            0
        }
    }

    /// Set a gauge to `value`.
    pub fn gauge(&self, name: &str, labels: HashMap<String, String>, value: f64) {
        let key = MetricKey::new(name, &labels);
        let mut inner = self.inner.lock().unwrap();
        let entry = inner.entries.entry(key).or_insert_with(|| {
            (MetricKind::Gauge, RegistryEntry::Gauge(0.0), labels.clone())
        });
        if let RegistryEntry::Gauge(ref mut v) = entry.1 {
            *v = value;
        }
    }

    /// Add `delta` to a gauge.
    pub fn gauge_add(&self, name: &str, labels: HashMap<String, String>, delta: f64) {
        let key = MetricKey::new(name, &labels);
        let mut inner = self.inner.lock().unwrap();
        let entry = inner.entries.entry(key).or_insert_with(|| {
            (MetricKind::Gauge, RegistryEntry::Gauge(0.0), labels.clone())
        });
        if let RegistryEntry::Gauge(ref mut v) = entry.1 {
            *v += delta;
        }
    }

    /// Record a histogram observation.
    pub fn histogram_observe(&self, name: &str, labels: HashMap<String, String>, value: f64) {
        let key = MetricKey::new(name, &labels);
        let mut inner = self.inner.lock().unwrap();
        let buckets = inner.default_buckets.clone();
        let entry = inner.entries.entry(key).or_insert_with(|| {
            (MetricKind::Histogram, RegistryEntry::Histogram(InternalHistogram::new(buckets)), labels.clone())
        });
        if let RegistryEntry::Histogram(ref mut h) = entry.1 {
            h.observe(value);
        }
    }

    /// Override the default bucket boundaries for future histogram metrics.
    pub fn set_default_buckets(&self, boundaries: Vec<f64>) {
        let mut inner = self.inner.lock().unwrap();
        inner.default_buckets = boundaries;
    }

    /// Take a snapshot of all current metric values.
    pub fn snapshot(&self) -> Vec<Metric> {
        let inner = self.inner.lock().unwrap();
        let ts = now_ms();
        inner.entries.iter().map(|(key, (kind, entry, labels))| {
            let value = match entry {
                RegistryEntry::Counter(v) => MetricValue::Int(*v),
                RegistryEntry::Gauge(v)   => MetricValue::Float(*v),
                RegistryEntry::Histogram(h) => {
                    match kind {
                        MetricKind::Summary => h.to_summary_value(),
                        _                   => h.to_metric_value(),
                    }
                }
            };
            Metric {
                name:        key.name.clone(),
                kind:        kind.clone(),
                value,
                labels:      labels.clone(),
                last_update: ts,
            }
        }).collect()
    }

    /// Get the current counter value (returns 0 if not found).
    pub fn get_counter(&self, name: &str, labels: &HashMap<String, String>) -> i64 {
        let key = MetricKey::new(name, labels);
        let inner = self.inner.lock().unwrap();
        if let Some((_, RegistryEntry::Counter(v), _)) = inner.entries.get(&key) {
            *v
        } else {
            0
        }
    }

    /// Get the current gauge value (returns 0.0 if not found).
    pub fn get_gauge(&self, name: &str, labels: &HashMap<String, String>) -> f64 {
        let key = MetricKey::new(name, labels);
        let inner = self.inner.lock().unwrap();
        if let Some((_, RegistryEntry::Gauge(v), _)) = inner.entries.get(&key) {
            *v
        } else {
            0.0
        }
    }

    /// Reset all metrics (clear all entries).
    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.entries.clear();
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self { Self::new() }
}

// ── RollingCounter ────────────────────────────────────────────────────────────

/// A counter that tracks events in a fixed time window using a ring buffer.
///
/// `rate()` returns events per second observed within the window.
pub struct RollingCounter {
    /// Ring buffer of (timestamp_us, delta) pairs.
    buffer:      Vec<(u64, u64)>,
    head:        usize,
    capacity:    usize,
    window_us:   u64,
    total:       u64,
}

impl RollingCounter {
    /// Create a new rolling counter with the specified window in seconds.
    pub fn new(window_secs: f64) -> Self {
        let capacity = 4096;
        Self {
            buffer:    vec![(0, 0); capacity],
            head:      0,
            capacity,
            window_us: (window_secs * 1_000_000.0) as u64,
            total:     0,
        }
    }

    /// Record that `delta` events have occurred right now.
    pub fn record(&mut self, delta: u64) {
        let ts = now_us();
        self.buffer[self.head] = (ts, delta);
        self.head = (self.head + 1) % self.capacity;
        self.total += delta;
    }

    /// Increment by 1.
    pub fn increment(&mut self) { self.record(1); }

    /// Compute the event rate (events per second) within the rolling window.
    pub fn rate(&self) -> f64 {
        let now = now_us();
        let cutoff = now.saturating_sub(self.window_us);
        let events_in_window: u64 = self.buffer.iter()
            .filter(|&&(ts, _)| ts >= cutoff && ts > 0)
            .map(|&(_, delta)| delta)
            .sum();
        let window_secs = self.window_us as f64 / 1_000_000.0;
        events_in_window as f64 / window_secs
    }

    /// Total events ever recorded (not windowed).
    pub fn total(&self) -> u64 { self.total }

    /// Events within the current window.
    pub fn window_count(&self) -> u64 {
        let now = now_us();
        let cutoff = now.saturating_sub(self.window_us);
        self.buffer.iter()
            .filter(|&&(ts, _)| ts >= cutoff && ts > 0)
            .map(|&(_, delta)| delta)
            .sum()
    }

    /// Reset the counter.
    pub fn reset(&mut self) {
        for entry in &mut self.buffer { *entry = (0, 0); }
        self.head  = 0;
        self.total = 0;
    }
}

// ── ExponentialMovingAverage ──────────────────────────────────────────────────

/// Exponential moving average with configurable smoothing factor α.
///
/// EMA_n = α * value + (1 - α) * EMA_{n-1}.
/// Smaller α → smoother but slower to respond.
#[derive(Debug, Clone)]
pub struct ExponentialMovingAverage {
    alpha:       f64,
    value:       f64,
    initialized: bool,
    sample_count: u64,
}

impl ExponentialMovingAverage {
    /// Create a new EMA. `alpha` must be in (0, 1].
    ///
    /// A good default for frame times is α = 0.1 (90% weight on history).
    pub fn new(alpha: f64) -> Self {
        let alpha = alpha.clamp(1e-9, 1.0);
        Self { alpha, value: 0.0, initialized: false, sample_count: 0 }
    }

    /// Create an EMA tuned to smooth over approximately `n` samples.
    pub fn with_samples(n: f64) -> Self {
        Self::new(2.0 / (n + 1.0))
    }

    /// Update with a new observation.
    pub fn update(&mut self, value: f64) {
        if !self.initialized {
            self.value       = value;
            self.initialized = true;
        } else {
            self.value = self.alpha * value + (1.0 - self.alpha) * self.value;
        }
        self.sample_count += 1;
    }

    /// Get the current EMA value.
    pub fn get(&self) -> f64 { self.value }

    /// Number of samples seen.
    pub fn sample_count(&self) -> u64 { self.sample_count }

    /// Reset the EMA to uninitialized state.
    pub fn reset(&mut self) {
        self.value       = 0.0;
        self.initialized = false;
        self.sample_count = 0;
    }

    /// Current smoothing factor.
    pub fn alpha(&self) -> f64 { self.alpha }
}

// ── MetricsExporter ───────────────────────────────────────────────────────────

/// Formats a snapshot of metrics as Prometheus text exposition format.
///
/// Each metric is rendered as:
/// ```text
/// # HELP name <empty>
/// # TYPE name counter|gauge|histogram|summary
/// name{label="value",...} <value> <timestamp_ms>
/// ```
pub struct MetricsExporter {
    registry: Arc<MetricsRegistry>,
}

impl MetricsExporter {
    pub fn new(registry: Arc<MetricsRegistry>) -> Self {
        Self { registry }
    }

    /// Export all metrics in Prometheus text format.
    pub fn export(&self) -> String {
        let metrics = self.registry.snapshot();
        let mut lines = Vec::new();

        for m in &metrics {
            let type_str = match m.kind {
                MetricKind::Counter   => "counter",
                MetricKind::Gauge     => "gauge",
                MetricKind::Histogram => "histogram",
                MetricKind::Summary   => "summary",
            };
            lines.push(format!("# HELP {} ", m.name));
            lines.push(format!("# TYPE {} {}", m.name, type_str));

            let label_str = Self::format_labels(&m.labels);

            match &m.value {
                MetricValue::Int(v) => {
                    lines.push(format!("{}{} {} {}", m.name, label_str, v, m.last_update));
                }
                MetricValue::Float(v) => {
                    lines.push(format!("{}{} {} {}", m.name, label_str, v, m.last_update));
                }
                MetricValue::Histogram { buckets, sum, count } => {
                    for (bound, cnt) in buckets {
                        let bucket_label = Self::format_labels_with_extra(&m.labels, "le", &bound.to_string());
                        lines.push(format!("{}_bucket{} {} {}", m.name, bucket_label, cnt, m.last_update));
                    }
                    // +Inf bucket
                    let inf_label = Self::format_labels_with_extra(&m.labels, "le", "+Inf");
                    lines.push(format!("{}_bucket{} {} {}", m.name, inf_label, count, m.last_update));
                    lines.push(format!("{}_sum{} {} {}", m.name, label_str, sum, m.last_update));
                    lines.push(format!("{}_count{} {} {}", m.name, label_str, count, m.last_update));
                }
                MetricValue::Summary { p50, p90, p95, p99, count } => {
                    let q50 = Self::format_labels_with_extra(&m.labels, "quantile", "0.5");
                    let q90 = Self::format_labels_with_extra(&m.labels, "quantile", "0.9");
                    let q95 = Self::format_labels_with_extra(&m.labels, "quantile", "0.95");
                    let q99 = Self::format_labels_with_extra(&m.labels, "quantile", "0.99");
                    lines.push(format!("{}{} {} {}", m.name, q50, p50, m.last_update));
                    lines.push(format!("{}{} {} {}", m.name, q90, p90, m.last_update));
                    lines.push(format!("{}{} {} {}", m.name, q95, p95, m.last_update));
                    lines.push(format!("{}{} {} {}", m.name, q99, p99, m.last_update));
                    lines.push(format!("{}_count{} {} {}", m.name, label_str, count, m.last_update));
                }
            }
        }

        lines.join("\n") + "\n"
    }

    fn format_labels(labels: &HashMap<String, String>) -> String {
        if labels.is_empty() { return String::new(); }
        let mut pairs: Vec<_> = labels.iter().collect();
        pairs.sort_by_key(|(k, _)| k.as_str());
        let inner: Vec<String> = pairs.iter().map(|(k, v)| format!("{}=\"{}\"", k, v)).collect();
        format!("{{{}}}", inner.join(","))
    }

    fn format_labels_with_extra(labels: &HashMap<String, String>, key: &str, value: &str) -> String {
        let mut pairs: Vec<_> = labels.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        pairs.push((key, value));
        pairs.sort_by_key(|(k, _)| *k);
        let inner: Vec<String> = pairs.iter().map(|(k, v)| format!("{}=\"{}\"", k, v)).collect();
        format!("{{{}}}", inner.join(","))
    }
}

// ── EngineSnapshot ────────────────────────────────────────────────────────────

/// A snapshot of engine-level performance data passed to `PerformanceDashboard`.
#[derive(Debug, Clone, Default)]
pub struct EngineSnapshot {
    pub fps:              f64,
    pub frame_time_ms:    f64,
    pub entity_count:     usize,
    pub particle_count:   usize,
    pub glyph_count:      usize,
    /// Estimated heap usage in bytes.
    pub memory_estimate:  usize,
    /// Optional extra named values.
    pub extras:           Vec<(String, String)>,
}

// ── PerformanceDashboard ──────────────────────────────────────────────────────

/// Aggregates engine performance metrics and renders them as a formatted table
/// with box-drawing characters.
pub struct PerformanceDashboard {
    ema_fps:        ExponentialMovingAverage,
    ema_frame_ms:   ExponentialMovingAverage,
    peak_fps:       f64,
    min_fps:        f64,
    peak_frame_ms:  f64,
    last_snapshot:  EngineSnapshot,
    frame_count:    u64,
}

impl PerformanceDashboard {
    pub fn new() -> Self {
        Self {
            ema_fps:       ExponentialMovingAverage::new(0.1),
            ema_frame_ms:  ExponentialMovingAverage::new(0.1),
            peak_fps:      0.0,
            min_fps:       f64::MAX,
            peak_frame_ms: 0.0,
            last_snapshot: EngineSnapshot::default(),
            frame_count:   0,
        }
    }

    /// Update with a new engine snapshot.
    pub fn update(&mut self, snapshot: EngineSnapshot) {
        self.ema_fps.update(snapshot.fps);
        self.ema_frame_ms.update(snapshot.frame_time_ms);
        if snapshot.fps > self.peak_fps { self.peak_fps = snapshot.fps; }
        if snapshot.fps < self.min_fps  { self.min_fps  = snapshot.fps; }
        if snapshot.frame_time_ms > self.peak_frame_ms { self.peak_frame_ms = snapshot.frame_time_ms; }
        self.frame_count   += 1;
        self.last_snapshot  = snapshot;
    }

    /// Format the dashboard as a box-drawing table string.
    pub fn format_table(&self) -> String {
        let s = &self.last_snapshot;
        let rows: Vec<(&str, String)> = vec![
            ("FPS (cur)",    format!("{:>7.1}", s.fps)),
            ("FPS (avg)",    format!("{:>7.1}", self.ema_fps.get())),
            ("FPS (peak)",   format!("{:>7.1}", self.peak_fps)),
            ("FPS (min)",    format!("{:>7.1}", if self.min_fps == f64::MAX { 0.0 } else { self.min_fps })),
            ("Frame ms",     format!("{:>7.2}", s.frame_time_ms)),
            ("Frame ms avg", format!("{:>7.2}", self.ema_frame_ms.get())),
            ("Frame ms pk",  format!("{:>7.2}", self.peak_frame_ms)),
            ("Entities",     format!("{:>7}", s.entity_count)),
            ("Particles",    format!("{:>7}", s.particle_count)),
            ("Glyphs",       format!("{:>7}", s.glyph_count)),
            ("Memory",       format!("{:>6.1}K", s.memory_estimate as f64 / 1024.0)),
            ("Frames",       format!("{:>7}", self.frame_count)),
        ];

        // Compute column widths
        let key_width = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(10);
        let val_width = rows.iter().map(|(_, v)| v.len()).max().unwrap_or(7);
        let total_inner = key_width + 3 + val_width; // " │ "

        let top    = format!("╔{}╗", "═".repeat(total_inner + 2));
        let title  = format!("║ {:<width$} ║", "Performance Dashboard", width = total_inner);
        let sep    = format!("╠{}╣", "═".repeat(total_inner + 2));
        let bottom = format!("╚{}╝", "═".repeat(total_inner + 2));

        let mut lines = vec![top, title, sep];

        for (key, val) in &rows {
            lines.push(format!("║ {:<kw$} │ {:<vw$} ║", key, val, kw = key_width, vw = val_width));
        }

        // Extra rows
        for (key, val) in &s.extras {
            lines.push(format!("║ {:<kw$} │ {:<vw$} ║", key, val, kw = key_width, vw = val_width));
        }

        lines.push(bottom);
        lines.join("\n")
    }

    /// Format a compact single-line summary.
    pub fn format_line(&self) -> String {
        let s = &self.last_snapshot;
        format!(
            "FPS:{:.0} dt:{:.1}ms E:{} P:{} G:{} M:{:.0}K",
            s.fps, s.frame_time_ms,
            s.entity_count, s.particle_count, s.glyph_count,
            s.memory_estimate as f64 / 1024.0,
        )
    }
}

impl Default for PerformanceDashboard {
    fn default() -> Self { Self::new() }
}

// ── MemoryTracker ─────────────────────────────────────────────────────────────

/// Tracks per-category memory allocations with explicit alloc/free calls.
///
/// This is not a general allocator hook; it records explicit calls from
/// subsystems that want to track their approximate heap usage.
pub struct MemoryTracker {
    categories: HashMap<String, CategoryStats>,
}

#[derive(Debug, Clone, Default)]
struct CategoryStats {
    current: usize,
    peak:    usize,
    total_alloc: u64,
    total_free:  u64,
    alloc_count: u64,
    free_count:  u64,
}

impl MemoryTracker {
    pub fn new() -> Self {
        Self { categories: HashMap::new() }
    }

    /// Record an allocation of `bytes` bytes in `category`.
    pub fn alloc(&mut self, category: &str, bytes: usize) {
        let s = self.categories.entry(category.to_owned()).or_default();
        s.current     += bytes;
        s.total_alloc += bytes as u64;
        s.alloc_count += 1;
        if s.current > s.peak { s.peak = s.current; }
    }

    /// Record a free of `bytes` bytes in `category`.
    pub fn free(&mut self, category: &str, bytes: usize) {
        let s = self.categories.entry(category.to_owned()).or_default();
        s.current     = s.current.saturating_sub(bytes);
        s.total_free += bytes as u64;
        s.free_count += 1;
    }

    /// Total bytes currently tracked across all categories.
    pub fn total(&self) -> usize {
        self.categories.values().map(|s| s.current).sum()
    }

    /// Peak total bytes seen across all categories at any single point.
    pub fn peak_total(&self) -> usize {
        self.categories.values().map(|s| s.peak).sum()
    }

    /// Per-category report sorted by current usage (descending).
    pub fn report_by_category(&self) -> Vec<(String, usize)> {
        let mut rows: Vec<(String, usize)> = self.categories.iter()
            .map(|(k, v)| (k.clone(), v.current))
            .collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1));
        rows
    }

    /// Detailed per-category report including peak and alloc counts.
    pub fn detailed_report(&self) -> Vec<CategoryReport> {
        let mut rows: Vec<CategoryReport> = self.categories.iter().map(|(k, v)| {
            CategoryReport {
                category:    k.clone(),
                current:     v.current,
                peak:        v.peak,
                total_alloc: v.total_alloc,
                total_free:  v.total_free,
                alloc_count: v.alloc_count,
                free_count:  v.free_count,
            }
        }).collect();
        rows.sort_by(|a, b| b.current.cmp(&a.current));
        rows
    }

    /// Format a human-readable report.
    pub fn format_report(&self) -> String {
        let mut lines = vec!["=== Memory Tracker ===".to_owned()];
        lines.push(format!("Total: {} bytes  Peak: {} bytes", self.total(), self.peak_total()));
        for (cat, bytes) in self.report_by_category() {
            lines.push(format!("  {:24} {:>10} bytes", cat, bytes));
        }
        lines.join("\n")
    }

    /// Reset all tracking data.
    pub fn reset(&mut self) {
        self.categories.clear();
    }

    /// Reset a specific category.
    pub fn reset_category(&mut self, category: &str) {
        self.categories.remove(category);
    }
}

/// Detailed per-category memory statistics.
#[derive(Debug, Clone)]
pub struct CategoryReport {
    pub category:    String,
    pub current:     usize,
    pub peak:        usize,
    pub total_alloc: u64,
    pub total_free:  u64,
    pub alloc_count: u64,
    pub free_count:  u64,
}

impl Default for MemoryTracker {
    fn default() -> Self { Self::new() }
}

// ── TimeSeries ────────────────────────────────────────────────────────────────

/// A simple fixed-capacity ring buffer of (timestamp_ms, f64) samples.
#[derive(Debug, Clone)]
pub struct TimeSeries {
    samples:  Vec<(u64, f64)>,
    head:     usize,
    capacity: usize,
    count:    usize,
}

impl TimeSeries {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples:  vec![(0, 0.0); capacity.max(1)],
            head:     0,
            capacity: capacity.max(1),
            count:    0,
        }
    }

    /// Push a new sample with the current timestamp.
    pub fn push(&mut self, value: f64) {
        self.samples[self.head] = (now_ms(), value);
        self.head  = (self.head + 1) % self.capacity;
        self.count = (self.count + 1).min(self.capacity);
    }

    /// Push a sample with an explicit timestamp.
    pub fn push_at(&mut self, ts_ms: u64, value: f64) {
        self.samples[self.head] = (ts_ms, value);
        self.head  = (self.head + 1) % self.capacity;
        self.count = (self.count + 1).min(self.capacity);
    }

    /// Iterate over samples in chronological order.
    pub fn iter(&self) -> impl Iterator<Item = (u64, f64)> + '_ {
        let start = if self.count < self.capacity { 0 } else { self.head };
        (0..self.count).map(move |i| self.samples[(start + i) % self.capacity])
    }

    /// Latest value, or 0.0 if empty.
    pub fn latest(&self) -> f64 {
        if self.count == 0 { return 0.0; }
        let idx = if self.head == 0 { self.capacity - 1 } else { self.head - 1 };
        self.samples[idx].1
    }

    pub fn len(&self) -> usize { self.count }
    pub fn is_empty(&self) -> bool { self.count == 0 }
}

// ── AggregateStats ────────────────────────────────────────────────────────────

/// Compute summary statistics over a slice of f64 values.
#[derive(Debug, Clone)]
pub struct AggregateStats {
    pub min:    f64,
    pub max:    f64,
    pub mean:   f64,
    pub std_dev: f64,
    pub p50:    f64,
    pub p95:    f64,
    pub p99:    f64,
    pub count:  usize,
}

impl AggregateStats {
    pub fn compute(values: &[f64]) -> Option<Self> {
        if values.is_empty() { return None; }
        let count = values.len();
        let min   = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max   = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let sum: f64 = values.iter().sum();
        let mean  = sum / count as f64;
        let var   = values.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / count as f64;
        let std_dev = var.sqrt();

        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let percentile = |p: f64| -> f64 {
            let rank = p * (count - 1) as f64;
            let lo = rank.floor() as usize;
            let hi = rank.ceil() as usize;
            let frac = rank - lo as f64;
            if lo == hi { return sorted[lo]; }
            sorted[lo] * (1.0 - frac) + sorted[hi] * frac
        };

        Some(Self { min, max, mean, std_dev, p50: percentile(0.5), p95: percentile(0.95), p99: percentile(0.99), count })
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_increments() {
        let reg = MetricsRegistry::new();
        reg.counter("requests", HashMap::new());
        reg.counter("requests", HashMap::new());
        reg.counter("requests", HashMap::new());
        assert_eq!(reg.get_counter("requests", &HashMap::new()), 3);
    }

    #[test]
    fn counter_by_delta() {
        let reg = MetricsRegistry::new();
        reg.counter_by("bytes", HashMap::new(), 1024);
        reg.counter_by("bytes", HashMap::new(), 512);
        assert_eq!(reg.get_counter("bytes", &HashMap::new()), 1536);
    }

    #[test]
    fn gauge_set_and_get() {
        let reg = MetricsRegistry::new();
        reg.gauge("temperature", HashMap::new(), 98.6);
        assert!((reg.get_gauge("temperature", &HashMap::new()) - 98.6).abs() < 1e-9);
    }

    #[test]
    fn gauge_add() {
        let reg = MetricsRegistry::new();
        reg.gauge("level", HashMap::new(), 10.0);
        reg.gauge_add("level", HashMap::new(), 5.0);
        assert!((reg.get_gauge("level", &HashMap::new()) - 15.0).abs() < 1e-9);
    }

    #[test]
    fn snapshot_contains_all_metrics() {
        let reg = MetricsRegistry::new();
        reg.counter("c1", HashMap::new());
        reg.gauge("g1", HashMap::new(), 1.0);
        reg.histogram_observe("h1", HashMap::new(), 0.5);
        let snap = reg.snapshot();
        assert!(snap.len() >= 3);
    }

    #[test]
    fn histogram_buckets_percentile() {
        let mut h = HistogramBuckets::latency_ms();
        for v in [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0] {
            h.observe(v);
        }
        let p50 = h.percentile(0.5);
        assert!(p50 >= 5.0 && p50 <= 6.0, "p50={}", p50);
        let p90 = h.percentile(0.9);
        assert!(p90 >= 9.0, "p90={}", p90);
    }

    #[test]
    fn histogram_mean_and_std_dev() {
        let mut h = HistogramBuckets::new(vec![10.0, 100.0]);
        for v in [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0] {
            h.observe(v);
        }
        let mean = h.mean();
        assert!((mean - 5.0).abs() < 0.01, "mean={}", mean);
        let sd = h.std_dev();
        assert!(sd > 0.0, "std_dev should be positive");
    }

    #[test]
    fn rolling_counter_rate() {
        let mut rc = RollingCounter::new(1.0);
        for _ in 0..100 { rc.increment(); }
        assert_eq!(rc.total(), 100);
        // Rate should be > 0 since events just happened
        assert!(rc.rate() > 0.0);
    }

    #[test]
    fn ema_convergence() {
        let mut ema = ExponentialMovingAverage::new(0.5);
        // Feed many samples of 10.0; EMA should converge to 10.0
        for _ in 0..30 { ema.update(10.0); }
        assert!((ema.get() - 10.0).abs() < 0.01, "EMA={}", ema.get());
    }

    #[test]
    fn ema_with_samples() {
        let mut ema = ExponentialMovingAverage::with_samples(10.0);
        for _ in 0..50 { ema.update(5.0); }
        assert!((ema.get() - 5.0).abs() < 0.01);
    }

    #[test]
    fn memory_tracker_alloc_free() {
        let mut tracker = MemoryTracker::new();
        tracker.alloc("textures", 1024);
        tracker.alloc("textures", 2048);
        tracker.free("textures", 1024);
        assert_eq!(tracker.total(), 2048);
        let report = tracker.report_by_category();
        assert_eq!(report[0].0, "textures");
        assert_eq!(report[0].1, 2048);
    }

    #[test]
    fn memory_tracker_peak() {
        let mut tracker = MemoryTracker::new();
        tracker.alloc("verts", 4096);
        tracker.alloc("verts", 4096);
        tracker.free("verts", 8192);
        assert_eq!(tracker.peak_total(), 8192);
        assert_eq!(tracker.total(), 0);
    }

    #[test]
    fn performance_dashboard_update() {
        let mut dash = PerformanceDashboard::new();
        dash.update(EngineSnapshot {
            fps:             60.0,
            frame_time_ms:   16.7,
            entity_count:    100,
            particle_count:  500,
            glyph_count:     2000,
            memory_estimate: 1024 * 1024,
            extras:          vec![],
        });
        let table = dash.format_table();
        assert!(table.contains("60"), "table should contain fps=60");
        assert!(table.contains("╔"), "table should have box-drawing chars");
        assert!(table.contains("╚"), "table should have box-drawing chars");
    }

    #[test]
    fn metrics_exporter_counter() {
        let reg = Arc::new(MetricsRegistry::new());
        reg.counter("http_requests", HashMap::new());
        let exporter = MetricsExporter::new(Arc::clone(&reg));
        let out = exporter.export();
        assert!(out.contains("http_requests"), "export should mention metric name");
        assert!(out.contains("# TYPE"), "should have type annotation");
    }

    #[test]
    fn aggregate_stats() {
        let vals = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = AggregateStats::compute(&vals).unwrap();
        assert_eq!(stats.mean, 3.0);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
    }

    #[test]
    fn time_series_ring_buffer() {
        let mut ts = TimeSeries::new(5);
        for i in 0..8u64 { ts.push(i as f64); }
        assert_eq!(ts.len(), 5);
        assert_eq!(ts.latest(), 7.0);
    }

    #[test]
    fn metrics_with_labels() {
        let reg = MetricsRegistry::new();
        let mut labels_a = HashMap::new();
        labels_a.insert("method".to_owned(), "GET".to_owned());
        let mut labels_b = HashMap::new();
        labels_b.insert("method".to_owned(), "POST".to_owned());
        reg.counter("requests", labels_a.clone());
        reg.counter("requests", labels_a.clone());
        reg.counter("requests", labels_b.clone());
        assert_eq!(reg.get_counter("requests", &labels_a), 2);
        assert_eq!(reg.get_counter("requests", &labels_b), 1);
    }
}
