//! Multiplayer lobby system: lobby management, browser, matchmaking,
//! team assignment, ready-checks, and voice-chat metadata.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

// ─── IDs ─────────────────────────────────────────────────────────────────────

/// Opaque lobby identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LobbyId(pub u64);

/// Opaque player identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerId(pub u64);

/// Opaque team identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TeamId(pub u8);

/// Opaque game-mode identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameMode(pub u8);

impl GameMode {
    pub const DEATHMATCH:   Self = Self(0);
    pub const TEAM_VS_TEAM: Self = Self(1);
    pub const CAPTURE_FLAG: Self = Self(2);
    pub const KING_HILL:    Self = Self(3);
    pub const CUSTOM:       Self = Self(0xFF);
}

// ─── LobbyState ──────────────────────────────────────────────────────────────

/// Lifecycle phase of a lobby.
#[derive(Debug, Clone, PartialEq)]
pub enum LobbyState {
    /// Waiting for players; lobby is open.
    Waiting,
    /// Countdown in progress.  `f32` = remaining seconds.
    Countdown(f32),
    /// A match is actively running.
    InGame,
    /// Match has ended; displaying results.
    Postgame,
}

impl LobbyState {
    pub fn is_joinable(&self) -> bool {
        matches!(self, LobbyState::Waiting)
    }
}

// ─── LobbyConfig ─────────────────────────────────────────────────────────────

/// Immutable configuration set when the lobby is created.
#[derive(Debug, Clone)]
pub struct LobbyConfig {
    pub max_players: u8,
    pub min_players: u8,
    pub game_mode:   GameMode,
    pub map_id:      u32,
    /// Optional password.  Empty string = no password.
    pub password:    String,
    pub public:      bool,
    pub ranked:      bool,
    /// Duration of the pre-game countdown in seconds.
    pub countdown_secs: f32,
}

impl Default for LobbyConfig {
    fn default() -> Self {
        Self {
            max_players:    8,
            min_players:    2,
            game_mode:      GameMode::DEATHMATCH,
            map_id:         0,
            password:       String::new(),
            public:         true,
            ranked:         false,
            countdown_secs: 10.0,
        }
    }
}

impl LobbyConfig {
    pub fn has_password(&self) -> bool { !self.password.is_empty() }
}

// ─── LobbyPlayer ─────────────────────────────────────────────────────────────

/// State of one player inside a lobby.
#[derive(Debug, Clone)]
pub struct LobbyPlayer {
    pub id:        PlayerId,
    pub name:      String,
    pub ready:     bool,
    pub team:      Option<TeamId>,
    pub ping_ms:   u32,
    pub spectator: bool,
}

impl LobbyPlayer {
    pub fn new(id: PlayerId, name: impl Into<String>) -> Self {
        Self {
            id, name: name.into(), ready: false,
            team: None, ping_ms: 0, spectator: false,
        }
    }
}

// ─── Lobby ───────────────────────────────────────────────────────────────────

/// A single server-side lobby instance.
#[derive(Debug)]
pub struct Lobby {
    pub id:        LobbyId,
    pub name:      String,
    pub host_id:   PlayerId,
    pub players:   Vec<LobbyPlayer>,
    pub state:     LobbyState,
    pub config:    LobbyConfig,
    created_at:    Instant,
    countdown_started: Option<Instant>,
}

impl Lobby {
    pub fn new(id: LobbyId, name: impl Into<String>, host_id: PlayerId, config: LobbyConfig) -> Self {
        Self {
            id, name: name.into(), host_id,
            players: Vec::new(), state: LobbyState::Waiting,
            config, created_at: Instant::now(), countdown_started: None,
        }
    }

    /// Returns `true` if there is room for one more player.
    pub fn has_room(&self) -> bool {
        self.players.len() < self.config.max_players as usize
    }

    /// Returns `true` if the lobby has enough players to start.
    pub fn has_min_players(&self) -> bool {
        self.players.len() >= self.config.min_players as usize
    }

    pub fn all_ready(&self) -> bool {
        !self.players.is_empty()
            && self.players.iter().filter(|p| !p.spectator).all(|p| p.ready)
    }

    pub fn player(&self, id: PlayerId) -> Option<&LobbyPlayer> {
        self.players.iter().find(|p| p.id == id)
    }

    pub fn player_mut(&mut self, id: PlayerId) -> Option<&mut LobbyPlayer> {
        self.players.iter_mut().find(|p| p.id == id)
    }

    pub fn contains(&self, id: PlayerId) -> bool {
        self.players.iter().any(|p| p.id == id)
    }

    /// Tick the countdown; returns `true` when countdown expires.
    pub fn tick_countdown(&mut self, dt: f32) -> bool {
        if let LobbyState::Countdown(ref mut t) = self.state {
            *t -= dt;
            if *t <= 0.0 {
                return true;
            }
        }
        false
    }

    pub fn player_count(&self) -> usize { self.players.len() }
    pub fn spectator_count(&self) -> usize { self.players.iter().filter(|p| p.spectator).count() }
    pub fn active_count(&self) -> usize { self.players.iter().filter(|p| !p.spectator).count() }
}

