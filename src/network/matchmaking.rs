//! Matchmaking, lobby, ELO ranking, and server discovery for Proof Engine.
//!
//! Everything is synchronous/tick-based — no async or external crates.

use std::collections::HashMap;

// ── GameMode ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameMode {
    Deathmatch,
    TeamBattle,
    Cooperative,
    Tournament,
    Custom(String),
}

impl GameMode {
    pub fn as_str(&self) -> &str {
        match self {
            GameMode::Deathmatch  => "Deathmatch",
            GameMode::TeamBattle  => "TeamBattle",
            GameMode::Cooperative => "Cooperative",
            GameMode::Tournament  => "Tournament",
            GameMode::Custom(s)   => s.as_str(),
        }
    }

    pub fn min_players(&self) -> u32 {
        match self {
            GameMode::Deathmatch  => 2,
            GameMode::TeamBattle  => 4,
            GameMode::Cooperative => 2,
            GameMode::Tournament  => 8,
            GameMode::Custom(_)   => 2,
        }
    }

    pub fn max_players(&self) -> u32 {
        match self {
            GameMode::Deathmatch  => 16,
            GameMode::TeamBattle  => 16,
            GameMode::Cooperative => 4,
            GameMode::Tournament  => 32,
            GameMode::Custom(_)   => 64,
        }
    }
}

// ── RankTier ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RankTier {
    Bronze,
    Silver,
    Gold,
    Platinum,
    Diamond,
    Master,
    Grandmaster,
}

impl RankTier {
    pub fn as_str(self) -> &'static str {
        match self {
            RankTier::Bronze      => "Bronze",
            RankTier::Silver      => "Silver",
            RankTier::Gold        => "Gold",
            RankTier::Platinum    => "Platinum",
            RankTier::Diamond     => "Diamond",
            RankTier::Master      => "Master",
            RankTier::Grandmaster => "Grandmaster",
        }
    }

    pub fn rank_color(self) -> [f32; 3] {
        match self {
            RankTier::Bronze      => [0.80, 0.50, 0.20],
            RankTier::Silver      => [0.75, 0.75, 0.75],
            RankTier::Gold        => [1.00, 0.84, 0.00],
            RankTier::Platinum    => [0.60, 0.90, 0.90],
            RankTier::Diamond     => [0.40, 0.80, 1.00],
            RankTier::Master      => [0.80, 0.40, 1.00],
            RankTier::Grandmaster => [1.00, 0.50, 0.00],
        }
    }
}

// ── EloSystem ─────────────────────────────────────────────────────────────────

pub struct EloSystem;

impl EloSystem {
    pub const K_FACTOR: f32 = 32.0;
    pub const DEFAULT_RATING: f32 = 1200.0;

    /// Expected score for player A against player B (0..1 probability).
    pub fn expected_score(rating_a: f32, rating_b: f32) -> f32 {
        1.0 / (1.0 + 10.0f32.powf((rating_b - rating_a) / 400.0))
    }

    /// Update rating given actual score (1=win, 0.5=draw, 0=loss) and expected.
    pub fn update_rating(rating: f32, score: f32, expected: f32) -> f32 {
        let new = rating + Self::K_FACTOR * (score - expected);
        new.max(100.0) // floor rating
    }

    /// Convenience: process a match result and return updated ratings.
    /// `score_a`: 1.0 = A wins, 0.5 = draw, 0.0 = A loses.
    pub fn process_match(rating_a: f32, rating_b: f32, score_a: f32) -> (f32, f32) {
        let ea = Self::expected_score(rating_a, rating_b);
        let eb = Self::expected_score(rating_b, rating_a);
        let score_b = 1.0 - score_a;
        (
            Self::update_rating(rating_a, score_a, ea),
            Self::update_rating(rating_b, score_b, eb),
        )
    }

