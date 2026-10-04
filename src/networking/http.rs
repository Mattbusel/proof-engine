//! HTTP client with retry, caching, and rate limiting.
//!
//! All requests are queued and driven by `tick()`. Results arrive as
//! `HttpEvent` values polled from `drain_events()`. Each request runs on a
//! short-lived background thread using [`ureq`](https://crates.io/crates/ureq)
//! (rustls for HTTPS, no OpenSSL), so `tick()` never blocks the frame.
//!
//! The network backend needs the `http` cargo feature. Without it every
//! request fails at once with [`HttpError::InvalidRequest`] saying so; before
//! 0.3.0 every request was instead answered with a made-up empty `200 OK`.
//!
//! ## Features
//! - GET, POST, PUT, DELETE, PATCH
//! - Per-request timeout
//! - Retry with exponential backoff and jitter
//! - In-memory response cache (ETag / Last-Modified)
//! - Rate limiter (token bucket per base URL)
//! - JSON body helpers
//! - Binary response support

#![warn(missing_docs)]

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

// ── Method ────────────────────────────────────────────────────────────────────

/// HTTP request method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// GET.
    Get,
    /// POST.
    Post,
    /// PUT.
    Put,
    /// DELETE.
    Delete,
    /// PATCH.
    Patch,
    /// HEAD.
    Head,
    /// OPTIONS.
    Options,
}

impl Method {
    /// The method name as sent on the wire, for example `"GET"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get     => "GET",
            Self::Post    => "POST",
            Self::Put     => "PUT",
            Self::Delete  => "DELETE",
            Self::Patch   => "PATCH",
            Self::Head    => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

// ── HttpRequest ───────────────────────────────────────────────────────────────

/// An HTTP request to be issued by the client.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// Unique id, used to match events to requests.
    pub id:           RequestId,
    /// HTTP method.
    pub method:       Method,
    /// Full URL including scheme, for example `https://example.com/scores`.
    pub url:          String,
    /// Request headers.
    pub headers:      HashMap<String, String>,
    /// Request body, if any.
    pub body:         Option<Vec<u8>>,
    /// Time allowed for each attempt (default 10 s).
    pub timeout:      Duration,
    /// Retries after a connection error, timeout or 5xx response (default 3).
    pub max_retries:  u32,
    /// Priority: higher = processed first. Default 0.
    pub priority:     i32,
    /// Tag for grouping/cancellation.
    pub tag:          Option<String>,
    /// Cache behavior.
    pub cache_policy: CachePolicy,
}

/// How a GET request uses the response cache.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CachePolicy {
    /// Never cache.
    NoStore,
    /// Use cached response if fresh.
    UseCache,
    /// Revalidate with ETag even if cached.
    Revalidate,
    /// Force fresh fetch, bypass cache.
    NoCache,
}

impl HttpRequest {
    /// A GET request.
    pub fn get(url: impl Into<String>) -> Self {
        Self::new(Method::Get, url)
    }

    /// A POST request with a raw body.
    pub fn post(url: impl Into<String>, body: Vec<u8>) -> Self {
        let mut r = Self::new(Method::Post, url);
        r.body = Some(body);
        r
    }

    /// A POST request with a JSON body and `Content-Type: application/json`.
    pub fn post_json(url: impl Into<String>, json: impl Into<String>) -> Self {
        let mut r = Self::new(Method::Post, url);
        r.body = Some(json.into().into_bytes());
        r.headers.insert("Content-Type".into(), "application/json".into());
        r
    }

    /// A request with a fresh id, 10 s timeout, 3 retries and `UseCache`.
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            id:           RequestId::next(),
            method,
            url:          url.into(),
            headers:      HashMap::new(),
            body:         None,
            timeout:      Duration::from_secs(10),
            max_retries:  3,
            priority:     0,
            tag:          None,
            cache_policy: CachePolicy::UseCache,
        }
    }

    /// Add or replace a header.
    pub fn with_header(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.headers.insert(key.into(), val.into());
        self
    }

    /// Set the per-attempt timeout.
    pub fn with_timeout(mut self, t: Duration) -> Self { self.timeout = t; self }
    /// Set the number of retries.
    pub fn with_retries(mut self, n: u32) -> Self { self.max_retries = n; self }
    /// Set the priority; higher is sent first.
    pub fn with_priority(mut self, p: i32) -> Self { self.priority = p; self }
    /// Set a tag that `HttpClient::cancel_by_tag` can match.
    pub fn with_tag(mut self, t: impl Into<String>) -> Self { self.tag = Some(t.into()); self }
    /// Set the cache policy.
    pub fn with_cache(mut self, p: CachePolicy) -> Self { self.cache_policy = p; self }

    /// Add `Authorization: Bearer <token>`.
    pub fn bearer_auth(self, token: impl Into<String>) -> Self {
        self.with_header("Authorization", format!("Bearer {}", token.into()))
    }
}

