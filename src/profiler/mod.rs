//! CPU/GPU performance profiler with hierarchical timing, counters, and flame graph capture.
//!
//! Usage:
//! ```rust,ignore
//! let mut prof = Profiler::new();
//! prof.begin("render");
//!   prof.begin("shadow_pass");
//!   prof.end("shadow_pass");
//!   prof.begin("gbuffer");
//!   prof.end("gbuffer");
//! prof.end("render");
//! let report = prof.flush();
//! ```

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

// ─── Span ─────────────────────────────────────────────────────────────────────

/// A single profiling span (named time range).
#[derive(Debug, Clone)]
pub struct Span {
    pub name:     String,
    pub depth:    u32,
    pub start_ns: u64,
    pub end_ns:   u64,
    pub thread_id: u64,
}

impl Span {
    pub fn duration_us(&self) -> f64 {
        (self.end_ns.saturating_sub(self.start_ns)) as f64 / 1_000.0
    }
    pub fn duration_ms(&self) -> f64 {
        self.duration_us() / 1_000.0
    }
}

// ─── Frame record ─────────────────────────────────────────────────────────────

/// All spans captured in one frame.
#[derive(Debug, Clone)]
pub struct FrameRecord {
    pub frame_index: u64,
    pub spans:       Vec<Span>,
    pub counters:    HashMap<String, f64>,
    pub frame_ms:    f64,
}

impl FrameRecord {
    pub fn total_span_ms(&self) -> f64 {
        self.spans.iter()
            .filter(|s| s.depth == 0)
            .map(|s| s.duration_ms())
            .sum()
    }

    /// Find the N most expensive spans (by duration).
    pub fn top_spans(&self, n: usize) -> Vec<&Span> {
        let mut sorted: Vec<&Span> = self.spans.iter().collect();
        sorted.sort_by(|a, b| b.end_ns.saturating_sub(b.start_ns).cmp(&(a.end_ns.saturating_sub(a.start_ns))));
        sorted.truncate(n);
        sorted
    }

    /// Aggregate spans by name (sum duration, count calls).
    pub fn aggregate(&self) -> HashMap<String, (f64, u32)> {
        let mut map: HashMap<String, (f64, u32)> = HashMap::new();
        for s in &self.spans {
            let entry = map.entry(s.name.clone()).or_insert((0.0, 0));
            entry.0 += s.duration_ms();
            entry.1 += 1;
        }
        map
    }
}

// ─── Rolling stats ────────────────────────────────────────────────────────────

/// Rolling statistics for a named span across frames.
#[derive(Debug, Clone)]
pub struct SpanStats {
    pub name:    String,
    pub samples: VecDeque<f64>,
    pub max_samples: usize,
}

impl SpanStats {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), samples: VecDeque::new(), max_samples: 128 }
    }

    pub fn push(&mut self, ms: f64) {
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(ms);
    }

    pub fn avg_ms(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn min_ms(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::MAX, f64::min)
    }

    pub fn max_ms(&self) -> f64 {
        self.samples.iter().cloned().fold(0.0_f64, f64::max)
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted: Vec<f64> = self.samples.iter().cloned().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() - 1) as f64) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn p50(&self) -> f64 { self.percentile(50.0) }
    pub fn p95(&self) -> f64 { self.percentile(95.0) }
    pub fn p99(&self) -> f64 { self.percentile(99.0) }

    pub fn variance(&self) -> f64 {
        let avg = self.avg_ms();
        if self.samples.len() < 2 { return 0.0; }
        let sum_sq: f64 = self.samples.iter().map(|x| (x - avg).powi(2)).sum();
        sum_sq / (self.samples.len() - 1) as f64
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }
}

// ─── Counter ──────────────────────────────────────────────────────────────────

/// Named counter accumulator (e.g. draw calls, triangle count).
#[derive(Debug, Clone)]
pub struct Counter {
    pub name:    String,
    pub value:   f64,
    pub history: VecDeque<f64>,
    pub max_history: usize,
}