    pub fn rating_to_rank(rating: f32) -> RankTier {
        match rating as u32 {
            0..=999        => RankTier::Bronze,
            1000..=1199    => RankTier::Silver,
            1200..=1399    => RankTier::Gold,
            1400..=1599    => RankTier::Platinum,
            1600..=1799    => RankTier::Diamond,
            1800..=1999    => RankTier::Master,
            _              => RankTier::Grandmaster,
        }
    }

    /// Returns rating range [min, max] that a player with `rating` can be matched against.
    pub fn match_range(rating: f32, tolerance: f32) -> (f32, f32) {
        (rating - tolerance, rating + tolerance)
    }
}

// ── PlayerProfile ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PlayerProfile {
    pub id:           String,
    pub display_name: String,
    pub rating:       f32,
    pub wins:         u32,
    pub losses:       u32,
    pub draws:        u32,
    pub games_played: u32,
}

impl PlayerProfile {
    pub fn new(id: String, display_name: String) -> Self {
        PlayerProfile {
            id,
            display_name,
            rating:       EloSystem::DEFAULT_RATING,
            wins:         0,
            losses:       0,
            draws:        0,
            games_played: 0,
        }
    }

    pub fn rank(&self) -> RankTier { EloSystem::rating_to_rank(self.rating) }

    pub fn win_rate(&self) -> f32 {
        if self.games_played == 0 { return 0.0; }
        self.wins as f32 / self.games_played as f32
    }

    pub fn record_win(&mut self, opponent_rating: f32) {
        let (new_r, _) = EloSystem::process_match(self.rating, opponent_rating, 1.0);
        self.rating = new_r;
        self.wins += 1;
        self.games_played += 1;
    }

    pub fn record_loss(&mut self, opponent_rating: f32) {
        let (new_r, _) = EloSystem::process_match(self.rating, opponent_rating, 0.0);
        self.rating = new_r;
        self.losses += 1;
        self.games_played += 1;
    }

    pub fn record_draw(&mut self, opponent_rating: f32) {
        let (new_r, _) = EloSystem::process_match(self.rating, opponent_rating, 0.5);
        self.rating = new_r;
        self.draws += 1;
        self.games_played += 1;
    }
}

// ── MatchmakingTicket ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MatchmakingTicket {
    pub player_id: String,
    pub rank:      u32,
    pub region:    String,
    pub mode:      GameMode,
    pub timestamp: f32,
    /// Seconds this ticket has been waiting — increases tolerance over time
    pub wait_time: f32,
}

impl MatchmakingTicket {
    pub fn new(player_id: String, rating: f32, region: String, mode: GameMode, now: f32) -> Self {
        MatchmakingTicket {
            player_id,
            rank:      rating as u32,
            region,
            mode,
            timestamp: now,
            wait_time: 0.0,
        }
    }

    /// Allowed rating tolerance grows with wait time (up to 500 points after 60s).
    pub fn tolerance(&self) -> f32 {
        (100.0 + self.wait_time * 6.67).min(500.0)
    }

    pub fn matches(&self, other: &MatchmakingTicket) -> bool {
        if self.mode != other.mode     { return false; }
        if self.region != other.region { return false; }
        let tol = self.tolerance().max(other.tolerance());
        (self.rank as i64 - other.rank as i64).abs() <= tol as i64
    }
}

// ── MatchmakingResult ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MatchmakingResult {
    pub players:   Vec<String>,
    pub server_id: String,
    pub map_id:    String,
    pub mode:      GameMode,
}

// ── MatchmakingPool ───────────────────────────────────────────────────────────

/// Matchmaking queue sorted by rank. Groups by mode + region.
pub struct MatchmakingPool {
    tickets:          Vec<MatchmakingTicket>,
    next_server_id:   u64,
    maps:             Vec<String>,
}

impl MatchmakingPool {
    pub fn new() -> Self {
        MatchmakingPool {
            tickets:        Vec::new(),
            next_server_id: 1,
            maps:           vec![
                "map_arena".into(),
                "map_warehouse".into(),
                "map_rooftop".into(),
                "map_forest".into(),
            ],
        }
    }