// ─── LobbyError ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyError {
    LobbyNotFound(LobbyId),
    LobbyFull,
    LobbyNotJoinable,
    WrongPassword,
    PlayerNotFound(PlayerId),
    PlayerAlreadyInLobby,
    NotHost,
    NotEnoughPlayers,
    AlreadyInGame,
    CannotKickSelf,
    TeamNotFound(TeamId),
    MatchmakingError(String),
}

impl std::fmt::Display for LobbyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for LobbyError {}

// ─── LobbyManager ────────────────────────────────────────────────────────────

/// Server-side manager for all active lobbies.
pub struct LobbyManager {
    lobbies:       HashMap<LobbyId, Lobby>,
    /// Maps player → their current lobby (if any).
    player_lobby:  HashMap<PlayerId, LobbyId>,
    next_lobby_id: u64,
}

impl LobbyManager {
    pub fn new() -> Self {
        Self {
            lobbies: HashMap::new(),
            player_lobby: HashMap::new(),
            next_lobby_id: 1,
        }
    }

    fn alloc_id(&mut self) -> LobbyId {
        let id = LobbyId(self.next_lobby_id);
        self.next_lobby_id += 1;
        id
    }

    /// Create a new lobby.  The host is automatically added as the first player.
    pub fn create_lobby(
        &mut self,
        host_id: PlayerId,
        host_name: impl Into<String>,
        name: impl Into<String>,
        config: LobbyConfig,
    ) -> Result<LobbyId, LobbyError> {
        // A player can only be in one lobby at a time
        if self.player_lobby.contains_key(&host_id) {
            return Err(LobbyError::PlayerAlreadyInLobby);
        }
        let id = self.alloc_id();
        let mut lobby = Lobby::new(id, name, host_id, config);
        lobby.players.push(LobbyPlayer::new(host_id, host_name));
        self.player_lobby.insert(host_id, id);
        self.lobbies.insert(id, lobby);
        Ok(id)
    }

    /// Destroy a lobby, removing all players from the tracking map.
    pub fn destroy_lobby(&mut self, id: LobbyId) -> Result<(), LobbyError> {
        let lobby = self.lobbies.remove(&id).ok_or(LobbyError::LobbyNotFound(id))?;
        for p in &lobby.players {
            self.player_lobby.remove(&p.id);
        }
        Ok(())
    }

    /// Add a player to a lobby.
    pub fn join(
        &mut self,
        player_id: PlayerId,
        player_name: impl Into<String>,
        lobby_id: LobbyId,
        password: &str,
    ) -> Result<(), LobbyError> {
        if self.player_lobby.contains_key(&player_id) {
            return Err(LobbyError::PlayerAlreadyInLobby);
        }
        let lobby = self.lobbies.get_mut(&lobby_id).ok_or(LobbyError::LobbyNotFound(lobby_id))?;
        if !lobby.state.is_joinable() {
            return Err(LobbyError::LobbyNotJoinable);
        }
        if !lobby.has_room() {
            return Err(LobbyError::LobbyFull);
        }
        if lobby.config.has_password() && lobby.config.password != password {
            return Err(LobbyError::WrongPassword);
        }
        lobby.players.push(LobbyPlayer::new(player_id, player_name));
        self.player_lobby.insert(player_id, lobby_id);
        Ok(())
    }

    /// Remove a player from their current lobby.
    pub fn leave(&mut self, player_id: PlayerId) -> Result<LobbyId, LobbyError> {
        let lobby_id = self.player_lobby.remove(&player_id)
            .ok_or(LobbyError::PlayerNotFound(player_id))?;
        let lobby = self.lobbies.get_mut(&lobby_id)
            .ok_or(LobbyError::LobbyNotFound(lobby_id))?;
        lobby.players.retain(|p| p.id != player_id);

        // If host left, transfer host to next player or destroy
        if lobby.host_id == player_id {
            if let Some(new_host) = lobby.players.first() {
                lobby.host_id = new_host.id;
            } else {
                // Empty lobby — destroy
                self.lobbies.remove(&lobby_id);
            }
        }
        Ok(lobby_id)
    }

    /// Set the ready state of a player.
    pub fn set_ready(&mut self, player_id: PlayerId, ready: bool) -> Result<(), LobbyError> {
        let lobby_id = *self.player_lobby.get(&player_id)
            .ok_or(LobbyError::PlayerNotFound(player_id))?;
        let lobby = self.lobbies.get_mut(&lobby_id)
            .ok_or(LobbyError::LobbyNotFound(lobby_id))?;
        let player = lobby.player_mut(player_id)
            .ok_or(LobbyError::PlayerNotFound(player_id))?;
        player.ready = ready;
        Ok(())
    }