// ── RequestId ─────────────────────────────────────────────────────────────────

/// Identifies one request across its retries and events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(pub u64);

impl RequestId {
    /// A new process-wide unique id.
    pub fn next() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

// ── HttpResponse ──────────────────────────────────────────────────────────────

/// A response from the server (or from the cache).
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status:   u16,
    /// Response headers, names in lower case.
    pub headers:  HashMap<String, String>,
    /// Raw response body.
    pub body:     Vec<u8>,
    /// Parsed as UTF-8 if possible.
    pub text:     Option<String>,
    /// Time from sending the request to reading the whole body.
    pub latency:  Duration,
    /// True if this response was served from the client's cache.
    pub from_cache: bool,
}

impl HttpResponse {
    /// Status is 2xx.
    pub fn is_success(&self) -> bool { (200..300).contains(&self.status) }
    /// Status is 4xx.
    pub fn is_client_error(&self) -> bool { (400..500).contains(&self.status) }
    /// Status is 5xx.
    pub fn is_server_error(&self) -> bool { (500..600).contains(&self.status) }
    /// Status is 304 Not Modified.
    pub fn is_not_modified(&self) -> bool { self.status == 304 }

    /// The `content-type` header.
    pub fn content_type(&self) -> Option<&str> {
        self.headers.get("content-type").map(|s| s.as_str())
    }

    /// The `etag` header.
    pub fn etag(&self) -> Option<&str> {
        self.headers.get("etag").map(|s| s.as_str())
    }

    /// The `last-modified` header.
    pub fn last_modified(&self) -> Option<&str> {
        self.headers.get("last-modified").map(|s| s.as_str())
    }

    /// The body as UTF-8 text, or `""` if it is not valid UTF-8.
    pub fn text_body(&self) -> &str {
        self.text.as_deref().unwrap_or("")
    }

    /// Parse the body as JSON.
    pub fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_slice(&self.body)
    }

    /// A top-level field of a JSON object body, as text: strings are
    /// returned unquoted and unescaped, other values as JSON (`42`, `true`).
    /// `None` if the body is not a JSON object, has no such key, or the value
    /// is `null`.
    pub fn json_field(&self, key: &str) -> Option<String> {
        match self.json().ok()?.get(key)? {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Null => None,
            other => Some(other.to_string()),
        }
    }
}

// ── HttpEvent ─────────────────────────────────────────────────────────────────

/// Something that happened to a request, from `HttpClient::drain_events`.
#[derive(Debug, Clone)]
pub enum HttpEvent {
    /// The server answered (any status; check `response.is_success()`).
    Success {
        /// The request.
        id: RequestId,
        /// The server's answer.
        response: HttpResponse,
    },
    /// A request failed after all retries.
    Failure {
        /// The request.
        id: RequestId,
        /// What went wrong.
        error: HttpError,
        /// The request URL.
        url: String,
    },
    /// A request timed out on its last attempt.
    Timeout {
        /// The request.
        id: RequestId,
        /// The request URL.
        url: String,
    },
    /// A request was cancelled.
    Cancelled {
        /// The request.
        id: RequestId,
    },
    /// Rate limit hit: request was delayed.
    RateLimited {
        /// The request.
        id: RequestId,
        /// How long it waits before it can be sent, in milliseconds.
        delay_ms: u64,
    },
}

/// Why a request failed.
#[derive(Debug, Clone)]
pub enum HttpError {
    /// Could not establish connection.
    ConnectionFailed(String),
    /// DNS resolution failed.
    DnsFailure(String),
    /// TLS/SSL error.
    TlsError(String),
    /// Server returned an error status.
    ServerError(u16, String),
    /// Response body could not be read.
    ReadError(String),
    /// Request was malformed.
    InvalidRequest(String),
    /// All retries exhausted.
    RetriesExhausted {
        /// Attempts made, including the first.
        attempts: u32,
    },
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(s) => write!(f, "Connection failed: {}", s),
            Self::DnsFailure(s)       => write!(f, "DNS failure: {}", s),
            Self::TlsError(s)         => write!(f, "TLS error: {}", s),
            Self::ServerError(c, s)   => write!(f, "HTTP {}: {}", c, s),
            Self::ReadError(s)        => write!(f, "Read error: {}", s),
            Self::InvalidRequest(s)   => write!(f, "Invalid request: {}", s),
            Self::RetriesExhausted { attempts } => write!(f, "Failed after {} attempts", attempts),
        }
    }
}