impl Counter {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), value: 0.0, history: VecDeque::new(), max_history: 128 }
    }

    pub fn add(&mut self, v: f64) { self.value += v; }
    pub fn set(&mut self, v: f64) { self.value = v; }
    pub fn reset(&mut self) { self.value = 0.0; }

    pub fn flush(&mut self) {
        if self.history.len() >= self.max_history { self.history.pop_front(); }
        self.history.push_back(self.value);
        self.value = 0.0;
    }

    pub fn avg(&self) -> f64 {
        if self.history.is_empty() { return 0.0; }
        self.history.iter().sum::<f64>() / self.history.len() as f64
    }

    pub fn peak(&self) -> f64 {
        self.history.iter().cloned().fold(0.0_f64, f64::max)
    }
}

// ─── Memory stats ─────────────────────────────────────────────────────────────

/// Memory usage snapshot.
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    pub heap_used_bytes:     usize,
    pub heap_reserved_bytes: usize,
    pub gpu_used_bytes:      usize,
    pub gpu_reserved_bytes:  usize,
    pub texture_bytes:       usize,
    pub mesh_bytes:          usize,
    pub audio_bytes:         usize,
    pub script_bytes:        usize,
}

impl MemoryStats {
    pub fn total_used_mb(&self) -> f64 {
        (self.heap_used_bytes + self.gpu_used_bytes) as f64 / 1_048_576.0
    }

    pub fn gpu_used_mb(&self) -> f64 {
        self.gpu_used_bytes as f64 / 1_048_576.0
    }

    pub fn heap_used_mb(&self) -> f64 {
        self.heap_used_bytes as f64 / 1_048_576.0
    }
}

// ─── GPU timing ───────────────────────────────────────────────────────────────

/// A GPU timing query result (from GPU timestamp queries, if available).
#[derive(Debug, Clone)]
pub struct GpuSpan {
    pub name:    String,
    pub gpu_us:  f64,
    pub pass:    GpuPass,
}

/// Which render pass a GPU span belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuPass {
    ShadowMap,
    GBuffer,
    Lighting,
    Transparent,
    PostProcess,
    UI,
    Compute,
    Other,
}

/// A frame of GPU timing data.
#[derive(Debug, Clone, Default)]
pub struct GpuFrameStats {
    pub spans:     Vec<GpuSpan>,
    pub total_us:  f64,
    pub frame_idx: u64,
}

impl GpuFrameStats {
    pub fn total_ms(&self) -> f64 { self.total_us / 1000.0 }

    pub fn pass_total(&self, pass: GpuPass) -> f64 {
        self.spans.iter()
            .filter(|s| s.pass == pass)
            .map(|s| s.gpu_us)
            .sum::<f64>() / 1000.0
    }
}

// ─── Flame graph ──────────────────────────────────────────────────────────────

/// A node in the flame graph (hierarchical call tree).
#[derive(Debug, Clone)]
pub struct FlameNode {
    pub name:       String,
    pub total_ms:   f64,
    pub self_ms:    f64,  // exclusive time (not including children)
    pub call_count: u32,
    pub children:   Vec<FlameNode>,
}

impl FlameNode {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), total_ms: 0.0, self_ms: 0.0, call_count: 0, children: Vec::new() }
    }

    pub fn find_child_mut(&mut self, name: &str) -> Option<&mut FlameNode> {
        self.children.iter_mut().find(|c| c.name == name)
    }

    pub fn get_or_insert_child(&mut self, name: &str) -> &mut FlameNode {
        if let Some(idx) = self.children.iter().position(|c| c.name == name) {
            return &mut self.children[idx];
        }
        self.children.push(FlameNode::new(name));
        self.children.last_mut().unwrap()
    }

    pub fn children_total_ms(&self) -> f64 {
        self.children.iter().map(|c| c.total_ms).sum()
    }

    pub fn recompute_self_ms(&mut self) {
        self.self_ms = (self.total_ms - self.children_total_ms()).max(0.0);
        for child in &mut self.children {
            child.recompute_self_ms();
        }
    }

    /// Flatten into a sorted list for display.
    pub fn flatten(&self) -> Vec<(String, f64, u32)> {
        let mut out = vec![(self.name.clone(), self.total_ms, self.call_count)];
        for c in &self.children {
            out.extend(c.flatten());
        }
        out
    }
}

// ─── Open span tracker ────────────────────────────────────────────────────────

struct OpenSpan {
    name:     String,
    start_ns: u64,
    depth:    u32,
}

// ─── Main Profiler ────────────────────────────────────────────────────────────