    /// Host kicks a player.
    pub fn kick(&mut self, host_id: PlayerId, target_id: PlayerId) -> Result<(), LobbyError> {
        if host_id == target_id {
            return Err(LobbyError::CannotKickSelf);
        }
        let lobby_id = *self.player_lobby.get(&host_id)
            .ok_or(LobbyError::PlayerNotFound(host_id))?;
        let lobby = self.lobbies.get(&lobby_id)
            .ok_or(LobbyError::LobbyNotFound(lobby_id))?;
        if lobby.host_id != host_id {
            return Err(LobbyError::NotHost);
        }
        if !lobby.contains(target_id) {
            return Err(LobbyError::PlayerNotFound(target_id));
        }
        self.leave(target_id)?;
        Ok(())
    }

    /// Attempt to start the game.  Called by host or automatically when all ready.
    /// Returns `Ok(())` and transitions lobby to `Countdown`.
    pub fn start_game(&mut self, host_id: PlayerId) -> Result<(), LobbyError> {
        let lobby_id = *self.player_lobby.get(&host_id)
            .ok_or(LobbyError::PlayerNotFound(host_id))?;
        let lobby = self.lobbies.get_mut(&lobby_id)
            .ok_or(LobbyError::LobbyNotFound(lobby_id))?;
        if lobby.host_id != host_id {
            return Err(LobbyError::NotHost);
        }
        if matches!(lobby.state, LobbyState::InGame) {
            return Err(LobbyError::AlreadyInGame);
        }
        if !lobby.has_min_players() {
            return Err(LobbyError::NotEnoughPlayers);
        }
        let secs = lobby.config.countdown_secs;
        lobby.state = LobbyState::Countdown(secs);
        lobby.countdown_started = Some(Instant::now());
        Ok(())
    }

    /// Tick all lobby countdowns.  Returns a list of lobby IDs that transitioned to `InGame`.
    pub fn tick(&mut self, dt: f32) -> Vec<LobbyId> {
        let mut started = Vec::new();
        for (id, lobby) in self.lobbies.iter_mut() {
            if lobby.tick_countdown(dt) {
                lobby.state = LobbyState::InGame;
                started.push(*id);
            }
        }
        started
    }

    /// Mark a lobby as finished and transition to Postgame.
    pub fn end_game(&mut self, lobby_id: LobbyId) -> Result<(), LobbyError> {
        let lobby = self.lobbies.get_mut(&lobby_id)
            .ok_or(LobbyError::LobbyNotFound(lobby_id))?;
        lobby.state = LobbyState::Postgame;
        Ok(())
    }

    /// Reset lobby back to waiting state.
    pub fn reset_lobby(&mut self, lobby_id: LobbyId) -> Result<(), LobbyError> {
        let lobby = self.lobbies.get_mut(&lobby_id)
            .ok_or(LobbyError::LobbyNotFound(lobby_id))?;
        lobby.state = LobbyState::Waiting;
        for p in lobby.players.iter_mut() {
            p.ready = false;
        }
        Ok(())
    }

    pub fn lobby(&self, id: LobbyId) -> Option<&Lobby> {
        self.lobbies.get(&id)
    }

    pub fn lobby_for_player(&self, player_id: PlayerId) -> Option<&Lobby> {
        let lid = self.player_lobby.get(&player_id)?;
        self.lobbies.get(lid)
    }

    pub fn lobby_count(&self) -> usize { self.lobbies.len() }

    /// All public lobbies sorted by player count (descending).
    pub fn public_lobbies(&self) -> Vec<&Lobby> {
        let mut list: Vec<&Lobby> = self.lobbies.values()
            .filter(|l| l.config.public && l.state.is_joinable())
            .collect();
        list.sort_by(|a, b| b.players.len().cmp(&a.players.len()));
        list
    }
}

impl Default for LobbyManager {
    fn default() -> Self { Self::new() }
}

// ─── LobbyInfo ────────────────────────────────────────────────────────────────

/// Lightweight snapshot of a lobby sent to browsing clients.
#[derive(Debug, Clone)]
pub struct LobbyInfo {
    pub id:           LobbyId,
    pub name:         String,
    pub player_count: u8,
    pub max_players:  u8,
    pub map_id:       u32,
    pub game_mode:    GameMode,
    /// Measured RTT from the browsing client's perspective.
    pub ping_ms:      u32,
    pub has_password: bool,
}

impl LobbyInfo {
    pub fn from_lobby(lobby: &Lobby, ping_ms: u32) -> Self {
        Self {
            id:           lobby.id,
            name:         lobby.name.clone(),
            player_count: lobby.players.len() as u8,
            max_players:  lobby.config.max_players,
            map_id:       lobby.config.map_id,
            game_mode:    lobby.config.game_mode,
            ping_ms,
            has_password: lobby.config.has_password(),
        }
    }
}

// ─── LobbyBrowser ─────────────────────────────────────────────────────────────

/// Client-side lobby listing with filter/sort and rate-limited refresh.
pub struct LobbyBrowser {
    listings: Vec<LobbyInfo>,
    last_refresh: Option<Instant>,
    /// Minimum time between refreshes.
    refresh_cooldown: Duration,
}

/// Sort order for lobby listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LobbySort {
    ByPlayerCount,
    ByPing,
    ByName,
    ByGameMode,
}

