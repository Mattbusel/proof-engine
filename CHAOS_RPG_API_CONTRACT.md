# Proof Engine ↔ CHAOS RPG Integration Contract

Generated from reading `chaos-rpg/` source on 2026-03-24.
This document defines everything proof-engine must support to replace the bracket-lib graphical frontend.

---

## 1. Core Types the Renderer Must Read

### Character (`core/src/character.rs`)
- `CharacterClass` — 12 classes (Mage, Berserker, Ranger, Thief, Necromancer, Alchemist, Paladin, VoidWalker, Warlord, Trickster, Runesmith, Chronomancer)
- `StatBlock` — vitality, force, mana, cunning, precision, entropy, luck (7 stats)
- `Character` — full player state: class, stats, hp, max_hp, level, xp, gold, inventory, spells, boons, passive tree allocations, status effects, misery state
- `Background`, `Boon`, `Difficulty`

### Enemy (`core/src/enemy.rs`)
- `Enemy` — name, tier, hp, max_hp, base_damage, attack_modifier, chaos_level, ascii_sprite, special_ability, floor_ability
- `EnemyTier` — Minion, Elite, Champion, Boss, Abomination
- `FloorAbility` — None, StatMirror, EngineTheft, NullifyAura

### Combat (`core/src/combat.rs`)
- `CombatAction` — Attack, HeavyAttack, Defend, UseSpell(usize), UseItem(usize), Flee, Taunt
- `CombatOutcome` — PlayerWon, PlayerDied, PlayerFled, Ongoing
- `CombatState` — round, defending, last_action, status_effects on both sides
- `CombatEvent` — all 11 variants (PlayerAttack, EnemyAttack, SpellCast, EnemyDied, etc.)

### Chaos Pipeline (`core/src/chaos_pipeline.rs`)
- `ChaosRollResult` — final_value (f64), chain (Vec<ChainStep>), game_value (i64)
- `ChainStep` — engine_name, input, output, seed_used
- Methods: `is_critical()`, `is_catastrophe()`, `is_success()`, `combat_trace_lines()`

### World (`core/src/world.rs`)
- `Floor` — rooms: Vec<Room>, current_room_index, floor_number, env_effect
- `Room` — room_type, seed, visited, index
- `RoomType` — Combat, Treasure, Shop, Shrine, Trap, Boss, Portal, Empty, ChaosRift, CraftingBench
- `EnvEffect` — ManaBoost, DamageAura, SpeedBoost, ChaosAmplify, StatDebuff, VisionBlur, GoldMultiplier

### Power Tier (`core/src/power_tier.rs`)
- `PowerTier` — 40+ tiers from TheVoid through OMEGA
- `TierEffect` — Normal, Rainbow, RainbowFast, Pulse, Flash, Glitch, Inverted, Static, Freeze, Fading, PureBlack, FullFlash, GoldFlash, BoldWhiteFlash, DarkRainbow

### Items (`core/src/items.rs`)
- `Item` — name, base_type, rarity, stat_modifiers, is_weapon, durability, max_durability
- `Rarity` — Normal, Magic, Rare, Unique, Artifact, Cursed
- `StatModifier` — stat name + value

### Spells (`core/src/spells.rs`)
- `Spell` — name, school, damage, mana_cost, description, level, cooldown
- `SpellSchool` — Fire, Ice, Lightning, Arcane, Nature, Shadow, Chaos

### Bosses (`core/src/bosses.rs`)
- Boss IDs 1–12: Mirror, Accountant, Fibonacci Hydra, Eigenstate, Taxman, Null, Ouroboros, Collatz Titan, Committee, Recursion, Paradox, Algorithm Reborn
- `boss_name(id)`, `boss_pool_for_floor(floor)`, `random_unique_boss(floor, seed)`
- `BossOutcome` — PlayerWon, PlayerDied, Escaped

### Nemesis (`core/src/nemesis.rs`)
- `NemesisRecord` — name, floor_defeated, abilities, promoted_times

### Misery System (`core/src/misery_system.rs`)
- `MiseryMilestone` — 11 milestones from ItGetsWorse (100) through NegativeGod (1_000_000)
- `MiserySource` — all damage/failure event sources

### Achievements (`core/src/achievements.rs`)
- `AchievementStore` — list of unlocked achievements
- `RunSummary`, `CombatSnapshot`

### Run History (`core/src/run_history.rs`)
- `RunHistory`, `RunRecord`, narrative events

### Scoreboard (`core/src/scoreboard.rs`)
- `ScoreEntry` — name, score, floor, class, kills, etc.

### Daily Leaderboard (`core/src/daily_leaderboard.rs`)
- `LocalDailyStore`, `DailyEntry`, `LeaderboardRow`

### Config (`core/src/chaos_config.rs`)
- `ChaosConfig` — all configurable parameters (audio, visual, gameplay toggles)

### Audio Events (`core/src/audio_events.rs`)
- `AudioEvent` — 30+ variants covering all game events
- `MusicVibe`, `MusicState`

---

