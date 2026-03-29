// profiler.rs — CPU/GPU profiler with hierarchical spans, flame graph data,
// counter tracking, memory accounting, and budget alerts.

use std::collections::{HashMap, VecDeque};
use std::fmt;

// ─── Time primitives ─────────────────────────────────────────────────────────

/// Monotonic timestamp in microseconds
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub fn elapsed_since(&self, earlier: Timestamp) -> Duration {
        Duration(self.0.saturating_sub(earlier.0))
    }
}

/// Duration in microseconds
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Duration(pub u64);

impl Duration {
    pub const ZERO: Self = Self(0);

    pub fn from_us(us: u64) -> Self { Self(us) }
    pub fn from_ms(ms: f64) -> Self { Self((ms * 1000.0) as u64) }
    pub fn from_secs(s: f64) -> Self { Self((s * 1_000_000.0) as u64) }

    pub fn as_us(&self) -> u64 { self.0 }
    pub fn as_ms(&self) -> f64 { self.0 as f64 / 1000.0 }
    pub fn as_secs(&self) -> f64 { self.0 as f64 / 1_000_000.0 }

    pub fn add(self, other: Self) -> Self { Self(self.0 + other.0) }
    pub fn saturating_sub(self, other: Self) -> Self { Self(self.0.saturating_sub(other.0)) }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 < 1000 {
            write!(f, "{}µs", self.0)
        } else if self.0 < 1_000_000 {
            write!(f, "{:.2}ms", self.as_ms())
        } else {
            write!(f, "{:.3}s", self.as_secs())
        }
    }
}

// ─── Span ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpanId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    Cpu,
    Gpu,
    Memory,
    Io,
    Script,
    Physics,
    Render,
    Audio,
    Custom,
}