/// Filter predicate for lobby listings.
#[derive(Debug, Clone, Default)]
pub struct LobbyFilter {
    pub max_ping_ms:  Option<u32>,
    pub game_mode:    Option<GameMode>,
    pub map_id:       Option<u32>,
    pub name_substr:  Option<String>,
    pub hide_full:    bool,
    pub hide_private: bool,
}

impl LobbyBrowser {
    pub fn new(refresh_cooldown_ms: u64) -> Self {
        Self {
            listings: Vec::new(),
            last_refresh: None,
            refresh_cooldown: Duration::from_millis(refresh_cooldown_ms),
        }
    }

    /// Returns `true` if we are allowed to refresh now.
    pub fn can_refresh(&self) -> bool {
        match self.last_refresh {
            None => true,
            Some(t) => t.elapsed() >= self.refresh_cooldown,
        }
    }

    /// Update listings from a server response.
    pub fn update_listings(&mut self, infos: Vec<LobbyInfo>) {
        self.listings = infos;
        self.last_refresh = Some(Instant::now());
    }

    /// Apply filter then sort.
    pub fn filtered_sorted(&self, filter: &LobbyFilter, sort: LobbySort) -> Vec<&LobbyInfo> {
        let mut list: Vec<&LobbyInfo> = self.listings.iter()
            .filter(|l| {
                if let Some(max_ping) = filter.max_ping_ms {
                    if l.ping_ms > max_ping { return false; }
                }
                if let Some(gm) = filter.game_mode {
                    if l.game_mode != gm { return false; }
                }
                if let Some(map) = filter.map_id {
                    if l.map_id != map { return false; }
                }
                if let Some(ref substr) = filter.name_substr {
                    if !l.name.to_lowercase().contains(&substr.to_lowercase()) { return false; }
                }
                if filter.hide_full && l.player_count >= l.max_players { return false; }
                if filter.hide_private && l.has_password { return false; }
                true
            })
            .collect();

        match sort {
            LobbySort::ByPlayerCount => list.sort_by(|a, b| b.player_count.cmp(&a.player_count)),
            LobbySort::ByPing        => list.sort_by_key(|l| l.ping_ms),
            LobbySort::ByName        => list.sort_by(|a, b| a.name.cmp(&b.name)),
            LobbySort::ByGameMode    => list.sort_by_key(|l| l.game_mode.0),
        }
        list
    }

    pub fn listing_count(&self) -> usize { self.listings.len() }
}

// ─── MatchmakingQueue ─────────────────────────────────────────────────────────

/// Entry in the matchmaking queue.
#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub player_id:     PlayerId,
    pub skill_rating:  f32,
    pub queue_time:    Instant,
    /// Party ID (players with same party_id must be kept together).
    pub party_id:      Option<u64>,
    pub preferences:   MatchPreferences,
}

/// Player preferences for matchmaking.
#[derive(Debug, Clone, Default)]
pub struct MatchPreferences {
    pub game_mode:       Option<GameMode>,
    pub preferred_map:   Option<u32>,
    pub max_ping_ms:     Option<u32>,
    pub ranked:          bool,
}

/// A proposed match from the matchmaking system.
#[derive(Debug, Clone)]
pub struct ProposedMatch {
    pub players:   Vec<PlayerId>,
    pub game_mode: GameMode,
    pub map_id:    u32,
    pub avg_skill: f32,
}

/// Fill an existing game's open slot.
#[derive(Debug, Clone)]
pub struct BackfillJob {
    pub lobby_id:   LobbyId,
    pub open_slots: u8,
    pub skill_avg:  f32,
    pub game_mode:  GameMode,
    pub map_id:     u32,
}

pub struct MatchmakingQueue {
    queue:           VecDeque<QueueEntry>,
    backfill_jobs:   Vec<BackfillJob>,
    /// Maximum skill spread for initial match window.
    base_skill_range: f32,
    /// How much the range expands per second in queue.
    range_per_sec:    f32,
    /// Maximum range (after 60s at default settings: 500).
    max_skill_range:  f32,
    /// Required players per match.
    match_size:       usize,
}

impl MatchmakingQueue {
    pub fn new(match_size: usize) -> Self {
        Self {
            queue:            VecDeque::new(),
            backfill_jobs:    Vec::new(),
            base_skill_range: 50.0,
            range_per_sec:    (500.0 - 50.0) / 60.0, // expands to 500 over 60s
            max_skill_range:  500.0,
            match_size,
        }
    }

    /// Enqueue a player.
    pub fn enqueue(&mut self, entry: QueueEntry) {
        self.queue.push_back(entry);
    }

    /// Remove a player from the queue (cancelled or timed out).
    pub fn dequeue(&mut self, player_id: PlayerId) -> bool {
        let before = self.queue.len();
        self.queue.retain(|e| e.player_id != player_id);
        self.queue.len() < before
    }

    /// Skill range for an entry given how long it has been queuing.
    fn skill_range_for(&self, entry: &QueueEntry) -> f32 {
        let wait_secs = entry.queue_time.elapsed().as_secs_f32();
        (self.base_skill_range + self.range_per_sec * wait_secs).min(self.max_skill_range)
    }