## 2. Core Functions the Renderer Calls

```rust
// Combat
resolve_action(state, action, player, enemy, seed) -> (CombatOutcome, Vec<CombatEvent>)
chaos_roll_verbose(input, seed) -> ChaosRollResult
destiny_roll(seed) -> ChaosRollResult

// World
generate_floor(floor_num, seed) -> Floor
room_enemy(room: &Room, floor_num) -> Enemy

// Character
Character::apply_level_up(...)
Character::total_stats() -> i64   // for power tier calc
Character::power_tier() -> PowerTier

// Items
Item::generate(floor, seed) -> Item
Item::rarity_color() -> (u8,u8,u8)

// Enemies
generate_enemy(floor, seed) -> Enemy

// Bosses
boss_pool_for_floor(floor) -> Vec<u8>
random_unique_boss(floor, seed) -> Option<u8>
run_boss_fight(id, player, seed, config) -> BossOutcome   // unique boss dispatch

// Nemesis
load_nemesis() -> Option<NemesisRecord>
save_nemesis(record)
clear_nemesis()

// Persistence
AchievementStore::load() / save()
RunHistory::load() / append(record)
ChaosConfig::load()
load_scores() / save_score(entry)
LocalDailyStore::load() / save()

// Daily
submit_score(entry) -> Result<()>
fetch_scores(seed) -> Result<Vec<LeaderboardRow>>

// Skill checks
perform_skill_check(skill_type, difficulty, player) -> SkillCheckResult
```

---

## 3. All 22 Screens

| Screen | Key Content |
|---|---|
| `Title` | Logo assembly animation, 5 theme options, menu items, save-continue option |
| `Tutorial` | Multi-slide tutorial with navigation (15 slides) |
| `ModeSelect` | Story / Infinite / Daily mode selection |
| `CharacterCreation` | Class select (12), background (4), difficulty (3), name input |
| `BoonSelect` | 3 random boons shown, select one |
| `FloorNav` | Floor map grid, room icons, current room cursor, floor stats |
| `RoomView` | Room event text, choices, pending item/spell display |
| `Combat` | Player panel, enemy panel, action menu, combat log, chaos trace, particles, boss overlays |
| `Shop` | NPC text, item list with prices, heal option |
| `Crafting` | Item list, operation select (8 ops: Reforge, Corrupt, Shatter, Imbue, Salvage, Upgrade, Mirror, Socket) |
| `CharacterSheet` | 5 tabs: Stats, Inventory, Effects, Lore, Log |
| `BodyChart` | Body parts visualization, injury/buff markers |
| `PassiveTree` | 820+ node tree with scroll, allocation, keystone display |
| `GameOver` | Death recap, stats, shareable text, Hall of Misery entry |
| `Victory` | Story completion, final stats, score submission |
| `Scoreboard` | Local high scores list |
| `Achievements` | Achievement list with filter (All/Unlocked/Locked), scroll |
| `RunHistory` | Per-run records with narrative events, scroll |
| `DailyLeaderboard` | Daily seed rank table, fetch/submit UI |
| `Bestiary` | Enemy gallery with selected entry detail |
| `Codex` | Lore entries with selected entry detail |
| `Settings` | Config toggles, theme select, audio, visual timing |

---

## 4. Input Bindings (per screen)

### Global (any screen)
- `T` — cycle color theme
- `Escape` — back / quit confirm

### Title
- Arrow keys / `j`/`k` — menu cursor
- `Enter` — select
- `C` — continue save
- `T` — next theme

### Combat
- `A` — Attack
- `H` — Heavy Attack
- `D` — Defend
- `1`–`5` — Use spell (by index)
- `I` — open item menu (then 1–9 to use)
- `F` — Flee
- `T` — Taunt
- `C` — Character sheet overlay
- `V` — Chaos viz overlay toggle
- `L` — collapse/expand combat log
- `.` — Auto-play toggle

### Floor Nav
- Arrow keys / `hjkl` — move cursor
- `Enter` — enter room
- `C` — character sheet
- `M` — map zoom

### Character Sheet
- `1`–`5` — tab select (Stats, Inventory, Effects, Lore, Log)
- Arrow keys — scroll within tab
- `Escape` — back

### Shop
- Arrow keys — item cursor
- `B` — buy selected
- `H` — buy healing
- `Escape` — leave shop

### Crafting
- Arrow keys — cursor
- `Enter` / `Space` — confirm selection
- `Escape` — back / cancel
- Text input when `item_filter_active`

### Passive Tree
- Arrow keys — scroll
- `Space` / `Enter` — allocate node
- `Escape` — back

### Achievements / Run History / Bestiary / Codex
- Arrow keys — scroll / select entry
- `Escape` — back
- `1`/`2`/`3` — filter tabs (Achievements)

### Daily Leaderboard
- `F` — fetch scores
- `S` — submit score
- `Escape` — back

### Settings
- Arrow keys — option cursor
- `Enter` / `Space` — toggle option
- `+`/`-` — numeric adjustments

---

## 5. Visual Effects Currently Implemented