// ── CacheEntry ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct CacheEntry {
    response:    HttpResponse,
    etag:        Option<String>,
    last_modified: Option<String>,
    stored_at:   Instant,
    ttl:         Duration,
}

impl CacheEntry {
    fn is_fresh(&self) -> bool {
        self.stored_at.elapsed() < self.ttl
    }
}

// ── RateLimiter ───────────────────────────────────────────────────────────────

/// Token-bucket rate limiter per base URL.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    /// Max requests per window.
    pub limit:   u32,
    /// Window size in seconds.
    pub window:  f32,
    /// Tokens currently available.
    tokens:      f32,
    last_refill: Option<Instant>,
}

impl RateLimiter {
    /// Allow `limit` requests per `window_secs` seconds, starting full.
    pub fn new(limit: u32, window_secs: f32) -> Self {
        Self { limit, window: window_secs, tokens: limit as f32, last_refill: None }
    }

    /// Take one token if available; returns false if the caller must wait.
    pub fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_refill {
            let elapsed = last.elapsed().as_secs_f32();
            let rate = self.limit as f32 / self.window.max(1e-3);
            self.tokens = (self.tokens + rate * elapsed).min(self.limit as f32);
        }
        self.last_refill = Some(now);
    }

    /// Seconds until the next token is available.
    pub fn wait_time(&self) -> f32 {
        if self.tokens >= 1.0 { return 0.0; }
        let rate = self.limit as f32 / self.window.max(1e-3);
        (1.0 - self.tokens) / rate.max(1e-6)
    }
}

// ── InFlightRequest ───────────────────────────────────────────────────────────

#[derive(Debug)]
struct InFlightRequest {
    request:        HttpRequest,
    attempt:        u32,
    started:        Instant,
    retry_after:    Option<Instant>,
}

/// What a background request thread reports back.
type Outcome = Result<HttpResponse, HttpError>;

/// Message from a request thread: request id, attempt number, outcome.
type Report = (RequestId, u32, Outcome);

// ── HttpClient ────────────────────────────────────────────────────────────────

/// Non-blocking HTTP client driven by `tick()`.
///
/// `send` queues a request; `tick` starts queued requests (up to
/// `max_concurrent`, honouring rate limits and retry backoff), collects the
/// finished ones, retries connection errors, timeouts and 5xx responses with
/// exponential backoff, and fills the event queue. Successful `GET`
/// responses are cached and, once stale, revalidated with `If-None-Match` /
/// `If-Modified-Since`.
pub struct HttpClient {
    /// Pending requests not yet dispatched.
    queue:        Vec<InFlightRequest>,
    /// Requests currently in-flight.
    in_flight:    Vec<InFlightRequest>,
    /// Response cache keyed by URL.
    cache:        HashMap<String, CacheEntry>,
    /// Rate limiters keyed by base URL (scheme + host).
    rate_limiters: HashMap<String, RateLimiter>,
    /// Completed events to be drained.
    events:       VecDeque<HttpEvent>,
    results_tx:   std::sync::mpsc::Sender<Report>,
    results_rx:   std::sync::mpsc::Receiver<Report>,
    /// Default cache TTL.
    pub cache_ttl: Duration,
    /// Maximum simultaneous connections.
    pub max_concurrent: usize,
    /// Log each dispatched request through the `log` crate at debug level.
    pub verbose:   bool,
    /// Global headers added to every request.
    pub default_headers: HashMap<String, String>,
}

impl HttpClient {
    /// A client with a 60 s cache TTL and at most 6 requests in flight.
    pub fn new() -> Self {
        let (results_tx, results_rx) = std::sync::mpsc::channel();
        Self {
            queue:           Vec::new(),
            in_flight:       Vec::new(),
            cache:           HashMap::new(),
            rate_limiters:   HashMap::new(),
            events:          VecDeque::new(),
            results_tx,
            results_rx,
            cache_ttl:       Duration::from_secs(60),
            max_concurrent:  6,
            verbose:         false,
            default_headers: HashMap::new(),
        }
    }