impl SpanKind {
    pub fn color_rgb(&self) -> [u8; 3] {
        match self {
            Self::Cpu     => [70, 130, 200],
            Self::Gpu     => [200, 100, 50],
            Self::Memory  => [100, 180, 80],
            Self::Io      => [180, 60, 180],
            Self::Script  => [220, 180, 40],
            Self::Physics => [60, 180, 180],
            Self::Render  => [220, 80, 80],
            Self::Audio   => [80, 200, 140],
            Self::Custom  => [160, 160, 160],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProfileSpan {
    pub id: SpanId,
    pub name: String,
    pub kind: SpanKind,
    pub start: Timestamp,
    pub end: Option<Timestamp>,
    pub parent: Option<SpanId>,
    pub depth: u32,
    pub thread: u32,
    pub extra: Option<String>,
}

impl ProfileSpan {
    pub fn duration(&self) -> Option<Duration> {
        self.end.map(|e| e.elapsed_since(self.start))
    }

    pub fn is_open(&self) -> bool { self.end.is_none() }

    pub fn contains(&self, other: &ProfileSpan) -> bool {
        if let (Some(self_end), Some(other_end)) = (self.end, other.end) {
            self.start <= other.start && self_end >= other_end
        } else {
            false
        }
    }
}

// ─── Frame data ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FrameProfile {
    pub frame_number: u64,
    pub frame_start: Timestamp,
    pub frame_end: Option<Timestamp>,
    pub spans: Vec<ProfileSpan>,
    pub counters: HashMap<String, CounterSample>,
    pub memory: MemorySnapshot,
    pub gpu_timestamp_start: Timestamp,
    pub gpu_timestamp_end: Option<Timestamp>,
}

impl FrameProfile {
    pub fn new(frame_number: u64, start: Timestamp) -> Self {
        Self {
            frame_number,
            frame_start: start,
            frame_end: None,
            spans: Vec::new(),
            counters: HashMap::new(),
            memory: MemorySnapshot::default(),
            gpu_timestamp_start: Timestamp::default(),
            gpu_timestamp_end: None,
        }
    }

    pub fn cpu_duration(&self) -> Option<Duration> {
        self.frame_end.map(|e| e.elapsed_since(self.frame_start))
    }

    pub fn gpu_duration(&self) -> Option<Duration> {
        self.gpu_timestamp_end.map(|e| e.elapsed_since(self.gpu_timestamp_start))
    }

    pub fn total_span_time(&self, kind: SpanKind) -> Duration {
        self.spans.iter()
            .filter(|s| s.kind == kind && s.parent.is_none())
            .filter_map(|s| s.duration())
            .fold(Duration::ZERO, |acc, d| acc.add(d))
    }

    pub fn slowest_span(&self) -> Option<&ProfileSpan> {
        self.spans.iter()
            .filter_map(|s| s.duration().map(|d| (s, d)))
            .max_by_key(|(_, d)| d.0)
            .map(|(s, _)| s)
    }

    pub fn spans_sorted_by_duration(&self) -> Vec<&ProfileSpan> {
        let mut s: Vec<&ProfileSpan> = self.spans.iter().collect();
        s.sort_by(|a, b| {
            let da = a.duration().unwrap_or(Duration::ZERO);
            let db = b.duration().unwrap_or(Duration::ZERO);
            db.cmp(&da)
        });
        s
    }

    pub fn build_flamegraph(&self) -> FlamegraphNode {
        let root_spans: Vec<&ProfileSpan> = self.spans.iter()
            .filter(|s| s.parent.is_none())
            .collect();
        FlamegraphNode {
            name: "Frame".into(),
            kind: SpanKind::Cpu,
            duration: self.cpu_duration().unwrap_or(Duration::ZERO),
            children: root_spans.iter().map(|s| self.build_flame_node(s)).collect(),
            depth: 0,
        }
    }

    fn build_flame_node(&self, span: &ProfileSpan) -> FlamegraphNode {
        let children: Vec<&ProfileSpan> = self.spans.iter()
            .filter(|s| s.parent == Some(span.id))
            .collect();
        FlamegraphNode {
            name: span.name.clone(),
            kind: span.kind,
            duration: span.duration().unwrap_or(Duration::ZERO),
            children: children.iter().map(|s| self.build_flame_node(s)).collect(),
            depth: span.depth,
        }
    }

    pub fn top_n_spans(&self, n: usize) -> Vec<(&ProfileSpan, Duration)> {
        let mut v: Vec<(&ProfileSpan, Duration)> = self.spans.iter()
            .filter_map(|s| s.duration().map(|d| (s, d)))
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.truncate(n);
        v
    }
}

// ─── Flame graph ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FlamegraphNode {
    pub name: String,
    pub kind: SpanKind,
    pub duration: Duration,
    pub children: Vec<FlamegraphNode>,
    pub depth: u32,
}

impl FlamegraphNode {
    pub fn self_time(&self) -> Duration {
        let children_total: u64 = self.children.iter()
            .map(|c| c.duration.0)
            .sum();
        Duration(self.duration.0.saturating_sub(children_total))
    }

    pub fn max_depth(&self) -> u32 {
        self.children.iter()
            .map(|c| c.max_depth())
            .max()
            .unwrap_or(0)
            + 1
    }

    pub fn total_span_count(&self) -> usize {
        1 + self.children.iter().map(|c| c.total_span_count()).sum::<usize>()
    }

    pub fn visit<F: FnMut(&FlamegraphNode, u32)>(&self, depth: u32, f: &mut F) {
        f(self, depth);
        for child in &self.children {
            child.visit(depth + 1, f);
        }
    }