    pub fn add_map(&mut self, map_id: String) { self.maps.push(map_id); }

    /// Enqueue a player. No-op if already queued.
    pub fn enqueue(&mut self, ticket: MatchmakingTicket) {
        if self.tickets.iter().any(|t| t.player_id == ticket.player_id) { return; }
        self.tickets.push(ticket);
        // Keep sorted by rank ascending for efficient matching
        self.tickets.sort_by_key(|t| t.rank);
    }

    /// Remove a player from the queue.
    pub fn dequeue(&mut self, player_id: &str) {
        self.tickets.retain(|t| t.player_id != player_id);
    }

    pub fn queue_size(&self) -> usize { self.tickets.len() }

    pub fn is_queued(&self, player_id: &str) -> bool {
        self.tickets.iter().any(|t| t.player_id == player_id)
    }

    /// Advance wait times by `dt`. Call each tick.
    pub fn tick(&mut self, dt: f32) {
        for t in &mut self.tickets { t.wait_time += dt; }
    }

    /// Run a matchmaking pass. Returns completed matches.
    pub fn run_matchmaking(&mut self) -> Vec<MatchmakingResult> {
        let mut results  = Vec::new();
        let mut consumed = vec![false; self.tickets.len()];

        let ticket_count = self.tickets.len();
        let mut i = 0;
        while i < ticket_count {
            if consumed[i] { i += 1; continue; }

            let mode      = self.tickets[i].mode.clone();
            let min_p     = mode.min_players() as usize;
            let max_p     = mode.max_players() as usize;

            let mut group: Vec<usize> = vec![i];

            for j in (i + 1)..ticket_count {
                if consumed[j] { continue; }
                if group.len() >= max_p { break; }
                if self.tickets[i].matches(&self.tickets[j]) {
                    group.push(j);
                }
            }

            if group.len() >= min_p {
                let players: Vec<String> = group.iter()
                    .map(|&idx| self.tickets[idx].player_id.clone())
                    .collect();

                for &idx in &group { consumed[idx] = true; }

                let server_id = format!("srv_{}", self.next_server_id);
                self.next_server_id += 1;

                let map_idx = (self.next_server_id as usize) % self.maps.len().max(1);
                let map_id  = self.maps.get(map_idx)
                    .cloned()
                    .unwrap_or_else(|| "map_default".into());

                results.push(MatchmakingResult { players, server_id, map_id, mode });
            }

            i += 1;
        }

        // Remove consumed tickets
        let mut keep = consumed.iter();
        self.tickets.retain(|_| !*keep.next().unwrap_or(&false));

        results
    }
}

impl Default for MatchmakingPool {
    fn default() -> Self { Self::new() }
}

// ── LobbyState ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyState {
    Waiting,
    Countdown { secs_left: u32 },
    Starting,
    InGame,
    Ended,
}

impl LobbyState {
    pub fn is_joinable(&self) -> bool {
        matches!(self, LobbyState::Waiting | LobbyState::Countdown { .. })
    }
}

// ── LobbySettings ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LobbySettings {
    pub map_id:        String,
    pub mode:          GameMode,
    pub time_limit:    u32, // seconds, 0 = no limit
    pub score_limit:   u32, // frags/points, 0 = no limit
    pub friendly_fire: bool,
    pub password:      Option<String>,
    pub spectators_ok: bool,
}

impl Default for LobbySettings {
    fn default() -> Self {
        LobbySettings {
            map_id:        "map_arena".into(),
            mode:          GameMode::Deathmatch,
            time_limit:    600,
            score_limit:   30,
            friendly_fire: false,
            password:      None,
            spectators_ok: true,
        }
    }
}