    /// Submit a request. Returns the RequestId for tracking.
    pub fn send(&mut self, mut request: HttpRequest) -> RequestId {
        let id = request.id;

        // Apply default headers
        for (k, v) in &self.default_headers {
            request.headers.entry(k.clone()).or_insert_with(|| v.clone());
        }

        // A fresh cache hit is served at once (UseCache); otherwise a cached
        // copy is revalidated. Before 0.3.0 a UseCache request was served
        // from the cache however stale it was.
        let cacheable = request.method == Method::Get
            && matches!(request.cache_policy, CachePolicy::UseCache | CachePolicy::Revalidate);
        if cacheable {
            if let Some(entry) = self.cache.get(&request.url) {
                if entry.is_fresh() && request.cache_policy == CachePolicy::UseCache {
                    let mut resp = entry.response.clone();
                    resp.from_cache = true;
                    self.events.push_back(HttpEvent::Success { id, response: resp });
                    return id;
                }
                if let Some(etag) = &entry.etag {
                    request.headers.insert("If-None-Match".into(), etag.clone());
                }
                if let Some(lm) = &entry.last_modified {
                    request.headers.insert("If-Modified-Since".into(), lm.clone());
                }
            }
        }

        self.queue.push(InFlightRequest {
            request,
            attempt: 0,
            started: Instant::now(),
            retry_after: None,
        });

        id
    }

    /// Cancel all requests with the given tag.
    pub fn cancel_by_tag(&mut self, tag: &str) {
        let cancelled: Vec<RequestId> = self.queue.iter()
            .chain(self.in_flight.iter())
            .filter(|r| r.request.tag.as_deref() == Some(tag))
            .map(|r| r.request.id)
            .collect();
        for id in cancelled {
            self.events.push_back(HttpEvent::Cancelled { id });
        }
        self.queue.retain(|r| r.request.tag.as_deref() != Some(tag));
        self.in_flight.retain(|r| r.request.tag.as_deref() != Some(tag));
    }

    /// Set a default rate limiter for a base URL.
    pub fn set_rate_limit(&mut self, base_url: &str, limit: u32, window_secs: f32) {
        self.rate_limiters.insert(base_url.to_owned(), RateLimiter::new(limit, window_secs));
    }

    /// Set a default header on all outgoing requests.
    pub fn set_default_header(&mut self, key: impl Into<String>, val: impl Into<String>) {
        self.default_headers.insert(key.into(), val.into());
    }

    /// Drive the client state machine. Call once per frame.
    pub fn tick(&mut self, _dt: f32) {
        let now = Instant::now();
        // Highest priority first; the sort is stable, so equal priorities
        // keep their FIFO order.
        self.queue.sort_by_key(|r| -r.request.priority);

        // Start queued requests whose backoff has passed. (Before 0.3.0
        // `retry_after` was set but never checked, so retries went out at
        // once.)
        let mut i = 0;
        while self.in_flight.len() < self.max_concurrent && i < self.queue.len() {
            if self.queue[i].retry_after.is_some_and(|t| t > now) {
                i += 1;
                continue;
            }
            let mut req = self.queue.remove(i);

            let base = base_url(&req.request.url);
            if let Some(limiter) = self.rate_limiters.get_mut(&base) {
                if !limiter.try_consume() {
                    let wait_ms = (limiter.wait_time() * 1000.0).ceil() as u64;
                    self.events.push_back(HttpEvent::RateLimited {
                        id: req.request.id,
                        delay_ms: wait_ms,
                    });
                    req.retry_after = Some(now + Duration::from_millis(wait_ms));
                    self.queue.push(req);
                    continue;
                }
            }

            req.started = now;
            if self.verbose {
                log::debug!(
                    "http {} {} (attempt {})",
                    req.request.method.as_str(),
                    req.request.url,
                    req.attempt + 1
                );
            }
            dispatch(&req.request, req.attempt, self.results_tx.clone());
            self.in_flight.push(req);
        }

        // Collect finished requests.
        while let Ok((id, attempt, outcome)) = self.results_rx.try_recv() {
            let Some(pos) = self
                .in_flight
                .iter()
                .position(|r| r.request.id == id && r.attempt == attempt)
            else {
                continue; // cancelled, or a late answer to an attempt that timed out
            };
            let req = self.in_flight.remove(pos);
            match outcome {
                Ok(response)
                    if response.is_server_error() && req.attempt < req.request.max_retries =>
                {
                    self.retry(req);
                }
                Ok(response) => self.finish(req, response),
                Err(HttpError::InvalidRequest(msg)) => {
                    self.events.push_back(HttpEvent::Failure {
                        id,
                        url: req.request.url.clone(),
                        error: HttpError::InvalidRequest(msg),
                    });
                }
                Err(err) if req.attempt < req.request.max_retries => {
                    if self.verbose {
                        log::debug!("http {}: {err}, retrying", req.request.url);
                    }
                    self.retry(req);
                }
                Err(err) => {
                    let error = if req.request.max_retries == 0 {
                        err
                    } else {
                        HttpError::RetriesExhausted { attempts: req.attempt + 1 }
                    };
                    self.events.push_back(HttpEvent::Failure {
                        id,
                        url: req.request.url.clone(),
                        error,
                    });
                }
            }
        }

        // Time out requests that have not answered.
        let mut k = 0;
        while k < self.in_flight.len() {
            if self.in_flight[k].started.elapsed() > self.in_flight[k].request.timeout {
                let req = self.in_flight.remove(k);
                if req.attempt < req.request.max_retries {
                    self.retry(req);
                } else {
                    self.events.push_back(HttpEvent::Timeout {
                        id: req.request.id,
                        url: req.request.url.clone(),
                    });
                }
            } else {
                k += 1;
            }
        }
    }