    /// Attempt to form matches.  Returns a list of proposed matches.
    /// Party members are kept together.
    pub fn tick(&mut self) -> Vec<ProposedMatch> {
        let mut matched: Vec<ProposedMatch> = Vec::new();
        let mut used: std::collections::HashSet<usize> = std::collections::HashSet::new();

        let entries: Vec<(usize, &QueueEntry)> = self.queue.iter().enumerate().collect();

        'outer: for (i, anchor) in &entries {
            if used.contains(i) { continue; }

            let range = self.skill_range_for(anchor);
            let mut group: Vec<usize> = vec![*i];
            let anchor_party = anchor.party_id;

            for (j, candidate) in &entries {
                if used.contains(j) || j == i { continue; }
                if (candidate.skill_rating - anchor.skill_rating).abs() > range { continue; }
                // Party check: if the anchor is in a party, candidate must be too
                if let Some(ap) = anchor_party {
                    if candidate.party_id != Some(ap) && group.len() > 1 { continue; }
                }
                // Preference alignment
                if let Some(gm) = anchor.preferences.game_mode {
                    if candidate.preferences.game_mode.map_or(false, |cg| cg != gm) { continue; }
                }
                group.push(*j);
                if group.len() >= self.match_size {
                    break;
                }
            }

            if group.len() >= self.match_size {
                let players: Vec<PlayerId> = group.iter()
                    .map(|&idx| entries[idx].1.player_id)
                    .collect();
                let avg_skill = group.iter().map(|&idx| entries[idx].1.skill_rating).sum::<f32>()
                    / group.len() as f32;
                let game_mode = entries[group[0]].1.preferences.game_mode
                    .unwrap_or(GameMode::DEATHMATCH);

                for &idx in &group { used.insert(idx); }

                matched.push(ProposedMatch {
                    players,
                    game_mode,
                    map_id: 0,
                    avg_skill,
                });
            }
        }

        // Remove matched players from queue
        let matched_ids: std::collections::HashSet<PlayerId> = matched.iter()
            .flat_map(|m| m.players.iter().copied())
            .collect();
        self.queue.retain(|e| !matched_ids.contains(&e.player_id));

        matched
    }

    /// Register a backfill job (open slots in an ongoing game).
    pub fn add_backfill(&mut self, job: BackfillJob) {
        self.backfill_jobs.push(job);
    }

    /// Try to fill backfill jobs from the queue.
    /// Returns (BackfillJob, Vec<PlayerId>) pairs for each successful fill.
    pub fn process_backfill(&mut self) -> Vec<(BackfillJob, Vec<PlayerId>)> {
        let mut results = Vec::new();
        let mut filled_ids: std::collections::HashSet<PlayerId> = std::collections::HashSet::new();

        let jobs = std::mem::take(&mut self.backfill_jobs);
        for job in jobs {
            let mut candidates: Vec<PlayerId> = Vec::new();
            for entry in self.queue.iter() {
                if filled_ids.contains(&entry.player_id) { continue; }
                if entry.preferences.game_mode.map_or(false, |gm| gm != job.game_mode) { continue; }
                if (entry.skill_rating - job.skill_avg).abs() > 200.0 { continue; }
                candidates.push(entry.player_id);
                if candidates.len() >= job.open_slots as usize { break; }
            }
            if !candidates.is_empty() {
                for &pid in &candidates { filled_ids.insert(pid); }
                results.push((job, candidates));
            } else {
                self.backfill_jobs.push(job); // put back
            }
        }
        self.queue.retain(|e| !filled_ids.contains(&e.player_id));
        results
    }

    pub fn queue_len(&self) -> usize { self.queue.len() }
    pub fn backfill_count(&self) -> usize { self.backfill_jobs.len() }
}

// ─── VoiceChat ────────────────────────────────────────────────────────────────

/// Voice channel type (metadata only — no actual audio transport).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceChannel {
    /// All players in the lobby hear each other.
    Lobby,
    /// Only members of the same team.
    Team(TeamId),
    /// Direct voice to one player.
    Direct(PlayerId),
}

/// Player's current voice state.
#[derive(Debug, Clone)]
pub struct VoiceState {
    pub player_id:   PlayerId,
    pub channel:     VoiceChannel,
    pub muted:       bool,
    pub deafened:    bool,
    pub push_to_talk: bool,
    pub speaking:    bool,
}

impl VoiceState {
    pub fn new(player_id: PlayerId) -> Self {
        Self {
            player_id, channel: VoiceChannel::Lobby,
            muted: false, deafened: false, push_to_talk: false, speaking: false,
        }
    }
}

/// Voice-chat metadata manager for a lobby.
pub struct VoiceChatManager {
    states: HashMap<PlayerId, VoiceState>,
}

impl VoiceChatManager {
    pub fn new() -> Self { Self { states: HashMap::new() } }

    pub fn add_player(&mut self, player_id: PlayerId) {
        self.states.insert(player_id, VoiceState::new(player_id));
    }

    pub fn remove_player(&mut self, player_id: PlayerId) {
        self.states.remove(&player_id);
    }

    pub fn set_muted(&mut self, player_id: PlayerId, muted: bool) {
        if let Some(s) = self.states.get_mut(&player_id) { s.muted = muted; }
    }