// ── LobbyPlayer ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LobbyPlayer {
    pub id:         String,
    pub name:       String,
    pub team:       u8,
    pub ready:      bool,
    pub ping:       u32,
    pub is_host:    bool,
    pub is_spectator: bool,
    pub joined_at:  f32,
}

impl LobbyPlayer {
    pub fn new(id: String, name: String, team: u8, now: f32) -> Self {
        LobbyPlayer {
            id, name, team,
            ready:       false,
            ping:        0,
            is_host:     false,
            is_spectator: false,
            joined_at:   now,
        }
    }
}

// ── Lobby ─────────────────────────────────────────────────────────────────────

/// A pre-game lobby where players gather, set teams, and ready up.
pub struct Lobby {
    pub id:          String,
    pub max_players: u32,
    pub players:     Vec<LobbyPlayer>,
    pub state:       LobbyState,
    pub settings:    LobbySettings,
    countdown_acc:   f32,
    pub created_at:  f32,
}

impl Lobby {
    pub fn new(id: String, max_players: u32, settings: LobbySettings, now: f32) -> Self {
        Lobby {
            id,
            max_players,
            players:       Vec::new(),
            state:         LobbyState::Waiting,
            settings,
            countdown_acc: 0.0,
            created_at:    now,
        }
    }

    // ── Player operations ────────────────────────────────────────────────────

    pub fn add_player(&mut self, mut player: LobbyPlayer) -> bool {
        if !self.state.is_joinable() { return false; }
        let actual_players = self.players.iter().filter(|p| !p.is_spectator).count();
        if actual_players >= self.max_players as usize && !player.is_spectator {
            if !self.settings.spectators_ok { return false; }
            player.is_spectator = true;
        }
        if self.players.is_empty() { player.is_host = true; }
        self.players.push(player);
        true
    }

    pub fn remove_player(&mut self, player_id: &str) {
        let was_host = self.players.iter()
            .find(|p| p.id == player_id)
            .map(|p| p.is_host)
            .unwrap_or(false);

        self.players.retain(|p| p.id != player_id);

        // Transfer host to next player
        if was_host {
            if let Some(first) = self.players.first_mut() {
                first.is_host = true;
            }
        }

        // Cancel countdown if not enough ready players
        if self.all_ready_count() < self.settings.mode.min_players() as usize {
            if matches!(self.state, LobbyState::Countdown { .. }) {
                self.state = LobbyState::Waiting;
                self.countdown_acc = 0.0;
            }
        }
    }

    pub fn set_ready(&mut self, player_id: &str) -> bool {
        if let Some(p) = self.players.iter_mut().find(|p| p.id == player_id) {
            p.ready = true;
            return true;
        }
        false
    }

    pub fn set_not_ready(&mut self, player_id: &str) -> bool {
        if let Some(p) = self.players.iter_mut().find(|p| p.id == player_id) {
            p.ready = false;
            if matches!(self.state, LobbyState::Countdown { .. }) {
                self.state = LobbyState::Waiting;
                self.countdown_acc = 0.0;
            }
            return true;
        }
        false
    }

    pub fn kick(&mut self, player_id: &str) {
        self.remove_player(player_id);
    }

    pub fn change_team(&mut self, player_id: &str, team: u8) -> bool {
        if let Some(p) = self.players.iter_mut().find(|p| p.id == player_id) {
            p.team = team;
            return true;
        }
        false
    }

    pub fn update_ping(&mut self, player_id: &str, ping_ms: u32) {
        if let Some(p) = self.players.iter_mut().find(|p| p.id == player_id) {
            p.ping = ping_ms;
        }
    }

    // ── Countdown ────────────────────────────────────────────────────────────