    fn retry(&mut self, req: InFlightRequest) {
        let backoff = backoff_duration(req.attempt);
        self.queue.push(InFlightRequest {
            attempt: req.attempt + 1,
            started: Instant::now(),
            retry_after: Some(Instant::now() + backoff),
            ..req
        });
    }

    fn finish(&mut self, req: InFlightRequest, mut response: HttpResponse) {
        let id = req.request.id;
        let get = req.request.method == Method::Get;
        if response.is_not_modified() && get {
            // Revalidated: serve the cached copy and restart its TTL.
            if let Some(entry) = self.cache.get_mut(&req.request.url) {
                entry.stored_at = Instant::now();
                let latency = response.latency;
                response = entry.response.clone();
                response.latency = latency;
                response.from_cache = true;
            }
        } else if response.is_success() && get && req.request.cache_policy != CachePolicy::NoStore {
            self.cache.insert(req.request.url.clone(), CacheEntry {
                etag:          response.etag().map(|s| s.to_owned()),
                last_modified: response.last_modified().map(|s| s.to_owned()),
                stored_at:     Instant::now(),
                ttl:           self.cache_ttl,
                response:      response.clone(),
            });
        }
        self.events.push_back(HttpEvent::Success { id, response });
    }

    /// Drain all completed events.
    pub fn drain_events(&mut self) -> impl Iterator<Item = HttpEvent> + '_ {
        self.events.drain(..)
    }

    /// Number of pending + in-flight requests.
    pub fn pending_count(&self) -> usize {
        self.queue.len() + self.in_flight.len()
    }

    /// Clear the response cache.
    pub fn clear_cache(&mut self) { self.cache.clear(); }

    /// Remove cache entries older than their TTL.
    pub fn evict_stale_cache(&mut self) {
        self.cache.retain(|_, entry| entry.is_fresh());
    }
}

impl Default for HttpClient {
    fn default() -> Self { Self::new() }
}

// ── Network backend ───────────────────────────────────────────────────────────

/// Run `request` on a background thread and report back on `tx`.
fn dispatch(request: &HttpRequest, attempt: u32, tx: std::sync::mpsc::Sender<Report>) {
    let request = request.clone();
    let spawned = std::thread::Builder::new()
        .name("proof-http".into())
        .spawn({
            let tx = tx.clone();
            let request = request.clone();
            move || {
                let outcome = perform(&request);
                let _ = tx.send((request.id, attempt, outcome));
            }
        });
    if let Err(e) = spawned {
        let _ = tx.send((
            request.id,
            attempt,
            Err(HttpError::ConnectionFailed(format!("could not start a request thread: {e}"))),
        ));
    }
}