    pub fn set_deafened(&mut self, player_id: PlayerId, deafened: bool) {
        if let Some(s) = self.states.get_mut(&player_id) { s.deafened = deafened; }
    }

    pub fn set_push_to_talk(&mut self, player_id: PlayerId, ptt: bool) {
        if let Some(s) = self.states.get_mut(&player_id) { s.push_to_talk = ptt; }
    }

    pub fn set_speaking(&mut self, player_id: PlayerId, speaking: bool) {
        if let Some(s) = self.states.get_mut(&player_id) {
            if !s.muted { s.speaking = speaking; }
        }
    }

    pub fn set_channel(&mut self, player_id: PlayerId, channel: VoiceChannel) {
        if let Some(s) = self.states.get_mut(&player_id) { s.channel = channel; }
    }

    /// Returns the list of players that `listener` can hear in the current state.
    pub fn audible_speakers(&self, listener: PlayerId) -> Vec<PlayerId> {
        let listener_state = match self.states.get(&listener) {
            Some(s) => s,
            None => return Vec::new(),
        };
        if listener_state.deafened { return Vec::new(); }

        self.states.values()
            .filter(|s| s.player_id != listener && s.speaking && !s.muted)
            .filter(|s| {
                // Can hear if: same channel or global channel
                match &s.channel {
                    VoiceChannel::Lobby => true,
                    VoiceChannel::Team(t) => {
                        listener_state.channel == VoiceChannel::Team(*t)
                    }
                    VoiceChannel::Direct(target) => *target == listener,
                }
            })
            .map(|s| s.player_id)
            .collect()
    }
}

impl Default for VoiceChatManager {
    fn default() -> Self { Self::new() }
}

// ─── TeamSystem ───────────────────────────────────────────────────────────────

/// Balance strategy for team assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamBalance {
    /// Server automatically assigns to minimize skill imbalance.
    Auto,
    /// Random assignment.
    Random,
    /// Players choose manually; server enforces max-per-team.
    Manual,
}

/// A single team.
#[derive(Debug, Clone)]
pub struct Team {
    pub id:      TeamId,
    pub name:    String,
    pub players: Vec<PlayerId>,
    pub max_size: u8,
}

impl Team {
    pub fn new(id: TeamId, name: impl Into<String>, max_size: u8) -> Self {
        Self { id, name: name.into(), players: Vec::new(), max_size }
    }
    pub fn is_full(&self) -> bool { self.players.len() >= self.max_size as usize }
    pub fn has_player(&self, pid: PlayerId) -> bool { self.players.contains(&pid) }
}

/// Manages team composition and automatic balancing.
pub struct TeamSystem {
    pub teams:        Vec<Team>,
    pub balance_mode: TeamBalance,
    rng_state:        u64, // simple LCG for random assignment
}

impl TeamSystem {
    pub fn new(balance_mode: TeamBalance) -> Self {
        Self { teams: Vec::new(), balance_mode, rng_state: 0xDEAD_BEEF_CAFE_BABE }
    }

    fn lcg_rand(&mut self) -> u64 {
        self.rng_state = self.rng_state.wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.rng_state
    }

    pub fn add_team(&mut self, id: TeamId, name: impl Into<String>, max_size: u8) {
        self.teams.push(Team::new(id, name, max_size));
    }

    pub fn remove_team(&mut self, id: TeamId) {
        self.teams.retain(|t| t.id != id);
    }

    pub fn team(&self, id: TeamId) -> Option<&Team> {
        self.teams.iter().find(|t| t.id == id)
    }

    pub fn team_mut(&mut self, id: TeamId) -> Option<&mut Team> {
        self.teams.iter_mut().find(|t| t.id == id)
    }

    /// Assign `player_id` to a team based on `balance_mode`.
    /// Returns the assigned `TeamId`.
    pub fn assign(
        &mut self,
        player_id: PlayerId,
        skill_rating: f32,
        preferred_team: Option<TeamId>,
    ) -> Result<TeamId, LobbyError> {
        if self.teams.is_empty() {
            return Err(LobbyError::TeamNotFound(TeamId(0)));
        }

        match self.balance_mode {
            TeamBalance::Manual => {
                let tid = preferred_team.ok_or(LobbyError::TeamNotFound(TeamId(0)))?;
                let team = self.team_mut(tid).ok_or(LobbyError::TeamNotFound(tid))?;
                if team.is_full() { return Err(LobbyError::LobbyFull); }
                team.players.push(player_id);
                Ok(tid)
            }
            TeamBalance::Random => {
                let n = self.teams.len();
                let idx = (self.lcg_rand() as usize) % n;
                // Try to find a non-full team starting from idx
                for offset in 0..n {
                    let team = &mut self.teams[(idx + offset) % n];
                    if !team.is_full() {
                        team.players.push(player_id);
                        return Ok(team.id);
                    }
                }
                Err(LobbyError::LobbyFull)
            }
            TeamBalance::Auto => {
                // Assign to team with lowest total skill rating that has room
                let mut best_idx = None;
                let mut best_skill = f32::MAX;

                // Compute each team's current skill sum
                // (We'd normally have access to a skill map, but for now assign to smallest team)
                let _ = skill_rating; // used for future skill-based balancing

                for (i, team) in self.teams.iter().enumerate() {
                    if team.is_full() { continue; }
                    // Use player count as a proxy: prefer smaller teams
                    let proxy = team.players.len() as f32;
                    if proxy < best_skill {
                        best_skill = proxy;
                        best_idx = Some(i);
                    }
                }
                if let Some(i) = best_idx {
                    let tid = self.teams[i].id;
                    self.teams[i].players.push(player_id);
                    Ok(tid)
                } else {
                    Err(LobbyError::LobbyFull)
                }
            }
        }
    }