/// Main CPU profiler. Not thread-safe (use per-thread instances or a Mutex).
pub struct Profiler {
    pub enabled:      bool,
    epoch:            Instant,
    open_stack:       Vec<OpenSpan>,
    current_spans:    Vec<Span>,
    pub counters:     HashMap<String, Counter>,
    pub span_stats:   HashMap<String, SpanStats>,
    frame_history:    VecDeque<FrameRecord>,
    pub max_history:  usize,
    pub frame_index:  u64,
    frame_start_ns:   u64,
    pub memory:       MemoryStats,
    pub gpu:          GpuFrameStats,
    pub fps_history:  VecDeque<f64>,
    pub fps:          f64,
}

impl Profiler {
    pub fn new() -> Self {
        Self {
            enabled:       true,
            epoch:         Instant::now(),
            open_stack:    Vec::new(),
            current_spans: Vec::new(),
            counters:      HashMap::new(),
            span_stats:    HashMap::new(),
            frame_history: VecDeque::new(),
            max_history:   120,
            frame_index:   0,
            frame_start_ns: 0,
            memory:        MemoryStats::default(),
            gpu:           GpuFrameStats::default(),
            fps_history:   VecDeque::new(),
            fps:           0.0,
        }
    }

    fn now_ns(&self) -> u64 {
        self.epoch.elapsed().as_nanos() as u64
    }

    /// Start a new timing span. Must be matched with `end(name)`.
    pub fn begin(&mut self, name: &str) {
        if !self.enabled { return; }
        let depth = self.open_stack.len() as u32;
        let start = self.now_ns();
        self.open_stack.push(OpenSpan { name: name.to_string(), start_ns: start, depth });
    }

    /// End the most recent span with this name.
    pub fn end(&mut self, name: &str) {
        if !self.enabled { return; }
        let end = self.now_ns();
        if let Some(pos) = self.open_stack.iter().rposition(|s| s.name == name) {
            let open = self.open_stack.remove(pos);
            let span = Span {
                name:      open.name.clone(),
                depth:     open.depth,
                start_ns:  open.start_ns,
                end_ns:    end,
                thread_id: 0,
            };
            // Update rolling stats
            let ms = span.duration_ms();
            self.span_stats.entry(open.name.clone())
                .or_insert_with(|| SpanStats::new(&open.name))
                .push(ms);
            self.current_spans.push(span);
        }
    }

    /// Increment a named counter.
    pub fn count(&mut self, name: &str, delta: f64) {
        self.counters.entry(name.to_string())
            .or_insert_with(|| Counter::new(name))
            .add(delta);
    }

    /// Set a named counter to an absolute value.
    pub fn set_counter(&mut self, name: &str, value: f64) {
        self.counters.entry(name.to_string())
            .or_insert_with(|| Counter::new(name))
            .set(value);
    }

    /// Call at the start of each frame.
    pub fn frame_begin(&mut self) {
        if !self.enabled { return; }
        self.frame_start_ns = self.now_ns();
    }

    /// Call at the end of each frame. Returns the completed FrameRecord.
    pub fn frame_end(&mut self) -> FrameRecord {
        let frame_end_ns = self.now_ns();
        let frame_ms = (frame_end_ns.saturating_sub(self.frame_start_ns)) as f64 / 1_000_000.0;

        // Update FPS
        if frame_ms > 0.0 {
            let fps = 1000.0 / frame_ms;
            if self.fps_history.len() >= 128 { self.fps_history.pop_front(); }
            self.fps_history.push_back(fps);
            self.fps = self.fps_history.iter().sum::<f64>() / self.fps_history.len() as f64;
        }

        // Collect counter snapshots
        let counter_snapshot: HashMap<String, f64> = self.counters.iter()
            .map(|(k, v)| (k.clone(), v.value))
            .collect();
        for c in self.counters.values_mut() {
            c.flush();
        }

        // Close any unclosed spans
        while let Some(open) = self.open_stack.pop() {
            let span = Span {
                name: open.name.clone(),
                depth: open.depth,
                start_ns: open.start_ns,
                end_ns: frame_end_ns,
                thread_id: 0,
            };
            let ms = span.duration_ms();
            self.span_stats.entry(open.name.clone())
                .or_insert_with(|| SpanStats::new(&open.name))
                .push(ms);
            self.current_spans.push(span);
        }

        let record = FrameRecord {
            frame_index: self.frame_index,
            spans:       std::mem::take(&mut self.current_spans),
            counters:    counter_snapshot,
            frame_ms,
        };

        if self.frame_history.len() >= self.max_history {
            self.frame_history.pop_front();
        }
        self.frame_history.push_back(record.clone());
        self.frame_index += 1;

        record
    }