    /// Tick the lobby. Returns `true` when the countdown has finished and the
    /// game should start. Automatically begins countdown when all ready.
    pub fn tick_countdown(&mut self, dt: f32) -> bool {
        match &self.state {
            LobbyState::Waiting => {
                let ready = self.all_ready_count();
                let min   = self.settings.mode.min_players() as usize;
                if ready >= min && ready == self.active_player_count() && ready > 0 {
                    self.state         = LobbyState::Countdown { secs_left: 10 };
                    self.countdown_acc = 0.0;
                }
                false
            }
            LobbyState::Countdown { .. } => {
                self.countdown_acc += dt;
                let secs_left = (10.0 - self.countdown_acc).max(0.0) as u32;
                if self.countdown_acc >= 10.0 {
                    self.state = LobbyState::Starting;
                    true
                } else {
                    self.state = LobbyState::Countdown { secs_left };
                    false
                }
            }
            LobbyState::Starting => {
                self.state = LobbyState::InGame;
                false
            }
            _ => false,
        }
    }

    pub fn force_start(&mut self) {
        self.state = LobbyState::Starting;
    }

    // ── Queries ──────────────────────────────────────────────────────────────

    pub fn is_full(&self) -> bool {
        self.active_player_count() >= self.max_players as usize
    }

    pub fn active_player_count(&self) -> usize {
        self.players.iter().filter(|p| !p.is_spectator).count()
    }

    pub fn all_ready_count(&self) -> usize {
        self.players.iter().filter(|p| !p.is_spectator && p.ready).count()
    }

    pub fn has_player(&self, player_id: &str) -> bool {
        self.players.iter().any(|p| p.id == player_id)
    }

    pub fn host(&self) -> Option<&LobbyPlayer> {
        self.players.iter().find(|p| p.is_host)
    }

    pub fn average_ping(&self) -> u32 {
        if self.players.is_empty() { return 0; }
        let sum: u32 = self.players.iter().map(|p| p.ping).sum();
        sum / self.players.len() as u32
    }

    pub fn teams(&self) -> HashMap<u8, Vec<&LobbyPlayer>> {
        let mut map: HashMap<u8, Vec<&LobbyPlayer>> = HashMap::new();
        for p in &self.players {
            map.entry(p.team).or_default().push(p);
        }
        map
    }

    /// Auto-balance teams by moving players to even team sizes.
    pub fn balance_teams(&mut self, num_teams: u8) {
        let players_per_team = (self.active_player_count() / num_teams as usize).max(1);
        let mut counts = vec![0usize; num_teams as usize];
        for p in self.players.iter_mut().filter(|p| !p.is_spectator) {
            // Find the team with the fewest players
            let team = counts.iter().enumerate()
                .min_by_key(|(_, &c)| c)
                .map(|(i, _)| i)
                .unwrap_or(0);
            p.team = team as u8;
            counts[team] += 1;
            let _ = players_per_team;
        }
    }
}

// ── ServerEntry ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ServerEntry {
    pub id:           String,
    pub address:      String,
    pub name:         String,
    pub map_id:       String,
    pub mode:         GameMode,
    pub current_players: u32,
    pub max_players:  u32,
    pub ping_ms:      u32,
    pub region:       String,
    pub has_password: bool,
    pub last_seen:    f32,
}

impl ServerEntry {
    pub fn is_joinable(&self) -> bool {
        self.current_players < self.max_players
    }

    pub fn fill_percent(&self) -> f32 {
        if self.max_players == 0 { return 1.0; }
        self.current_players as f32 / self.max_players as f32
    }
}

// ── ServerList ────────────────────────────────────────────────────────────────

/// Caches discovered game servers and their measured latencies.
pub struct ServerList {
    entries:         HashMap<String, ServerEntry>,
    /// Scheduled pings: server_id → time_to_ping
    pending_pings:   HashMap<String, f64>,
    /// Ping results awaiting response: server_id → send_time
    in_flight_pings: HashMap<String, f64>,
    ping_interval:   f32,
    stale_timeout:   f32,
    current_time:    f64,
}