    /// Remove a player from all teams.
    pub fn remove_player(&mut self, player_id: PlayerId) {
        for team in &mut self.teams {
            team.players.retain(|&p| p != player_id);
        }
    }

    /// Rebalance teams by moving one player from the largest team to the smallest.
    /// Returns the player moved if any.
    pub fn rebalance(&mut self) -> Option<(PlayerId, TeamId, TeamId)> {
        if self.teams.len() < 2 { return None; }

        let max_idx = self.teams.iter().enumerate().max_by_key(|(_, t)| t.players.len())?.0;
        let min_idx = self.teams.iter().enumerate().min_by_key(|(_, t)| t.players.len())?.0;

        if self.teams[max_idx].players.len() <= self.teams[min_idx].players.len() + 1 {
            return None; // already balanced
        }

        let player = *self.teams[max_idx].players.last()?;
        self.teams[max_idx].players.pop();
        let to = self.teams[min_idx].id;
        let from = self.teams[max_idx].id;
        self.teams[min_idx].players.push(player);
        Some((player, from, to))
    }
}

// ─── ReadyCheck ───────────────────────────────────────────────────────────────

/// Vote-based ready check before game start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadyCheckState {
    Pending,
    AllReady,
    TimedOut,
    Cancelled,
}

pub struct ReadyCheck {
    pub votes: HashMap<PlayerId, bool>,
    pub state: ReadyCheckState,
    started_at: Instant,
    timeout_secs: f32,
}

impl ReadyCheck {
    pub fn new(timeout_secs: f32) -> Self {
        Self {
            votes: HashMap::new(),
            state: ReadyCheckState::Pending,
            started_at: Instant::now(),
            timeout_secs,
        }
    }

    /// Register a player as participating in this check.
    pub fn add_participant(&mut self, player_id: PlayerId) {
        self.votes.insert(player_id, false);
    }

    /// Record a vote.
    pub fn vote(&mut self, player_id: PlayerId, ready: bool) {
        if let Some(v) = self.votes.get_mut(&player_id) {
            *v = ready;
        }
    }

    /// Check if all participants voted ready.
    pub fn check(&mut self) -> ReadyCheckState {
        if self.state != ReadyCheckState::Pending {
            return self.state.clone();
        }
        if self.started_at.elapsed().as_secs_f32() >= self.timeout_secs {
            self.state = ReadyCheckState::TimedOut;
            return self.state.clone();
        }
        if self.votes.values().all(|&v| v) && !self.votes.is_empty() {
            self.state = ReadyCheckState::AllReady;
        }
        self.state.clone()
    }

    pub fn cancel(&mut self) { self.state = ReadyCheckState::Cancelled; }

    pub fn ready_count(&self) -> usize { self.votes.values().filter(|&&v| v).count() }
    pub fn total_count(&self) -> usize { self.votes.len() }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(n: u64) -> PlayerId { PlayerId(n) }
    fn lid(n: u64) -> LobbyId { LobbyId(n) }

    fn default_config() -> LobbyConfig {
        LobbyConfig {
            max_players: 4,
            min_players: 2,
            countdown_secs: 0.01, // very short for tests
            ..LobbyConfig::default()
        }
    }

    // ── LobbyManager ─────────────────────────────────────────────────────────

    #[test]
    fn test_create_and_destroy_lobby() {
        let mut mgr = LobbyManager::new();
        let lid = mgr.create_lobby(pid(1), "Alice", "Alice's lobby", default_config()).unwrap();
        assert_eq!(mgr.lobby_count(), 1);
        mgr.destroy_lobby(lid).unwrap();
        assert_eq!(mgr.lobby_count(), 0);
    }

    #[test]
    fn test_join_leave() {
        let mut mgr = LobbyManager::new();
        let lid = mgr.create_lobby(pid(1), "Host", "Lobby", default_config()).unwrap();
        mgr.join(pid(2), "P2", lid, "").unwrap();
        assert_eq!(mgr.lobby(lid).unwrap().player_count(), 2);
        mgr.leave(pid(2)).unwrap();
        assert_eq!(mgr.lobby(lid).unwrap().player_count(), 1);
    }

    #[test]
    fn test_kick() {
        let mut mgr = LobbyManager::new();
        let lid = mgr.create_lobby(pid(1), "Host", "Lobby", default_config()).unwrap();
        mgr.join(pid(2), "P2", lid, "").unwrap();
        mgr.kick(pid(1), pid(2)).unwrap();
        assert_eq!(mgr.lobby(lid).unwrap().player_count(), 1);
    }