    pub fn ascii_tree(&self, indent: usize) -> String {
        let mut out = format!(
            "{}{} ({})\n",
            "  ".repeat(indent),
            self.name,
            self.duration,
        );
        for child in &self.children {
            out.push_str(&child.ascii_tree(indent + 1));
        }
        out
    }
}

// ─── Counter / gauge ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CounterSample {
    pub name: String,
    pub value: f64,
    pub unit: CounterUnit,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterUnit {
    Count,
    Bytes,
    Milliseconds,
    Microseconds,
    Percentage,
    Hertz,
    Triangles,
    DrawCalls,
    Custom,
}

impl CounterUnit {
    pub fn format(&self, v: f64) -> String {
        match self {
            Self::Count      => format!("{:.0}", v),
            Self::Bytes      => format_bytes(v as u64),
            Self::Milliseconds  => format!("{:.2}ms", v),
            Self::Microseconds  => format!("{:.1}µs", v),
            Self::Percentage => format!("{:.1}%", v),
            Self::Hertz      => format!("{:.0}Hz", v),
            Self::Triangles  => format!("{}K tri", (v / 1000.0) as u64),
            Self::DrawCalls  => format!("{:.0} DC", v),
            Self::Custom     => format!("{:.3}", v),
        }
    }
}

fn format_bytes(b: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if b >= GB      { format!("{:.2} GB", b as f64 / GB as f64) }
    else if b >= MB { format!("{:.1} MB", b as f64 / MB as f64) }
    else if b >= KB { format!("{:.0} KB", b as f64 / KB as f64) }
    else            { format!("{} B", b) }
}

// ─── Memory snapshot ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct MemorySnapshot {
    pub heap_bytes: u64,
    pub stack_bytes: u64,
    pub gpu_vram_bytes: u64,
    pub texture_bytes: u64,
    pub vertex_buffer_bytes: u64,
    pub index_buffer_bytes: u64,
    pub uniform_buffer_bytes: u64,
    pub render_target_bytes: u64,
    pub audio_bytes: u64,
    pub script_bytes: u64,
    pub asset_cache_bytes: u64,
}

impl MemorySnapshot {
    pub fn total_cpu(&self) -> u64 {
        self.heap_bytes + self.stack_bytes + self.audio_bytes + self.script_bytes
    }

    pub fn total_gpu(&self) -> u64 {
        self.gpu_vram_bytes
    }

    pub fn total_gpu_breakdown(&self) -> u64 {
        self.texture_bytes + self.vertex_buffer_bytes + self.index_buffer_bytes
            + self.uniform_buffer_bytes + self.render_target_bytes
    }

    pub fn format_summary(&self) -> String {
        format!(
            "CPU: {} | GPU: {} (Tex:{} VB:{} RT:{})",
            format_bytes(self.total_cpu()),
            format_bytes(self.total_gpu()),
            format_bytes(self.texture_bytes),
            format_bytes(self.vertex_buffer_bytes),
            format_bytes(self.render_target_bytes),
        )
    }
}

// ─── Budget ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PerformanceBudget {
    pub name: String,
    pub frame_time_ms: f64,
    pub cpu_ms: f64,
    pub gpu_ms: f64,
    pub draw_calls: u32,
    pub triangle_count: u32,
    pub vram_bytes: u64,
    pub texture_bytes: u64,
}

impl Default for PerformanceBudget {
    fn default() -> Self {
        Self {
            name: "60 FPS".into(),
            frame_time_ms: 16.67,
            cpu_ms: 8.0,
            gpu_ms: 8.0,
            draw_calls: 500,
            triangle_count: 500_000,
            vram_bytes: 512 * 1024 * 1024,
            texture_bytes: 256 * 1024 * 1024,
        }
    }
}

impl PerformanceBudget {
    pub fn mobile() -> Self {
        Self {
            name: "30 FPS Mobile".into(),
            frame_time_ms: 33.33,
            cpu_ms: 12.0,
            gpu_ms: 12.0,
            draw_calls: 100,
            triangle_count: 100_000,
            vram_bytes: 128 * 1024 * 1024,
            texture_bytes: 64 * 1024 * 1024,
        }
    }

    pub fn hi_end() -> Self {
        Self {
            name: "120 FPS High-End".into(),
            frame_time_ms: 8.33,
            cpu_ms: 4.0,
            gpu_ms: 4.0,
            draw_calls: 2000,
            triangle_count: 2_000_000,
            vram_bytes: 4 * 1024 * 1024 * 1024,
            texture_bytes: 2 * 1024 * 1024 * 1024,
        }
    }