#[cfg(feature = "http")]
fn perform(request: &HttpRequest) -> Outcome {
    use ureq::http;
    let started = Instant::now();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(request.timeout))
        .http_status_as_error(false)
        .build()
        .into();
    let mut builder = http::Request::builder()
        .method(request.method.as_str())
        .uri(&request.url);
    for (k, v) in &request.headers {
        builder = builder.header(k.as_str(), v.as_str());
    }
    let result = match &request.body {
        Some(body) => builder
            .body(body.clone())
            .map_err(|e| HttpError::InvalidRequest(e.to_string()))
            .and_then(|r| agent.run(r).map_err(map_ureq_error)),
        None => builder
            .body(())
            .map_err(|e| HttpError::InvalidRequest(e.to_string()))
            .and_then(|r| agent.run(r).map_err(map_ureq_error)),
    };
    let mut resp = result?;
    let status = resp.status().as_u16();
    let headers: HashMap<String, String> = resp
        .headers()
        .iter()
        .filter_map(|(k, v)| {
            v.to_str()
                .ok()
                .map(|v| (k.as_str().to_ascii_lowercase(), v.to_owned()))
        })
        .collect();
    let body = resp
        .body_mut()
        .with_config()
        .limit(64 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| HttpError::ReadError(e.to_string()))?;
    let text = String::from_utf8(body.clone()).ok();
    Ok(HttpResponse {
        status,
        headers,
        body,
        text,
        latency: started.elapsed(),
        from_cache: false,
    })
}

#[cfg(feature = "http")]
fn map_ureq_error(e: ureq::Error) -> HttpError {
    match e {
        ureq::Error::HostNotFound => HttpError::DnsFailure(e.to_string()),
        ureq::Error::BadUri(_) | ureq::Error::Http(_) => HttpError::InvalidRequest(e.to_string()),
        ureq::Error::Rustls(_) => HttpError::TlsError(e.to_string()),
        other => HttpError::ConnectionFailed(other.to_string()),
    }
}

#[cfg(not(feature = "http"))]
fn perform(_request: &HttpRequest) -> Outcome {
    Err(HttpError::InvalidRequest(
        "proof-engine was built without the `http` feature, so it cannot make network requests"
            .into(),
    ))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn base_url(url: &str) -> String {
    // Extract scheme + host (e.g. "https://example.com")
    if let Some(after_scheme) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) {
        let host_end = after_scheme.find('/').unwrap_or(after_scheme.len());
        let scheme = if url.starts_with("https") { "https" } else { "http" };
        format!("{}://{}", scheme, &after_scheme[..host_end])
    } else {
        url.to_owned()
    }
}

fn backoff_duration(attempt: u32) -> Duration {
    // Exponential backoff: 200ms, 400ms, 800ms, 1600ms, cap at 30s
    let base_ms = 200u64 * (1u64 << attempt.min(7));
    // Add ±25% jitter
    let jitter = simple_hash(attempt as u64) % (base_ms / 4).max(1);
    Duration::from_millis((base_ms + jitter).min(30_000))
}