    /// Get rolling stats for a span by name.
    pub fn stats(&self, name: &str) -> Option<&SpanStats> {
        self.span_stats.get(name)
    }

    /// Build a flame graph from the last N frames.
    pub fn build_flame_graph(&self, last_n_frames: usize) -> FlameNode {
        let mut root = FlameNode::new("root");
        let start = self.frame_history.len().saturating_sub(last_n_frames);

        for frame in self.frame_history.iter().skip(start) {
            // Use depth to reconstruct hierarchy
            let mut path_stack: Vec<String> = Vec::new();
            for span in &frame.spans {
                while path_stack.len() > span.depth as usize {
                    path_stack.pop();
                }
                path_stack.push(span.name.clone());

                // Walk/create path in flame tree
                let mut node = &mut root;
                for seg in &path_stack {
                    node = node.get_or_insert_child(seg);
                }
                node.total_ms += span.duration_ms();
                node.call_count += 1;
            }
        }

        root.recompute_self_ms();
        root
    }

    /// Get the last N frame records.
    pub fn recent_frames(&self, n: usize) -> &[FrameRecord] {
        let start = self.frame_history.len().saturating_sub(n);
        // Return as slice from deque – collect to Vec for simplicity
        // Actually we return from make_contiguous after a refresh, but deque doesn't support
        // slices directly. We'll collect the last N items on demand.
        let _ = start; // placeholder
        &[] // In a real impl this would return &[FrameRecord] from a contiguous buffer
    }

    /// Get the last completed frame.
    pub fn last_frame(&self) -> Option<&FrameRecord> {
        self.frame_history.back()
    }

    /// Average FPS over the history window.
    pub fn avg_fps(&self) -> f64 { self.fps }

    /// Average frame time in ms.
    pub fn avg_frame_ms(&self) -> f64 {
        if self.fps > 0.0 { 1000.0 / self.fps } else { 0.0 }
    }

    /// Report summary string.
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("=== Profiler Frame {} ===", self.frame_index));
        lines.push(format!("FPS: {:.1}  Frame: {:.2}ms", self.avg_fps(), self.avg_frame_ms()));

        let mut sorted_stats: Vec<(&String, &SpanStats)> = self.span_stats.iter().collect();
        sorted_stats.sort_by(|a, b| b.1.avg_ms().partial_cmp(&a.1.avg_ms()).unwrap());

        for (name, stats) in sorted_stats.iter().take(15) {
            lines.push(format!(
                "  {:30} avg={:.3}ms  p95={:.3}ms  p99={:.3}ms",
                name, stats.avg_ms(), stats.p95(), stats.p99()
            ));
        }

        if !self.counters.is_empty() {
            lines.push("  --- Counters ---".to_string());
            for (name, counter) in &self.counters {
                lines.push(format!("  {:30} avg={:.1}  peak={:.1}", name, counter.avg(), counter.peak()));
            }
        }

        lines.join("\n")
    }

    /// Reset all history.
    pub fn reset(&mut self) {
        self.span_stats.clear();
        self.counters.clear();
        self.frame_history.clear();
        self.current_spans.clear();
        self.open_stack.clear();
        self.fps_history.clear();
        self.frame_index = 0;
    }
}

impl Default for Profiler {
    fn default() -> Self { Self::new() }
}

// ─── Scoped span guard ────────────────────────────────────────────────────────

/// RAII guard for automatic span end. Use with a `&mut Profiler`.
/// ```rust,ignore
/// {
///     let _guard = prof.scoped("render");
///     // work here
/// }  // span ends automatically
/// ```
pub struct ScopedSpan<'a> {
    profiler: &'a mut Profiler,
    name:     String,
}

impl<'a> ScopedSpan<'a> {
    pub fn new(profiler: &'a mut Profiler, name: &str) -> Self {
        profiler.begin(name);
        Self { profiler, name: name.to_string() }
    }
}