    pub fn check(&self, frame: &FrameProfile) -> BudgetReport {
        let mut violations = Vec::new();
        let frame_ms = frame.cpu_duration().unwrap_or(Duration::ZERO).as_ms();
        if frame_ms > self.frame_time_ms {
            violations.push(BudgetViolation {
                item: "Frame Time".into(),
                actual: frame_ms,
                budget: self.frame_time_ms,
                unit: CounterUnit::Milliseconds,
            });
        }
        BudgetReport {
            budget_name: self.name.clone(),
            frame_number: frame.frame_number,
            violations,
            within_budget: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BudgetViolation {
    pub item: String,
    pub actual: f64,
    pub budget: f64,
    pub unit: CounterUnit,
}

impl BudgetViolation {
    pub fn overage_pct(&self) -> f64 {
        if self.budget > 0.0 {
            (self.actual - self.budget) / self.budget * 100.0
        } else {
            0.0
        }
    }
}

impl fmt::Display for BudgetViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} / {} (+{:.1}%)",
            self.item,
            self.unit.format(self.actual),
            self.unit.format(self.budget),
            self.overage_pct(),
        )
    }
}

#[derive(Debug, Clone)]
pub struct BudgetReport {
    pub budget_name: String,
    pub frame_number: u64,
    pub violations: Vec<BudgetViolation>,
    pub within_budget: bool,
}

impl BudgetReport {
    pub fn is_clean(&self) -> bool { self.violations.is_empty() }
}

// ─── Ring buffer stats ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct StatRingBuffer {
    pub name: String,
    pub unit: CounterUnit,
    data: VecDeque<f64>,
    capacity: usize,
}