fn simple_hash(n: u64) -> u64 {
    let mut x = n ^ (n >> 33);
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^= x >> 33;
    x
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_events(client: &mut HttpClient, want: usize) -> Vec<HttpEvent> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut out = Vec::new();
        while out.len() < want && Instant::now() < deadline {
            client.tick(0.016);
            out.extend(client.drain_events());
            std::thread::sleep(Duration::from_millis(5));
        }
        out
    }

    #[test]
    fn json_field_handles_escapes_and_nesting() {
        let resp = HttpResponse {
            status: 200,
            headers: HashMap::new(),
            body: br#"{"name":"A \"quoted\" name, with comma","rank":3,"meta":{"rank":99},"gone":null}"#.to_vec(),
            text: None,
            latency: Duration::ZERO,
            from_cache: false,
        };
        // The old substring parser cut the name at the escaped quote.
        assert_eq!(resp.json_field("name").as_deref(), Some("A \"quoted\" name, with comma"));
        assert_eq!(resp.json_field("rank").as_deref(), Some("3"));
        assert_eq!(resp.json_field("gone"), None);
        assert_eq!(resp.json_field("missing"), None);
    }

    #[test]
    fn retry_after_is_honoured() {
        let mut client = HttpClient::new();
        let mut req = HttpRequest::get("http://127.0.0.1:9/never");
        req.max_retries = 0;
        client.queue.push(InFlightRequest {
            request: req,
            attempt: 1,
            started: Instant::now(),
            retry_after: Some(Instant::now() + Duration::from_secs(3600)),
        });
        client.tick(0.016);
        assert_eq!(client.in_flight.len(), 0, "a request in backoff must stay queued");
        assert_eq!(client.queue.len(), 1);
    }

    #[cfg(not(feature = "http"))]
    #[test]
    fn without_feature_requests_fail_instead_of_faking_success() {
        let mut client = HttpClient::new();
        client.send(HttpRequest::get("http://127.0.0.1:9/"));
        let events = wait_events(&mut client, 1);
        assert!(matches!(
            events.first(),
            Some(HttpEvent::Failure { error: HttpError::InvalidRequest(_), .. })
        ), "{events:?}");
    }

    /// A tiny HTTP/1.1 server answering each connection with the next
    /// canned response, recording the request heads it saw.
    #[cfg(feature = "http")]
    fn serve(responses: Vec<&'static str>) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        std::thread::spawn(move || {
            for resp in responses {
                let Ok((stream, _)) = listener.accept() else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut head = String::new();
                let mut len = 0usize;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 { break; }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                    if line == "\r\n" { break; }
                    head.push_str(&line);
                }
                let mut body = vec![0u8; len];
                let _ = reader.read_exact(&mut body);
                seen2.lock().unwrap().push(head);
                let mut stream = stream;
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        (url, seen)
    }

    #[cfg(feature = "http")]
    #[test]
    fn real_get_returns_server_body() {
        let (url, seen) = serve(vec![
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 13\r\nConnection: close\r\n\r\n{\"rank\": 42}\n",
        ]);
        let mut client = HttpClient::new();
        let id = client.send(HttpRequest::get(format!("{url}/scores")).with_header("X-Test", "1"));
        let events = wait_events(&mut client, 1);
        match &events[..] {
            [HttpEvent::Success { id: got, response }] => {
                assert_eq!(*got, id);
                assert_eq!(response.status, 200);
                assert_eq!(response.json_field("rank").as_deref(), Some("42"));
                assert!(!response.from_cache);
            }
            other => panic!("{other:?}"),
        }
        let head = &seen.lock().unwrap()[0];
        assert!(head.starts_with("GET /scores HTTP/1.1"), "{head}");
        assert!(head.to_ascii_lowercase().contains("x-test: 1"), "{head}");
    }

    #[cfg(feature = "http")]
    #[test]
    fn server_error_is_retried_then_succeeds() {
        let (url, seen) = serve(vec![
            "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
        ]);
        let mut client = HttpClient::new();
        client.send(HttpRequest::post(format!("{url}/submit"), b"{}".to_vec()).with_retries(2));
        let events = wait_events(&mut client, 1);
        match &events[..] {
            [HttpEvent::Success { response, .. }] => assert_eq!(response.text_body(), "ok"),
            other => panic!("{other:?}"),
        }
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    #[cfg(feature = "http")]
    #[test]
    fn stale_cache_is_revalidated_with_etag() {
        let (url, seen) = serve(vec![
            "HTTP/1.1 200 OK\r\nETag: \"v1\"\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
            "HTTP/1.1 304 Not Modified\r\nETag: \"v1\"\r\nConnection: close\r\n\r\n",
        ]);
        let mut client = HttpClient::new();
        client.cache_ttl = Duration::ZERO; // every entry is stale at once
        client.send(HttpRequest::get(format!("{url}/a")));
        assert_eq!(wait_events(&mut client, 1).len(), 1);
        client.send(HttpRequest::get(format!("{url}/a")));
        let events = wait_events(&mut client, 1);
        match &events[..] {
            [HttpEvent::Success { response, .. }] => {
                assert_eq!(response.text_body(), "hello");
                assert!(response.from_cache);
            }
            other => panic!("{other:?}"),
        }
        let heads = seen.lock().unwrap();
        assert!(heads[1].to_ascii_lowercase().contains("if-none-match: \"v1\""), "{}", heads[1]);
    }

    #[cfg(feature = "http")]
    #[test]
    fn connection_refused_fails_after_retries() {
        // Bind then drop a listener so the port is very likely closed.
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let mut client = HttpClient::new();
        client.send(HttpRequest::get(format!("http://127.0.0.1:{port}/")).with_retries(1));
        let events = wait_events(&mut client, 1);
        assert!(matches!(
            events.first(),
            Some(HttpEvent::Failure { error: HttpError::RetriesExhausted { attempts: 2 }, .. })
        ), "{events:?}");
    }
}