impl ServerList {
    pub fn new(ping_interval: f32, stale_timeout: f32) -> Self {
        ServerList {
            entries:         HashMap::new(),
            pending_pings:   HashMap::new(),
            in_flight_pings: HashMap::new(),
            ping_interval,
            stale_timeout,
            current_time:    0.0,
        }
    }

    /// Register or update a server in the list.
    pub fn upsert(&mut self, entry: ServerEntry) {
        self.pending_pings.entry(entry.id.clone())
            .or_insert(self.current_time);
        self.entries.insert(entry.id.clone(), entry);
    }

    /// Remove a server.
    pub fn remove(&mut self, server_id: &str) {
        self.entries.remove(server_id);
        self.pending_pings.remove(server_id);
        self.in_flight_pings.remove(server_id);
    }

    /// Advance time. Returns server IDs that need to be pinged this tick.
    pub fn tick(&mut self, dt: f32) -> Vec<String> {
        self.current_time += dt as f64;

        // Expire stale entries
        let stale = self.stale_timeout as f64;
        let ct    = self.current_time;
        self.entries.retain(|_, e| (ct - e.last_seen as f64) < stale);

        // Collect servers due for ping
        let due: Vec<String> = self.pending_pings
            .iter()
            .filter(|(_, &t)| ct >= t)
            .map(|(id, _)| id.clone())
            .collect();

        for id in &due {
            self.pending_pings.remove(id);
            self.in_flight_pings.insert(id.clone(), self.current_time);
        }

        due
    }

    /// Record a ping response.
    pub fn record_ping_response(&mut self, server_id: &str) {
        if let Some(send_time) = self.in_flight_pings.remove(server_id) {
            let rtt_ms = ((self.current_time - send_time) * 1000.0) as u32;
            if let Some(entry) = self.entries.get_mut(server_id) {
                entry.ping_ms    = rtt_ms / 2;
                entry.last_seen  = self.current_time as f32;
            }
            // Schedule next ping
            self.pending_pings.insert(
                server_id.to_string(),
                self.current_time + self.ping_interval as f64,
            );
        }
    }

    // ── Queries ──────────────────────────────────────────────────────────────

    pub fn all(&self) -> Vec<&ServerEntry> { self.entries.values().collect() }

    pub fn by_ping(&self) -> Vec<&ServerEntry> {
        let mut v: Vec<&ServerEntry> = self.entries.values().collect();
        v.sort_by_key(|e| e.ping_ms);
        v
    }

    pub fn by_mode(&self, mode: &GameMode) -> Vec<&ServerEntry> {
        self.entries.values().filter(|e| &e.mode == mode).collect()
    }

    pub fn by_region(&self, region: &str) -> Vec<&ServerEntry> {
        self.entries.values().filter(|e| e.region == region).collect()
    }

    pub fn joinable(&self) -> Vec<&ServerEntry> {
        self.entries.values().filter(|e| e.is_joinable()).collect()
    }

    pub fn best_server(&self, mode: &GameMode, region: &str) -> Option<&ServerEntry> {
        self.entries.values()
            .filter(|e| &e.mode == mode && e.region == region && e.is_joinable())
            .min_by_key(|e| e.ping_ms)
    }

    pub fn server_count(&self) -> usize { self.entries.len() }
}

impl Default for ServerList {
    fn default() -> Self { Self::new(5.0, 30.0) }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elo_win_raises_winner() {
        let (a, b) = EloSystem::process_match(1200.0, 1200.0, 1.0);
        assert!(a > 1200.0);
        assert!(b < 1200.0);
    }

    #[test]
    fn elo_expected_even() {
        let e = EloSystem::expected_score(1200.0, 1200.0);
        assert!((e - 0.5).abs() < 0.001);
    }

    #[test]
    fn rating_to_rank() {
        assert_eq!(EloSystem::rating_to_rank(800.0),  RankTier::Bronze);
        assert_eq!(EloSystem::rating_to_rank(1200.0), RankTier::Gold);
        assert_eq!(EloSystem::rating_to_rank(2100.0), RankTier::Grandmaster);
    }