impl StatRingBuffer {
    pub fn new(name: &str, unit: CounterUnit, capacity: usize) -> Self {
        Self {
            name: name.to_string(),
            unit,
            data: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, value: f64) {
        if self.data.len() >= self.capacity {
            self.data.pop_front();
        }
        self.data.push_back(value);
    }

    pub fn mean(&self) -> f64 {
        if self.data.is_empty() { return 0.0; }
        self.data.iter().sum::<f64>() / self.data.len() as f64
    }

    pub fn min(&self) -> f64 {
        self.data.iter().cloned().fold(f64::MAX, f64::min)
    }

    pub fn max(&self) -> f64 {
        self.data.iter().cloned().fold(f64::MIN, f64::max)
    }

    pub fn percentile(&self, pct: f64) -> f64 {
        if self.data.is_empty() { return 0.0; }
        let mut sorted: Vec<f64> = self.data.iter().cloned().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((pct / 100.0) * (sorted.len() - 1) as f64) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn latest(&self) -> f64 {
        self.data.back().copied().unwrap_or(0.0)
    }

    pub fn sparkline(&self, width: usize, height: usize) -> Vec<Vec<bool>> {
        let mut grid = vec![vec![false; width]; height];
        if self.data.is_empty() { return grid; }
        let max_val = self.max().max(1e-10);
        let min_val = self.min();
        let range = (max_val - min_val).max(1e-10);
        let n = self.data.len();
        for (i, &v) in self.data.iter().enumerate() {
            let x = i * width / n.max(1);
            let y_norm = (v - min_val) / range;
            let y = ((1.0 - y_norm) * (height - 1) as f64) as usize;
            if x < width && y < height {
                grid[y][x] = true;
            }
        }
        grid
    }

    pub fn ascii_sparkline(&self, width: usize) -> String {
        const BARS: &[char] = &[' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        if self.data.is_empty() { return " ".repeat(width); }
        let max_val = self.max().max(1e-10);
        let n = self.data.len().min(width);
        let skip = self.data.len().saturating_sub(width);
        self.data.iter().skip(skip).take(n).map(|&v| {
            let norm = (v / max_val).clamp(0.0, 1.0);
            BARS[(norm * (BARS.len() - 1) as f64) as usize]
        }).collect()
    }

    pub fn len(&self) -> usize { self.data.len() }
    pub fn is_empty(&self) -> bool { self.data.is_empty() }
}

// ─── Profiler ─────────────────────────────────────────────────────────────────

pub struct Profiler {
    pub enabled: bool,
    pub frame_history: VecDeque<FrameProfile>,
    pub max_history: usize,
    pub current_frame: Option<FrameProfile>,
    pub frame_counter: u64,
    next_span_id: u32,
    open_spans: Vec<SpanId>,
    span_stack: Vec<SpanId>,
    pub budget: PerformanceBudget,
    pub budget_violations: Vec<BudgetReport>,
    pub stats: HashMap<String, StatRingBuffer>,
    pub capture_flamegraph: bool,
    pub flamegraph_history: VecDeque<FlamegraphNode>,
    max_flamegraph_history: usize,
    sim_time_us: u64,   // simulated monotonic clock for testing
}

impl Profiler {
    pub fn new() -> Self {
        let mut p = Self {
            enabled: true,
            frame_history: VecDeque::new(),
            max_history: 120,
            current_frame: None,
            frame_counter: 0,
            next_span_id: 1,
            open_spans: Vec::new(),
            span_stack: Vec::new(),
            budget: PerformanceBudget::default(),
            budget_violations: Vec::new(),
            stats: HashMap::new(),
            capture_flamegraph: true,
            flamegraph_history: VecDeque::new(),
            max_flamegraph_history: 30,
            sim_time_us: 0,
        };
        p.register_stat("fps",        CounterUnit::Hertz,       128);
        p.register_stat("frame_ms",   CounterUnit::Milliseconds, 128);
        p.register_stat("cpu_ms",     CounterUnit::Milliseconds, 128);
        p.register_stat("gpu_ms",     CounterUnit::Milliseconds, 128);
        p.register_stat("draw_calls", CounterUnit::DrawCalls,    128);
        p.register_stat("triangles",  CounterUnit::Triangles,    128);
        p.register_stat("vram",       CounterUnit::Bytes,        128);
        p.register_stat("heap",       CounterUnit::Bytes,        128);
        p.register_stat("particles",  CounterUnit::Count,        128);
        p
    }

    pub fn register_stat(&mut self, name: &str, unit: CounterUnit, capacity: usize) {
        self.stats.insert(name.to_string(), StatRingBuffer::new(name, unit, capacity));
    }

    fn now(&self) -> Timestamp {
        Timestamp(self.sim_time_us)
    }

    pub fn advance_sim_time(&mut self, us: u64) {
        self.sim_time_us += us;
    }

    pub fn begin_frame(&mut self) {
        if !self.enabled { return; }
        let ts = self.now();
        self.current_frame = Some(FrameProfile::new(self.frame_counter, ts));
        self.frame_counter += 1;
        self.open_spans.clear();
        self.span_stack.clear();
    }

    pub fn end_frame(&mut self, frame_ms: f64) {
        if !self.enabled { return; }
        self.advance_sim_time((frame_ms * 1000.0) as u64);
        let ts = self.now();
        if let Some(frame) = &mut self.current_frame {
            frame.frame_end = Some(ts);
        }
        if let Some(frame) = self.current_frame.take() {
            // Check budget
            let report = self.budget.check(&frame);
            if !report.is_clean() {
                self.budget_violations.push(report);
                if self.budget_violations.len() > 100 {
                    self.budget_violations.remove(0);
                }
            }
            // Build flamegraph
            if self.capture_flamegraph {
                let fg = frame.build_flamegraph();
                if self.flamegraph_history.len() >= self.max_flamegraph_history {
                    self.flamegraph_history.pop_front();
                }
                self.flamegraph_history.push_back(fg);
            }
            // Record stats
            if let Some(cpu_dur) = frame.cpu_duration() {
                let cpu_ms = cpu_dur.as_ms();
                if let Some(s) = self.stats.get_mut("frame_ms") { s.push(cpu_ms); }
                if let Some(s) = self.stats.get_mut("cpu_ms")   { s.push(cpu_ms); }
                if cpu_ms > 0.0 {
                    if let Some(s) = self.stats.get_mut("fps") { s.push(1000.0 / cpu_ms); }
                }
            }
            // Add to history
            if self.frame_history.len() >= self.max_history {
                self.frame_history.pop_front();
            }
            self.frame_history.push_back(frame);
        }
    }

    pub fn begin_span(&mut self, name: &str, kind: SpanKind) -> SpanId {
        if !self.enabled { return SpanId(0); }
        let id = SpanId(self.next_span_id);
        self.next_span_id += 1;
        let ts = self.now();
        let parent = self.span_stack.last().copied();
        let depth  = self.span_stack.len() as u32;
        let span   = ProfileSpan {
            id, name: name.to_string(), kind,
            start: ts, end: None,
            parent, depth, thread: 0, extra: None,
        };
        if let Some(frame) = &mut self.current_frame {
            frame.spans.push(span);
        }
        self.span_stack.push(id);
        self.open_spans.push(id);
        id
    }

    pub fn end_span(&mut self, id: SpanId) {
        if !self.enabled { return; }
        let ts = self.now();
        if let Some(frame) = &mut self.current_frame {
            if let Some(span) = frame.spans.iter_mut().find(|s| s.id == id) {
                span.end = Some(ts);
            }
        }
        self.span_stack.retain(|&s| s != id);
        self.open_spans.retain(|&s| s != id);
    }

    pub fn record_counter(&mut self, name: &str, value: f64, unit: CounterUnit) {
        if let Some(s) = self.stats.get_mut(name) {
            s.push(value);
        } else {
            self.register_stat(name, unit, 128);
            if let Some(s) = self.stats.get_mut(name) {
                s.push(value);
            }
        }
        if let Some(frame) = &mut self.current_frame {
            frame.counters.insert(name.to_string(), CounterSample {
                name: name.to_string(),
                value,
                unit,
                min: value,
                max: value,
            });
        }
    }

    pub fn record_memory(&mut self, mem: MemorySnapshot) {
        if let Some(s) = self.stats.get_mut("vram") {
            s.push(mem.gpu_vram_bytes as f64);
        }
        if let Some(s) = self.stats.get_mut("heap") {
            s.push(mem.heap_bytes as f64);
        }
        if let Some(frame) = &mut self.current_frame {
            frame.memory = mem;
        }
    }

    pub fn fps(&self) -> f64 {
        self.stats.get("fps").map(|s| s.latest()).unwrap_or(0.0)
    }

    pub fn frame_ms(&self) -> f64 {
        self.stats.get("frame_ms").map(|s| s.latest()).unwrap_or(0.0)
    }

    pub fn mean_fps(&self) -> f64 {
        self.stats.get("fps").map(|s| s.mean()).unwrap_or(0.0)
    }

    pub fn p95_frame_ms(&self) -> f64 {
        self.stats.get("frame_ms").map(|s| s.percentile(95.0)).unwrap_or(0.0)
    }

    pub fn latest_frame(&self) -> Option<&FrameProfile> {
        self.frame_history.back()
    }

    pub fn latest_flamegraph(&self) -> Option<&FlamegraphNode> {
        self.flamegraph_history.back()
    }

    pub fn stat(&self, name: &str) -> Option<&StatRingBuffer> {
        self.stats.get(name)
    }

    pub fn summary_line(&self) -> String {
        let fps = self.fps();
        let ms  = self.frame_ms();
        let p95 = self.p95_frame_ms();
        format!("FPS: {:.0}  avg: {:.2}ms  p95: {:.2}ms", fps, ms, p95)
    }

    pub fn detailed_summary(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("=== Profiler ({} frames) ===\n", self.frame_history.len()));
        out.push_str(&format!("FPS: {:.1}  Frame: {}  p95: {}\n",
            self.fps(),
            Duration::from_ms(self.frame_ms()),
            Duration::from_ms(self.p95_frame_ms()),
        ));
        for (name, stat) in &self.stats {
            if stat.is_empty() { continue; }
            out.push_str(&format!(
                "  {:<20} latest={:<12} mean={:<12} p95={:<12} {}\n",
                name,
                stat.unit.format(stat.latest()),
                stat.unit.format(stat.mean()),
                stat.unit.format(stat.percentile(95.0)),
                stat.ascii_sparkline(20),
            ));
        }
        if !self.budget_violations.is_empty() {
            let recent = &self.budget_violations[self.budget_violations.len().saturating_sub(3)..];
            out.push_str(&format!("⚠ {} budget violations (last 3 frames):\n",
                self.budget_violations.len()));
            for report in recent {
                for v in &report.violations {
                    out.push_str(&format!("  frame {}: {}\n", report.frame_number, v));
                }
            }
        }
        if let Some(fg) = self.latest_flamegraph() {
            out.push_str("--- Last Flamegraph ---\n");
            out.push_str(&fg.ascii_tree(0));
        }
        out
    }

    pub fn reset_stats(&mut self) {
        for stat in self.stats.values_mut() {
            stat.data.clear();
        }
        self.budget_violations.clear();
        self.frame_history.clear();
    }
}

impl Default for Profiler {
    fn default() -> Self { Self::new() }
}

// ─── Scoped span guard ────────────────────────────────────────────────────────

/// RAII guard that automatically closes a span on drop.
/// Usage: `let _guard = profiler.scoped_span("MyFunc", SpanKind::Cpu);`
pub struct SpanGuard<'a> {
    profiler: &'a mut Profiler,
    id: SpanId,
}

impl<'a> SpanGuard<'a> {
    pub fn new(profiler: &'a mut Profiler, name: &str, kind: SpanKind) -> Self {
        let id = profiler.begin_span(name, kind);
        Self { profiler, id }
    }
}

impl<'a> Drop for SpanGuard<'a> {
    fn drop(&mut self) {
        self.profiler.end_span(self.id);
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_display() {
        assert_eq!(Duration::from_us(500).to_string(), "500µs");
        assert_eq!(Duration::from_ms(2.5).to_string(), "2.50ms");
    }

    #[test]
    fn stat_ring_mean() {
        let mut buf = StatRingBuffer::new("test", CounterUnit::Count, 4);
        buf.push(1.0); buf.push(2.0); buf.push(3.0); buf.push(4.0);
        assert!((buf.mean() - 2.5).abs() < 1e-9);
    }

    #[test]
    fn stat_ring_capacity() {
        let mut buf = StatRingBuffer::new("test", CounterUnit::Count, 3);
        buf.push(1.0); buf.push(2.0); buf.push(3.0); buf.push(4.0);
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.latest(), 4.0);
    }