### Particles
- `emit_death_explosion` — 40 burst particles, radial explosion
- `emit_level_up_fountain` — 30 upward star/gold particles
- `emit_crit_burst` — 16 spark particles in circle
- `emit_hit_sparks` — N spark particles at impact
- `emit_loot_sparkle` — 12 orbiting sparkle particles (slow orbit)
- `emit_status_ambient` — Per-status effect ambient particles: Burn (orange sparks), Freeze (blue flakes), Poison (green bubbles), Bleed (red drips), Regen (green plus signs)
- `emit_stun_orbit` — Two stars orbiting the stunned entity
- `emit_room_ambient` — Per-room-type background particles (combat=red haze, treasure=gold sparkles, shrine=blue upward, chaos_rift=glitching random, boss=pulsing purple/red)
- `emit_boss_entrance_burst` — Per-boss-ID entrance animations (Mirror=symmetric split, Hydra=golden spiral, Committee=5 converging clusters, Algorithm=ring explosion)

### Borders / Overlays
- `player_flash` — Red border flash on player panel (on taking damage)
- `enemy_flash` — Colored border flash on enemy panel (on taking damage, color varies by damage type)
- `hit_shake` — Outer border shake on heavy crits / catastrophes
- `spell_beam` — Beam animation across screen during spell cast

### Bars
- `ghost_player_hp` / `ghost_enemy_hp` — Ghost HP bar showing previous HP level, lingers for `ghost_timer` frames
- `display_player_hp` / `display_enemy_hp` / `display_mp` — Smooth lerped display fractions (not snappping)

### Screen Transitions
- `floor_transition_timer` — Floor number transition overlay (shows "FLOOR N" text)
- `boss_entrance_timer` / `boss_entrance_name` — Boss name entrance cinematic
- `title_logo_timer` — Title screen logo particle assembly (90 frames)
- `room_entry_timer` — Room entry flash animation by room type
- `kill_linger` — Hold on combat screen for N frames after enemy death

### Post-Processing Systems (custom modules)
- `ChaosField` — Living mathematical background (2000 characters driven by math engines)
- `ColorGrade` — Screen-wide color grade (tint, saturation, contrast)
- `TileEffects` — Per-tile visual effects (floor staining, shimmer, corruption)
- `Weather` — Dynamic weather overlay (rain, snow, fog, corruption haze)
- `DeathSeq` — Full death cinematic sequence (tiles fall, screen shatters)
- `CombatAnim` — Weapon swing arcs, spell beam paths, hit stagger
- `NemesisReveal` — Nemesis encounter cinematic
- `AchievementBanner` / `RichBanner` — Multi-rarity achievement pop-up banners
- `AnimConfig` — All timing constants (FAST_MODE halves all durations)

### TierEffect Animations (power tier display)
- Rainbow, RainbowFast, Pulse, Flash, Glitch, Inverted, Static, Freeze, Fading, PureBlack, FullFlash, GoldFlash, BoldWhiteFlash, DarkRainbow

### Combat-Specific Overlays
- Boss visual overlays per boss ID (1–12) — each boss has unique rendering logic in `draw_boss_visual_overlay`
- Chaos pipeline viz overlay (`chaos_viz_open`) — shows the engine chain with colored step display
- Combat log collapse/expand
- Auto-play mode indicator

---

## 6. Architecture Notes

### What stays in core/ (unchanged)
- All game logic
- All math engines
- All type definitions
- Save/load utilities

### What the proof-engine graphical frontend replaces
- `graphical/src/main.rs` State struct → `graphical-proof/src/main.rs` ChaosRpgGame struct
- All `draw_*` functions → `render_*` methods using proof-engine Frame API
- All bracket-lib rendering calls → proof-engine GlyphCommand pushes
- Manual Particle struct → proof-engine MathParticle / ParticlePool
- Manual ChaosField → proof-engine chaos field simulation
- Manual screen shake → proof-engine camera trauma system
- Manual color grade → proof-engine post-processing pipeline
- Manual lerp functions → proof-engine SpringDamper / MathFunction

### The bracket-lib → proof-engine translation table

| bracket-lib | proof-engine |
|---|---|
| `ctx.set(x, y, fg, bg, ch)` | `frame.push_glyph(GlyphCommand { character: ch, position: vec3(x, y, 0), color: fg, .. })` |
| `ctx.print_color(x, y, fg, bg, str)` | Loop: push GlyphCommand per char |
| `ctx.draw_box(x, y, w, h, fg, bg)` | Push border GlyphCommands with box-drawing chars |
| `ctx.draw_bar_horizontal(...)` | Push block GlyphCommands (█ ░) |
| `hit_shake` | `engine.camera.add_trauma(intensity)` |
| `particles.push(Particle::burst(...))` | `frame.particles.push(ParticleCommand { behavior: MathFunction::..., .. })` |
| `color_grade.tint = red` | `frame.color_grade = ColorGrade { tint: red, .. }` |
| `THEMES[theme_idx]` | Engine-side theme system with same 5 themes |