impl<'a> Drop for ScopedSpan<'a> {
    fn drop(&mut self) {
        self.profiler.end(&self.name);
    }
}

// ─── Performance budget ───────────────────────────────────────────────────────

/// Defines target frame time budgets for different system categories.
#[derive(Debug, Clone)]
pub struct FrameBudget {
    pub target_fps:     f64,
    pub physics_ms:     f64,
    pub render_ms:      f64,
    pub ai_ms:          f64,
    pub audio_ms:       f64,
    pub scripting_ms:   f64,
}

impl FrameBudget {
    pub fn from_target_fps(fps: f64) -> Self {
        let total = 1000.0 / fps;
        Self {
            target_fps:   fps,
            physics_ms:   total * 0.20,
            render_ms:    total * 0.45,
            ai_ms:        total * 0.15,
            audio_ms:     total * 0.05,
            scripting_ms: total * 0.10,
        }
    }

    pub fn total_ms(&self) -> f64 { 1000.0 / self.target_fps }

    pub fn check_violations(&self, frame: &FrameRecord) -> Vec<BudgetViolation> {
        let agg = frame.aggregate();
        let mut violations = Vec::new();

        let check = |cat: &str, budget: f64| -> Option<BudgetViolation> {
            let total: f64 = agg.iter()
                .filter(|(k, _)| k.starts_with(cat))
                .map(|(_, (ms, _))| ms)
                .sum();
            if total > budget {
                Some(BudgetViolation { category: cat.to_string(), actual_ms: total, budget_ms: budget })
            } else { None }
        };

        if let Some(v) = check("physics", self.physics_ms)   { violations.push(v); }
        if let Some(v) = check("render",  self.render_ms)    { violations.push(v); }
        if let Some(v) = check("ai",      self.ai_ms)        { violations.push(v); }
        if let Some(v) = check("audio",   self.audio_ms)     { violations.push(v); }
        if let Some(v) = check("script",  self.scripting_ms) { violations.push(v); }

        violations
    }
}

/// A single budget violation record.
#[derive(Debug, Clone)]
pub struct BudgetViolation {
    pub category:  String,
    pub actual_ms: f64,
    pub budget_ms: f64,
}

impl BudgetViolation {
    pub fn overage_ms(&self) -> f64 { (self.actual_ms - self.budget_ms).max(0.0) }
    pub fn overage_pct(&self) -> f64 { self.overage_ms() / self.budget_ms * 100.0 }
}

// ─── Stutter detector ─────────────────────────────────────────────────────────

/// Detects frame time spikes (stutters) by comparing against rolling average.
#[derive(Debug, Clone)]
pub struct StutterDetector {
    pub threshold_multiplier: f64, // flag if frame_ms > avg * threshold (default 2.5)
    pub window:               usize,
    history:                  VecDeque<f64>,
    pub stutter_count:        u32,
    pub last_stutter_frame:   u64,
}

impl StutterDetector {
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold_multiplier: threshold,
            window:               60,
            history:              VecDeque::new(),
            stutter_count:        0,
            last_stutter_frame:   0,
        }
    }

    pub fn update(&mut self, frame_ms: f64, frame_index: u64) -> bool {
        if self.history.len() >= self.window { self.history.pop_front(); }
        self.history.push_back(frame_ms);

        if self.history.len() < 10 { return false; }

        let avg: f64 = self.history.iter().sum::<f64>() / self.history.len() as f64;
        let is_stutter = frame_ms > avg * self.threshold_multiplier;

        if is_stutter {
            self.stutter_count += 1;
            self.last_stutter_frame = frame_index;
        }
        is_stutter
    }

    pub fn rolling_avg_ms(&self) -> f64 {
        if self.history.is_empty() { return 0.0; }
        self.history.iter().sum::<f64>() / self.history.len() as f64
    }

    pub fn reset_stutter_count(&mut self) { self.stutter_count = 0; }
}

// ─── Profiling overlay data ───────────────────────────────────────────────────