    #[test]
    fn test_start_game_requires_min_players() {
        let mut mgr = LobbyManager::new();
        let lid = mgr.create_lobby(pid(1), "Host", "Lobby", default_config()).unwrap();
        // Only 1 player, min is 2
        assert_eq!(mgr.start_game(pid(1)), Err(LobbyError::NotEnoughPlayers));
        mgr.join(pid(2), "P2", lid, "").unwrap();
        assert!(mgr.start_game(pid(1)).is_ok());
    }

    #[test]
    fn test_lobby_password() {
        let mut mgr = LobbyManager::new();
        let config = LobbyConfig { password: "secret".into(), ..default_config() };
        let lid = mgr.create_lobby(pid(1), "Host", "Lobby", config).unwrap();
        assert_eq!(mgr.join(pid(2), "P2", lid, "wrong"), Err(LobbyError::WrongPassword));
        assert!(mgr.join(pid(2), "P2", lid, "secret").is_ok());
    }

    #[test]
    fn test_lobby_full() {
        let mut mgr = LobbyManager::new();
        let config = LobbyConfig { max_players: 2, ..default_config() };
        let lid = mgr.create_lobby(pid(1), "Host", "Lobby", config).unwrap();
        mgr.join(pid(2), "P2", lid, "").unwrap();
        assert_eq!(mgr.join(pid(3), "P3", lid, ""), Err(LobbyError::LobbyFull));
    }

    // ── TeamSystem ────────────────────────────────────────────────────────────

    #[test]
    fn test_team_auto_balance() {
        let mut ts = TeamSystem::new(TeamBalance::Auto);
        ts.add_team(TeamId(0), "Red", 4);
        ts.add_team(TeamId(1), "Blue", 4);

        for i in 0..4u64 {
            ts.assign(pid(i), 1000.0, None).unwrap();
        }
        let r = ts.team(TeamId(0)).unwrap().players.len();
        let b = ts.team(TeamId(1)).unwrap().players.len();
        assert_eq!(r + b, 4);
        assert!((r as i32 - b as i32).abs() <= 1);
    }

    #[test]
    fn test_team_rebalance() {
        let mut ts = TeamSystem::new(TeamBalance::Manual);
        ts.add_team(TeamId(0), "Red", 8);
        ts.add_team(TeamId(1), "Blue", 8);
        // Put 3 on Red, 1 on Blue
        for p in [0u64, 1, 2] {
            ts.team_mut(TeamId(0)).unwrap().players.push(pid(p));
        }
        ts.team_mut(TeamId(1)).unwrap().players.push(pid(3));
        let result = ts.rebalance();
        assert!(result.is_some());
        let r = ts.team(TeamId(0)).unwrap().players.len();
        let b = ts.team(TeamId(1)).unwrap().players.len();
        assert_eq!(r, 2);
        assert_eq!(b, 2);
    }

    // ── ReadyCheck ────────────────────────────────────────────────────────────

    #[test]
    fn test_ready_check_all_ready() {
        let mut rc = ReadyCheck::new(30.0);
        rc.add_participant(pid(1));
        rc.add_participant(pid(2));
        rc.vote(pid(1), true);
        rc.vote(pid(2), true);
        assert_eq!(rc.check(), ReadyCheckState::AllReady);
    }

    #[test]
    fn test_ready_check_not_all_ready() {
        let mut rc = ReadyCheck::new(30.0);
        rc.add_participant(pid(1));
        rc.add_participant(pid(2));
        rc.vote(pid(1), true);
        assert_eq!(rc.check(), ReadyCheckState::Pending);
    }

    // ── MatchmakingQueue ──────────────────────────────────────────────────────

    #[test]
    fn test_matchmaking_basic() {
        let mut q = MatchmakingQueue::new(2);
        for i in 0..2u64 {
            q.enqueue(QueueEntry {
                player_id:    pid(i),
                skill_rating: 1000.0,
                queue_time:   Instant::now(),
                party_id:     None,
                preferences:  MatchPreferences::default(),
            });
        }
        let matches = q.tick();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].players.len(), 2);
    }

    // ── LobbyBrowser ─────────────────────────────────────────────────────────

    #[test]
    fn test_lobby_browser_filter_and_sort() {
        let mut browser = LobbyBrowser::new(1000);
        browser.update_listings(vec![
            LobbyInfo { id: lid(1), name: "Alpha".into(), player_count: 3, max_players: 8,
                        map_id: 1, game_mode: GameMode::DEATHMATCH, ping_ms: 50, has_password: false },
            LobbyInfo { id: lid(2), name: "Beta".into(), player_count: 1, max_players: 8,
                        map_id: 2, game_mode: GameMode::TEAM_VS_TEAM, ping_ms: 200, has_password: true },
        ]);
        let filter = LobbyFilter { hide_private: true, ..LobbyFilter::default() };
        let list = browser.filtered_sorted(&filter, LobbySort::ByPlayerCount);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, lid(1));
    }
}