    #[test]
    fn stat_percentile() {
        let mut buf = StatRingBuffer::new("p", CounterUnit::Count, 100);
        for i in 0..100 { buf.push(i as f64); }
        let p95 = buf.percentile(95.0);
        assert!(p95 >= 90.0 && p95 <= 100.0);
    }

    #[test]
    fn profiler_frame_cycle() {
        let mut p = Profiler::new();
        p.begin_frame();
        let id = p.begin_span("RenderScene", SpanKind::Cpu);
        p.advance_sim_time(8000); // 8ms
        p.end_span(id);
        p.end_frame(8.0);
        assert_eq!(p.frame_history.len(), 1);
        let frame = p.frame_history.back().unwrap();
        let dur = frame.cpu_duration().unwrap();
        assert!(dur.as_ms() > 0.0);
    }

    #[test]
    fn flamegraph_tree() {
        let mut p = Profiler::new();
        p.begin_frame();
        let root = p.begin_span("Root", SpanKind::Cpu);
        p.advance_sim_time(1000);
        let child = p.begin_span("Child", SpanKind::Cpu);
        p.advance_sim_time(500);
        p.end_span(child);
        p.end_span(root);
        p.end_frame(1.5);
        let fg = p.latest_flamegraph().unwrap();
        assert!(!fg.children.is_empty());
    }

    #[test]
    fn budget_violation_overage() {
        let budget = PerformanceBudget {
            frame_time_ms: 16.0,
            ..Default::default()
        };
        let mut frame = FrameProfile::new(0, Timestamp(0));
        frame.frame_end = Some(Timestamp(20_000)); // 20ms > 16ms budget
        let report = budget.check(&frame);
        assert!(!report.violations.is_empty());
        assert!(report.violations[0].overage_pct() > 0.0);
    }

    #[test]
    fn format_bytes() {
        assert_eq!(super::format_bytes(1024), "1 KB");
        assert_eq!(super::format_bytes(1024*1024), "1.0 MB");
        assert_eq!(super::format_bytes(512), "512 B");
    }
}