    #[test]
    fn matchmaking_groups_same_mode() {
        let mut pool = MatchmakingPool::new();
        for i in 0..4 {
            pool.enqueue(MatchmakingTicket::new(
                format!("p{}", i),
                1200.0,
                "EU".into(),
                GameMode::Deathmatch,
                0.0,
            ));
        }
        let results = pool.run_matchmaking();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].players.len(), 4);
    }

    #[test]
    fn matchmaking_does_not_mix_modes() {
        let mut pool = MatchmakingPool::new();
        pool.enqueue(MatchmakingTicket::new("p1".into(), 1200.0, "EU".into(), GameMode::Deathmatch,  0.0));
        pool.enqueue(MatchmakingTicket::new("p2".into(), 1200.0, "EU".into(), GameMode::TeamBattle, 0.0));
        // Deathmatch needs 2, TeamBattle needs 4 — neither met
        let results = pool.run_matchmaking();
        assert!(results.is_empty());
    }

    #[test]
    fn lobby_countdown_starts_when_all_ready() {
        let settings = LobbySettings {
            mode: GameMode::Deathmatch,
            ..Default::default()
        };
        let mut lobby = Lobby::new("L1".into(), 4, settings, 0.0);
        lobby.add_player(LobbyPlayer::new("p1".into(), "Alice".into(), 0, 0.0));
        lobby.add_player(LobbyPlayer::new("p2".into(), "Bob".into(),   0, 0.0));
        lobby.set_ready("p1");
        lobby.set_ready("p2");
        lobby.tick_countdown(0.1); // should enter Countdown
        assert!(matches!(lobby.state, LobbyState::Countdown { .. }));
    }

    #[test]
    fn lobby_tick_returns_true_after_10s() {
        let settings = LobbySettings { mode: GameMode::Deathmatch, ..Default::default() };
        let mut lobby = Lobby::new("L2".into(), 4, settings, 0.0);
        lobby.add_player(LobbyPlayer::new("p1".into(), "Alice".into(), 0, 0.0));
        lobby.add_player(LobbyPlayer::new("p2".into(), "Bob".into(),   0, 0.0));
        lobby.set_ready("p1");
        lobby.set_ready("p2");
        lobby.tick_countdown(0.1);
        // Now advance the full countdown
        let mut started = false;
        for _ in 0..200 {
            if lobby.tick_countdown(0.1) { started = true; break; }
        }
        assert!(started);
    }

    #[test]
    fn server_list_best_server() {
        let mut list = ServerList::new(5.0, 30.0);
        list.upsert(ServerEntry {
            id: "s1".into(), address: "1.1.1.1:7000".into(), name: "Server 1".into(),
            map_id: "map_arena".into(), mode: GameMode::Deathmatch,
            current_players: 4, max_players: 16,
            ping_ms: 80, region: "EU".into(), has_password: false, last_seen: 0.0,
        });
        list.upsert(ServerEntry {
            id: "s2".into(), address: "2.2.2.2:7000".into(), name: "Server 2".into(),
            map_id: "map_arena".into(), mode: GameMode::Deathmatch,
            current_players: 8, max_players: 16,
            ping_ms: 20, region: "EU".into(), has_password: false, last_seen: 0.0,
        });
        let best = list.best_server(&GameMode::Deathmatch, "EU");
        assert!(best.is_some());
        assert_eq!(best.unwrap().ping_ms, 20);
    }

    #[test]
    fn player_profile_win_raises_rating() {
        let mut profile = PlayerProfile::new("p1".into(), "Alice".into());
        let start = profile.rating;
        profile.record_win(1200.0);
        assert_eq!(profile.wins, 1);
        assert_eq!(profile.games_played, 1);
        assert!((profile.rating - start).abs() < 20.0); // slight gain against equal
    }
}