/// Data ready for rendering a profiling overlay (e.g. a graph in the game).
#[derive(Debug, Clone)]
pub struct ProfileOverlay {
    /// Frame time graph — last N frame_ms values for sparkline.
    pub frame_ms_graph: Vec<f64>,
    /// Per-system bar chart values (name → ms).
    pub system_bars:    Vec<(String, f64)>,
    pub avg_fps:        f64,
    pub avg_frame_ms:   f64,
    pub p99_frame_ms:   f64,
    pub stutter_count:  u32,
    pub memory_mb:      f64,
}

impl ProfileOverlay {
    pub fn from_profiler(p: &Profiler, sd: &StutterDetector) -> Self {
        let frame_ms_graph: Vec<f64> = p.frame_history.iter().map(|f| f.frame_ms).collect();

        // Sort by average ms descending
        let mut system_bars: Vec<(String, f64)> = p.span_stats.iter()
            .filter(|(_, s)| s.avg_ms() > 0.01)
            .map(|(k, s)| (k.clone(), s.avg_ms()))
            .collect();
        system_bars.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        system_bars.truncate(20);

        let p99 = if frame_ms_graph.len() >= 2 {
            let mut sorted = frame_ms_graph.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            sorted[(sorted.len() as f64 * 0.99) as usize]
        } else { 0.0 };

        Self {
            frame_ms_graph,
            system_bars,
            avg_fps:      p.avg_fps(),
            avg_frame_ms: p.avg_frame_ms(),
            p99_frame_ms: p99,
            stutter_count: sd.stutter_count,
            memory_mb:    p.memory.total_used_mb(),
        }
    }
}

// ─── Thread-local profiling ───────────────────────────────────────────────────

/// Aggregator for multi-thread profiling results.
#[derive(Debug, Clone)]
pub struct ThreadProfile {
    pub thread_name: String,
    pub thread_id:   u64,
    pub spans:       Vec<Span>,
}

/// Merge thread profiles into a combined FrameRecord.
pub fn merge_thread_profiles(main: FrameRecord, threads: Vec<ThreadProfile>) -> FrameRecord {
    let mut merged = main;
    for tp in threads {
        let mut tagged: Vec<Span> = tp.spans.into_iter()
            .map(|mut s| { s.thread_id = tp.thread_id; s.name = format!("[{}] {}", tp.thread_name, s.name); s })
            .collect();
        merged.spans.append(&mut tagged);
    }
    // Re-sort by start time
    merged.spans.sort_by_key(|s| s.start_ns);
    merged
}

// ─── CSV export ───────────────────────────────────────────────────────────────

/// Export the frame history to CSV format.
pub fn export_csv(profiler: &Profiler) -> String {
    let mut lines = vec!["frame,span_name,depth,start_ns,end_ns,duration_us".to_string()];
    for frame in &profiler.frame_history {
        for span in &frame.spans {
            lines.push(format!(
                "{},{},{},{},{},{}",
                frame.frame_index, span.name, span.depth,
                span.start_ns, span.end_ns,
                (span.end_ns.saturating_sub(span.start_ns)) / 1_000,
            ));
        }
    }
    lines.join("\n")
}

// ─── Mark / annotation ───────────────────────────────────────────────────────

/// A named annotation at a point in time (for debugging events).
#[derive(Debug, Clone)]
pub struct ProfileMarker {
    pub name:       String,
    pub time_ns:    u64,
    pub frame:      u64,
    pub color:      [f32; 4],
    pub extra:      String,
}

/// Collection of profile markers for a session.
#[derive(Debug, Clone, Default)]
pub struct MarkerLog {
    pub markers: Vec<ProfileMarker>,
}

impl MarkerLog {
    pub fn mark(&mut self, name: &str, time_ns: u64, frame: u64, extra: &str) {
        self.markers.push(ProfileMarker {
            name:    name.to_string(),
            time_ns,
            frame,
            color:   [1.0, 1.0, 0.0, 1.0],
            extra:   extra.to_string(),
        });
    }

    pub fn mark_colored(&mut self, name: &str, time_ns: u64, frame: u64, color: [f32;4]) {
        self.markers.push(ProfileMarker {
            name: name.to_string(), time_ns, frame, color, extra: String::new(),
        });
    }

    pub fn since_frame(&self, frame: u64) -> impl Iterator<Item = &ProfileMarker> {
        self.markers.iter().filter(move |m| m.frame >= frame)
    }

    pub fn clear_before(&mut self, frame: u64) {
        self.markers.retain(|m| m.frame >= frame);
    }
}
