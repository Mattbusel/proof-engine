# Proof Engine

A mathematical rendering engine for Rust. **207,000+ lines of fully implemented systems across 236 source files.**

Every visual is the output of a mathematical function. Every animation is a continuous function over time. Every particle follows a real equation. Characters are rendered as textured quads in 3D space with bloom, distortion, motion blur, and force field physics.

## Philosophy

**Proof Engine does not render graphics. It renders mathematics.**

A traditional renderer draws shapes and colors that represent game state. Proof Engine computes mathematical functions and the visual IS the output. A Lorenz attractor does not look like a Lorenz attractor because someone drew one. It looks like a Lorenz attractor because 200 particles are following the actual differential equations in real time.

A damage hit does not look heavy because someone animated a heavy-looking effect. It looks heavy because a gravitational force field spawned at the impact point and every glyph in the vicinity was physically pulled toward it. The math is not a theme painted on top. The math is the rendering primitive.

### Weight and Life

Every object on screen has mathematical mass. Things with high values move slower, hit harder, and disturb the space around them more. A 3000-damage crit warps the surrounding space, bends nearby characters toward it, and leaves a gravitational afterimage. Stats are not numbers you read. They are forces you see acting on the screen.

The screen is never still. Every element is a living mathematical function that breathes, oscillates, and responds to its neighbors. A sine wave is alive. A Lorenz attractor is alive. The engine uses real mathematical objects as animation primitives instead of keyframes.

---

## What the Engine Can Do

### Rendering

- ASCII glyphs rendered as textured quads in 3D space with per-instance position, rotation, scale, color, emission, glow radius, and UV atlas region
- Instanced draw calls batched by render layer and blend mode (Alpha, Additive, Multiply, Screen)
- Six render layers: Background, World, Entity, Particle, Overlay, UI
- Font atlas rasterized at runtime from any system TTF via ab_glyph
- Full post-processing stack: 4-pass Gaussian bloom, chromatic aberration, film grain, CRT scanlines, vignette, color grading, screen-space distortion, velocity motion blur
- Dual-attachment scene FBO: color channel and emission channel feed separately into the bloom compositor
- Color science: linear/sRGB/HSV/HSL/Oklab/CIE Lab conversions, color difference metrics, tone mapping (Reinhard, ACES), LUT generation, gradient systems, palette extraction
- Node-based shader graph: 40+ node types (math, color, UV, noise, fractal, blend, filter, output), compiles to GLSL at runtime, with dead-node elimination, constant folding, and common subexpression sharing

### Color Grading

- Full CPU/GPU color grading pipeline: lift/gamma/gain per channel, exposure, contrast, saturation, hue shift, white balance
- Split toning: independent shadow and highlight tint with blend factor
- S-curves: per-channel and luminance curve control with cubic interpolation
- Vignette: oval mask with feathering and blend mode
- 3D LUT: 17x17x17 tetrahedral interpolation, import/export
- 9 built-in film looks: ACES, Kodak, Fuji, Noir, Golden Hour, Soft Beauty, RetroTV, Faded Film, Teal and Orange
- Animated grade keyframes: MathFunction-driven parameter animation over time
- GLSL shader source generation from grade parameters

### Mathematics

- 30+ MathFunction variants: Sine, Square, Triangle, Sawtooth, Noise, Breathing, Lorenz, Mandelbrot, Collatz, Fibonacci, Perlin, Spring, Orbit, Spiral, Heartbeat, BeatFrequency, WavePacket, FourierSeries, Sigmoid, VanDerPol, Duffing, TentMap, HenonMap, DoublePendulum, Projectile, SimpleHarmonic, DampedSine, Epicycle, FractionalBrownian, DomainWarp, Cellular, and more
- Utility methods on every function: derivative(), integrate(), sample_range(), zero_crossings(), evaluate_vec3()
- Complex number arithmetic with full transcendental functions (exp, ln, sqrt, sin, cos, tan, asin, acos, atan, sinh, cosh, tanh)
- Quaternion math: SLERP, axis-angle, Euler, exp map, log map, rotation of vectors
- Iterative fractals: Mandelbrot, Julia (8 presets), Burning Ship, Tricorn, Newton, Lyapunov -- all with smooth escape-time coloring and 7 palette types
- Parametric curves: quadratic/cubic/N-th Bezier, Catmull-Rom, Hermite, uniform B-Spline, CompositeCurve, arc-length tables for uniform-speed traversal, Frenet frames, CurveWalker iterator
- 7 strange attractors with RK4 integration: Lorenz, Rossler, Chen, Halvorsen, Aizawa, Thomas, Dadras -- Lyapunov exponent computation, warmup periods, full 3D trajectory output
- Force fields: 18+ ForceField types including Gravity, Vortex, Repulsion, Electromagnetic, HeatSource, MathField, StrangeAttractor, EntropyField, Damping, Flow, Pulsing, Shockwave, Wind, Warp, Tidal, MagneticDipole, Saddle -- all with fade-in/out, TTL, and tags
- FieldComposer with Add, Multiply, Max, Lerp blend operators; FieldSampler with streamline tracing and RGBA debug output; AnimatedField; FieldPresets factory
- Noise: Perlin 1D/2D/3D, Simplex 2D/3D, Cellular/Worley, Value, Ridged, Billowy, Turbulence, domain-warped variants, octave-stacked FBM
- Spring physics: SpringDamper, Spring3D, ConstrainedSpring, DistanceConstraint, PinConstraint, SpringChain, VerletPoint, VerletCloth (2D grid), SpringNetwork, CoupledOscillators

### Physics

- Eulerian grid fluid simulation (Navier-Stokes on a 2D grid): advection, pressure solve, density diffusion, boundary conditions
- Mass-spring soft body simulation: point masses, spring networks, structural/shear/bend springs, tear thresholds, wind forces, pinning
- Rigid body 2D: AABB collision, separating axis test, impulse resolution, friction, restitution
- Spatial acceleration: 3D SpatialGrid (O(1) average radius queries), BVH tree, KdTree, frustum culling, batch_radius_query, find_close_pairs

### Entities and AI

- AmorphousEntity: glyph clusters held together by binding force fields -- cohesion degrades with HP, entities visually fall apart as they take damage
- 10+ formation shapes: ring, grid, diamond, cross, arrow, helix, fibonacci spiral, star, scatter, and custom
- Spring-based cohesion dynamics: glyphs spring toward formation targets with per-entity tension and damping
- Finite State Machine: typed states with on_enter/on_exit, history stack, automatic transition dispatch
- Behavior Tree: Sequence, Selector, Parallel, RandomSelector, Inverter, Succeeder, Repeater, Cooldown, plus leaf nodes (Wait, CheckFlag, CheckFloat, SetFlag)
- Utility AI: scored actions with Consideration curves (Linear, Quadratic, Logistic, Exponential), inertia to prevent thrashing, cooldown tracking
- Shared Blackboard for all AI models: float/bool/Vec3/string keys

### Combat

- Damage resolution: base damage, crit multipliers, armor mitigation, resistances, damage type system
- 14 status effects: Burning, Frozen, Stunned, Poisoned, Bleeding, Cursed, Blessed, Hasted, Slowed, Confused, Feared, Charmed, Silenced, Invincible -- each with stack/tick/duration mechanics
- Hit detection, threat tables, DPS tracker with rolling window, combat log
- ResistanceProfile and CombatFormulas for extensible damage math

### Particles

- Pre-allocated particle pool with 13+ mathematical behaviors driven by MathFunctions
- Full particle system: emitter shapes (sphere, cone, box, disk, ring, torus, mesh surface), particle forces (turbulence, vortex, bounce, orbit, wind blast, point attractor), trail ribbons
- 30+ EmitterPreset variants: explosion, death, crit burst, loot sparkle, fire burst, smoke puff, electric discharge, blood splatter, ice shatter, poison cloud, teleport flash, shield hit, coin scatter, rain shower, snow fall, confetti, and more
- Particle templates: fire, smoke, electric spark, plasma, rain, snow — each fully parameterized with color gradients, size curves, drag, and physics flags
- ContinuousEmitter with burst events, duration limits, LOD particle system (4-tier quality reduction by camera distance)
- GPU-ready instance buffer export: flat `[position, size, color, velocity, age_frac]` per particle
- FloatCurve and ColorGradient keyframe systems for per-particle property animation over lifetime
- Flocking simulation: Craig Reynolds Boids rules extended with leader following, predator flee, obstacle avoidance, altitude bands, turbulence, subgroup formation
- ParticleLibrary: named effect registry with campfire, explosion, rain shower presets

### Audio

- Math-function-driven audio sources: MathFunction output maps to frequency and amplitude in real time -- the same function driving a visual also generates its sound
- Spatial 3D mixer: named buses, send levels, distance attenuation, stereo panning, proximity ducking, reverb send
- DSP synthesizer: oscillators (sine, triangle, square, sawtooth, noise), biquad filters (LP/HP/BP/notch), LFOs, FM synthesis, ADSR envelopes, effects chain
- Procedural music engine: 15 scale types including Major, Dorian, Phrygian, Blues, Pentatonic, WholeTone, Diminished; chord types (triad, seventh, sus2, sus4, power, add9); progressions (I-V-vi-IV, minor pop, ii-V-I jazz); rhythm patterns (four-on-floor, eighth notes, syncopated, offbeat, clave, waltz); melody generator with step bias and chord weighting
- 7 named vibes: Silence, Ambient, Combat, Boss, Victory, Exploration, Tension -- each with full instrument and scale configuration

### Cinematics and UI

- Timeline system: time-sorted CuePoints with 30+ action types covering camera moves, fades, spawns, dialogue, audio, flags, parallel groups, and callbacks
- CutsceneScript DSL: fluent builder with implicit cursor time -- write cutscenes like prose
- Dialogue system: branching DialogueTree with typewriter reveal, emotion tints (8 emotions), choice gates behind flags, consequence flag setting, auto-advance nodes
- Tween system: Tween<T> with 40+ easing curves (all Penner equations plus Spring, Sigmoid, Hermite, Parabola, Flash, Step), TweenSequence, TweenTimeline, KeyframeTrack, CameraPath, MultiTrack, AnimationGroup
- UI widgets: UiLabel with animated color/emission via MathFunction, UiProgressBar with sub-character precision using block fill chars, UiButton with hover/press states and pulse animation, UiPanel with box-drawing chars, UiPulseRing
- Anchor-based layout and auto-wrapping grid layout
- Text renderer: rich text markup ([color:r,g,b], [emit:v], [bold], [wave]), word wrap, TextBlock, TypewriterBlock, ScrollingText log, Marquee ticker

### Scene and World

- Scene graph with typed nodes, parent-child transforms, dirty flags, world-space baking
- Full transform hierarchy: SceneNode, Transform3D with lerp/inverse/transform_point; node attach/detach, find-by-name, find-by-tag, glyph attachment
- BVH spatial index: AABB, median-split construction, sphere query, ray query, AABB overlap query
- Ambient zones: axis-aligned regions with ambient color, fog density, reverb, wind, gravity scale, smooth edge blending
- Portal system: linked portal pairs for seamless space transitions
- Layer system: 8 named render/logic layers with per-layer visibility and shadow control
- Scene events: SceneEventQueue with EntitySpawned/Despawned, GlyphSpawned/Despawned, FieldAdded/Removed, custom events
- Scene snapshot/diff: serializable scene state with glyph/entity/field position captures and delta comparison
- Bulk spawn helpers: spawn_glyph_grid, spawn_glyph_ring, despawn_glyphs
- FieldManager: permanent and TTL fields, fade-in/out curves, tag-based bulk operations, field interference and resonance queries
- Spawn system: WaveManager driving SpawnWave sequences; SpawnGroup with rate control and delay; 7 SpawnZone types; 7 SpawnPattern types; BlueprintLibrary with default enemy set
- Effects coordinator: single EffectsController dispatching all postfx from named EffectEvents -- explosion, boss entrance, death, chaos rift, time slow, lightning, screen clear

### Scripting Engine

- Lua-like scripting language: lexer → parser → AST → compiler → stack VM, fully implemented in Rust
- Stack-based bytecode VM with closures, upvalues, metatables, first-class functions, and varargs
- Complete standard library: `math.*`, `string.*`, `table.*`, `io.*`, `os.*`, `pcall`, `xpcall`, `pairs`, `ipairs`
- `ScriptHost` API: register Rust closures as global functions, bind typed modules, exec/call/call_method
- `EventBus` for event-driven script callbacks; `ScriptComponent` for per-entity script instances
- Sandboxed mode (no stdlib), output capture, hot-reload via `exec_named`

### Replay and Networking

- Deterministic replay system: `ReplayRecorder` stores compressed input frames, `ReplayPlayer` re-simulates identically
- Replay segments, rewind, fast-forward, export/import to bytes
- WebSocket client with reconnect, ping/keep-alive, message queue, typed event dispatch
- Leaderboard client: score submission, ranked fetching, local cache, offline queue
- Analytics pipeline: event batching, session tracking, funnel analysis, retention metrics, export to JSON

### Animation

- Full animation state machine: `AnimationStateMachine` with states, typed transitions, blend trees
- Additive animation, pose blending, animation layers with masking
- Inverse kinematics: FABRIK solver, CCD solver, two-bone analytical IK, foot placement
- Inverse kinematics constraints: joint limits, pole vectors, reach limits
- Morph target blending, skeletal hierarchy, animation compression

### GPU Compute

- Compute pipeline abstraction for GPU-side math: particle simulation, physics solve, image processing
- Typed `ComputeBuffer<T>`, barrier management, pipeline specialization constants
- Built-in kernels: particle integrate, fluid diffuse, histogram equalize, prefix sum, sort

### Shader Graph

- 40+ node types: math, color, UV, noise, fractal, blend, filter, texture, output
- Visual-to-GLSL compiler: topological sort, dead-node elimination, constant folding, CSE sharing
- Runtime GLSL generation, uniform binding, shader variant caching

### Render Graph

- Deferred rendering pipeline: G-buffer pass (albedo, normal, roughness, metallic, depth), lighting pass, postfx
- Pass dependency graph with automatic resource lifetime and barrier insertion
- Temporal anti-aliasing integration, shadow map passes, ambient occlusion

### PBR and Lighting

- Cook-Torrance BRDF: GGX NDF (D), Smith geometry function (G), Schlick Fresnel (F), full shading loop
- Area lights: RectLight and DiskLight with irradiance estimation
- Animated lights: pulse, flicker, strobe, fade, math-driven, color cycle, heartbeat
- IES light profiles: bilinear-sampled photometric intensity distribution, downlight and uniform presets
- Cascaded shadow maps: per-cascade view-projection matrices fit to camera frustum slices
- Image-based lighting: spherical harmonics irradiance (SH9), BRDF integration LUT via Hammersley sampling, `eval_diffuse`, `shade_ibl`
- Tone mapping: Linear, Reinhard, ACES, Uncharted2, Hejl, Custom
- Light baking: hemisphere sampling, `bake_plane` with light map bilinear sampling and box blur
- PbrMaterial: albedo, metallic, roughness, IOR, anisotropy, clearcoat, subsurface scattering
- Light presets: torch, fluorescent, candle, LED strip, interior, moonlight, neon, cavern

### Terrain

- Heightmap generation: FBM noise, ridge noise, domain warping, caldera, canyon, island presets
- Hydraulic erosion: droplet simulation with sediment carry, erosion, and deposition
- Thermal erosion: talus angle and slide rate
- Biome classification: 12 biome types by altitude, moisture, and temperature
- Climate simulation: Perlin-based moisture/temperature maps, seasonal factors, vegetation density
- Vegetation placement: tree skeletons (segments, branching), grass clusters, rock scatter, LOD impostor billboards
- Terrain LOD: step-based quad mesh chain at multiple resolutions
- Marching cubes: isosurface extraction from scalar volume with full 256-entry edge/triangle tables
- Chunk streaming: async-style load queue, LRU cache, visibility set, prefetch scheduler
- Terrain deformer: paint, flatten, smooth brush, stamp, carve river
- Height-field collider: bilinear height sampling, normal query, ray cast with binary-search refinement, AABB intersection
- Terrain material: multi-layer albedo blending by altitude and slope (grass, rock, snow, sand)

### Character and RPG Systems

- Character stats: 40+ stat kinds with (base + flat) × (1 + pct) × mult formula, ModifierRegistry, derived stats (MaxHP, crit, evasion), ResourcePool with regen
- Inventory: item slots, weight, stacking, equip/unequip, item rarity/affixes, 60+ affix pool, LootTable, CraftingSystem, TradeSystem with reputation pricing
- Skill trees: SkillNode with prerequisites, SkillBook, 12-slot AbilityBar, CooldownTracker, ComboSystem, preset Warrior/Mage/Rogue/Healer trees
- Quest system: objectives, stages, rewards, journal, chain quests, procedural kill/collect/escort/explore generation
- Achievement system: 32 built-in achievements, progression trees, daily/weekly challenge generation, mastery levels
- Save system: versioned WorldSnapshot with EntitySnapshot, SnapshotDiff; SaveManager with slot management; checkpoint/respawn system

### DSP and Signal Processing

- FFT: Cooley-Tukey radix-2 from scratch; RealFft (half-spectrum), FftPlanner with twiddle-factor cache
- STFT, CQT, autocorrelation (direct + FFT-accelerated), YIN pitch detection (parabolic interpolation)
- MFCC: FFT → Mel filterbank → log → DCT-II; Chroma 12-bin pitch class profile; key estimation (Krumhansl-Schmuckler)
- 20+ window functions: Hann, Hamming, Blackman, Kaiser(β), FlatTop, Nuttall, Gaussian, Bartlett, Welch
- Digital filters: RBJ biquad cookbook (6 types), Butterworth, Chebyshev type I, Bessel up to order 8; FIR with windowed-sinc + Kaiser equiripple; SVF, comb, allpass, Kalman, PLL
- Convolution (direct + FFT overlap-add); LUFS metering (BS.1770-4 K-weighting), transient detection, harmonic analyzer, DTW similarity

### Audio Synthesis and Effects

- Synthesis engine: multi-voice oscillators with unison (8 voices), band-limited BLIT, wavetable; ADSR+hold; state-variable TPT filter with keytrack and env mod; ModMatrix (30+ source/dest routes)
- Polyphony: 32-voice pool with voice stealing (oldest/quietest/same-note), mono+portamento, Arpeggiator, 32-step sequencer with swing, 16-pad drum machine
- Effects chain: Compressor (RMS/peak, lookahead, sidechain), Limiter (true-peak 4x oversampled), Gate, Parametric EQ (16 bands), Freeverb reverb, Delay (ping-pong, tap-tempo), Chorus, Flanger, Phaser, Distortion (tanh/hard-clip/foldback/bit-crush/tube), Granular pitch shifter, AutoTune
- Mixer: 64 channels + 8 aux buses + master; automation lanes; VU meters (peak/RMS/LUFS); crossfader with equal-power curve

### Physics Constraints and Simulation

- Constraint solver: ContactConstraint (normal + friction impulses, Baumgarte, warm-starting), DistanceConstraint (rigid rod + soft spring), HingeConstraint (limits, motor), BallSocketConstraint (cone limit), SliderConstraint (prismatic + motor), WeldConstraint (6-DOF), PulleyConstraint, GearConstraint; XPBD substep mode; union-find island detection
- 3D rigid bodies: position/velocity/orientation (Quat), inv_inertia tensor (Mat3), force/torque accumulator; CollisionShape (Sphere/Box/Capsule/ConvexHull); 15-axis SAT box-box, sphere-box GJK fallback, capsule-capsule; sweep-and-prune broadphase; sleep system with island wake propagation
- SPH fluids: WCSPH cubic spline kernel, Tait equation, artificial viscosity, Akinci surface tension, CFL adaptive timestep; shallow-water height-field with Gaussian ripples; buoyancy force with Archimedes upthrust and viscous drag

### ECS (Entity Component System)

- Archetype-based storage: components packed in dense byte arrays per archetype; AnyVec with drop/clone function pointers
- World: entity creation/deletion, component add/remove with archetype migration; generational EntityAllocator
- Query system: Read<T>/Write<T>/OptionRead<T> marker types; QueryIter walking archetypes; With/Without filters; change detection (added_ticks/changed_ticks)
- Commands: deferred spawn/despawn/insert/remove/resource mutations applied via `apply(&mut World)`
- Events: double-buffered EventWriter/EventReader/EventCursor; Schedule with RunConditions and system labels

### Editor Tools

- Scene hierarchy panel: tree view with drag-drop reparent (cycle guard), multi-select, duplicate subtree, visibility/lock toggles, prefab nodes, text serialization round-trip
- Inspector panel: 14 field types (Bool/Int/Float/String/Vec2/Vec3/Vec4/Color with HSV/Enum/Slider/AssetRef/Script/List/Map), per-group collapsing, search bar, TransformInspector with snap
- Gizmos: TranslateHandle (3 axes + 3 planes), RotateHandle (arc circles), ScaleHandle (endpoint cubes); BoundingBox (12 edges), LightGizmo, ColliderGizmo (box/sphere/capsule/cylinder), CameraFrustumGizmo, PathGizmo (Catmull-Rom spline), VectorFieldGizmo, MeasurementTool
- Console: 10K-line ring buffer, 16 built-in commands, history, Tab-autocomplete, expression evaluator (`eval sin(pi*0.5)`), ConsoleSink

### Multiplayer Networking

- Protocol: 22-byte binary header (magic, version, flags, kind, sequence, ack, ack_bits), varint LEB128 encoding, position delta (i16 fixed-point), boolean bit-packing, 64-bit replay-detection sliding window, bandwidth tracker
- Transport: ReliableUdp with AIMD congestion control, exponential-backoff retransmit (100ms→8s), RTT/jitter EWMA; Fragmenter (MTU=1400), ReorderBuffer; ConnectionManager with keepalive and timeout
- Sync: delta snapshots with per-field change detection; StateInterpolator (lerp + velocity dead-reckoning); ClientPrediction with rollback+replay reconciliation; LagCompensation with 1s history; NTP-style NetworkClock
- Lobby: LobbyManager with matchmaking (Elo expanding radius ±50→±500), TeamSystem auto-balance, VoiceChatManager, ReadyCheck; LobbyBrowser with filter/sort
- RPC: 12 built-in RPCs (chat, damage numbers, camera shake, force field, particle burst, dialogue, etc.), rate limiting per client, RpcBatcher, RpcReplay record/export

### Scripting

- Lua-like scripting language: lexer → parser → AST → compiler → stack VM with closures, upvalues, metatables
- Full standard library: `math.*` (40+ functions), `string.*` with Lua-style pattern matching, `table.*`, `io.*`, `os.*`, `bit.*`, `pcall/xpcall`, `pairs/ipairs/next/select`
- Module system: ModuleRegistry with circular-dependency detection (DFS gray/black coloring), `require`, hot-reload watcher, Namespace with dotted-path lookup
- Debugger: breakpoints (line/function/conditional/exception), step-in/step-over/step-out, LocalInspector, WatchExpression, DebugSession REPL, CoverageTracker (bit-packed per chunk)

### Procedural Generation

- Dungeon: BSP splitter, room placer with L-shaped corridors, cellular cave generation, Wave Function Collapse tile grid with backtracking, maze generators (backtracker/Eller/Prim/Kruskal), DungeonDecorator with enemy/treasure/trap placement
- World generation: Whittaker biome classification (12 biomes), FBM heightmap with continent mask, climate simulation (latitude + elevation lapse rate + moisture advection), river system (gradient descent + erosion), road network (A* on terrain cost), settlement placement, Markov chain name generation (5 cultures), world history events
- Procedural items: 60+ affix pool, ItemGenerator with prefix/suffix rolls, 20 unique items, 5 item sets with progressive bonuses, gems/sockets, LootDropper by enemy type and depth

### Game Systems

- Game state machine: DifficultyPreset, GameConfig, pause-aware GameTimer, SessionStats (18 fields), ComboTracker with decay window, HighScoreTable
- Menu system: 10 fully implemented screens (main/pause/settings/character-select/level-select/load/game-over/victory/credits/loading), key-rebind capture, animated backgrounds
- Localization: 10 locales with per-language plural rules (EN/FR/RU/AR/CJK), 90+ built-in keys, NumberFormatter, DateTimeFormatter (relative "3 minutes ago"), ANSI colored text, markup parser (`[b]`, `[color:hex]`, `[wave]`, `[rainbow]`)

### Advanced Mathematics

- Numerical methods: 7 root-finding algorithms (bisect/Newton/secant/Brent/Illinois/Muller/fixed-point), 8 quadrature methods (adaptive Simpson, Gauss-Legendre, Romberg, Monte Carlo N-D), ODE solvers (Euler/RK4/RK45 Dormand-Prince/Adams-Bashforth/Verlet)
- Linear algebra: full Matrix struct with LU decomposition (partial pivoting), Cholesky, QR, power iteration, 2×2 SVD; Gram-Schmidt; cubic spline interpolation; RBF interpolation
- Computational geometry: 20+ primitive types, Möller-Trumbore ray-triangle, 15-axis OBB SAT, GJK+EPA, Bowyer-Watson Delaunay, Sutherland-Hodgman clipping, Minkowski sum
- Statistics: 12 distributions (PDF/CDF/inverse CDF/sampling), 6 hypothesis tests, polynomial/ridge/logistic regression, Beta-Bernoulli conjugate Bayesian inference; DTW, KL-divergence, mutual information
- Simulation: all 256 Wolfram 1D cellular automaton rules, Conway/WireWorld/ForestFire/BriansBrain/LangtonsAnt; Barnes-Hut O(n log n) N-body; Gray-Scott reaction-diffusion (6 presets); SIR/SEIRD epidemiology; Nagel-Schreckenberg traffic CA; IDM

### Config and Debug

- Hot-reloadable TOML config with command-line overrides; physics/input/debug/gameplay/accessibility config sections
- Engine profiles: low-end, steam-deck, ultra, debug -- one-call apply with per-profile overrides
- Debug overlay: FPS, entity counts, field count, camera info, scene time
- Frame profiler: rolling-window CPU timing per named span, ScopedSpan RAII guard, avg/max/min/last, formatted report
- Math graph: in-world oscilloscope rendering any MathFunction as a block-char bar graph
- Procedural generation: BSP dungeon floors, weighted spawn tables tiered by depth, loot tables with rarity tiers, phonetic name generation

---

## Architecture

```
proof-engine/src/
  audio/
    math_source.rs      MathAudioSource -- math function to frequency/amplitude
    mixer.rs            spatial 3D mixer, buses, ducking, reverb
    music_engine.rs     procedural music: scales, chords, progressions, melody
    output.rs           cpal real-time synthesis thread
    synth.rs            oscillators, filters, LFOs, FM, ADSR, effects chain
  combat/
    mod.rs              damage, 14 status effects, DPS tracker, threat table
  config/
    mod.rs              TOML config, profiles, hot reload, CLI overrides
  debug/
    graph.rs            in-world MathFunction oscilloscope
    mod.rs              FPS overlay, counts, camera HUD
    profiler.rs         rolling-window CPU frame profiler
  effects/
    mod.rs              EffectsController, named EffectEvents -> postfx dispatch
  entity/
    ai.rs               FSM, BehaviorTree, UtilityAI, Blackboard
    cohesion.rs         spring-based glyph formation dynamics
    formation.rs        10+ formation shapes as math functions
    mod.rs              AmorphousEntity: force-bound glyph clusters
  glyph/
    atlas.rs            ab_glyph TTF rasterization to R8 GPU texture
    batch.rs            layer/blend-sorted instanced draw call batching
    mod.rs              Glyph struct, GlyphPool, RenderLayer, BlendMode
  input/
    keybindings.rs      action system, chord detection, analog axes
    mod.rs              keyboard, mouse, scroll, frame-delta tracking
  math/
    attractors.rs       7 strange attractors with RK4, Lyapunov exponents
    color.rs            color spaces, tone mapping, LUT, gradients, palettes
    complex.rs          Complex, Quaternion, Mandelbrot/Julia/BurningShip/Newton/Lyapunov
    curves.rs           Bezier/BSpline/CatmullRom/Hermite, arc-length, Frenet
    eval.rs             30+ MathFunction variants, derivative, integrate, compose
    fields.rs           18+ ForceField types, FieldComposer, FieldSampler, AnimatedField
    mod.rs              remap, smoothstep, HSV, utility functions
    noise.rs            Perlin, Simplex, Cellular, FBM, turbulence
    springs.rs          SpringDamper, Verlet cloth, spring networks
  particle/
    emitters.rs         explosion, death, trail, vortex, beam presets
    flock.rs            full Boids flocking + leader/predator/obstacle
    mod.rs              MathParticle, ParticlePool, 13+ behaviors
  physics/
    fluid.rs            Navier-Stokes Eulerian grid fluid
    mod.rs              rigid body 2D, AABB, SAT, impulse resolution
    soft_body.rs        mass-spring deformable body simulation
  procedural/
    dungeon.rs          BSP dungeon floor generation
    loot.rs             rarity-tiered loot tables
    mod.rs              world generation orchestration
    names.rs            phonetic name generation
    spawn.rs            depth-scaled weighted spawn tables
  render/
    camera.rs           spring-follow, orbit, cinematic modes, trauma shake
    pipeline.rs         glutin/winit window, OpenGL 3.3, instanced glyph render
    postfx/
      bloom.rs          multi-level Gaussian bloom
      chromatic.rs      RGB channel offset
      color_grade.rs    lift/gamma/gain, S-curves, 3D LUT, 9 film looks, animated keyframes
      distortion.rs     gravitational lensing, heat shimmer, warp
      grain.rs          film grain white noise overlay
      motion_blur.rs    velocity-based multi-sample blur
      pipeline.rs       full postfx pass compositor
      scanlines.rs      CRT phosphor scanline simulation
    shader_graph/
      mod.rs            ShaderGraph: node/edge ownership, validation, compile entry
      nodes.rs          40+ NodeType variants with GLSL snippets and socket schemas
      compiler.rs       GraphCompiler: topological sort, GLSL codegen, uniform binding
      optimizer.rs      dead-node elimination, constant folding, CSE sharing
    shaders/            GLSL sources embedded at compile time
    text_renderer.rs    rich text layout, typewriter, scroll log, marquee
  scene/
    field_manager.rs    FieldManager: TTL, fade, tags, interference queries
    mod.rs              SceneGraph: glyphs, entities, particles, fields
    node.rs             typed scene nodes, parent-child transforms
    spawn_system.rs     WaveManager, SpawnZone, SpawnPattern, BlueprintLibrary
  spatial/
    mod.rs              SpatialGrid, BVH, KdTree, frustum cull, pair queries
  timeline/
    dialogue.rs         branching dialogue tree, typewriter, choices, emotions
    mod.rs              Timeline, CuePoint, TimelinePlayer, CutsceneLibrary
    script.rs           CutsceneScript DSL, DialogueSequence builder
  tween/
    easing.rs           40+ easing functions (Penner, Spring, Sigmoid, Hermite)
    keyframe.rs         KeyframeTrack, Keyframe, CameraPath, MultiTrack
    mod.rs              Tween<T>, TweenState, AnimationGroup, Tweens presets
    sequence.rs         TweenSequence, TweenTimeline, predefined animations
  ui/
    layout.rs           UiRect, Anchor, UiLayout, AutoLayout grid
    mod.rs              UiRoot, UiColors palette
    widgets.rs          Label, ProgressBar, Button, Panel, PulseRing
  scripting/
    lexer.rs            tokenizer: all tokens, string escapes, spans
    ast.rs              full AST: Expr, Stmt, BinOp, UnOp, TableField, Script
    parser.rs           recursive-descent Pratt parser
    compiler.rs         single-pass AST → bytecode compiler (Chunk/Instruction)
    vm.rs               stack-based bytecode VM with closures and metatables
    stdlib.rs           complete standard library: math, string, table, io, os
    host.rs             ScriptHost, EventBus, ScriptComponent, ScriptObject
  anim/
    mod.rs              AnimationStateMachine, blend trees, IK solvers, morph targets
  animation/
    mod.rs              skeletal animation, pose blending, layers, compression
  replay/
    mod.rs              ReplayRecorder, ReplayPlayer, segment rewind, export
  networking/
    mod.rs              WebSocket client, reconnect, event dispatch
    leaderboard.rs      score submission, ranked fetch, offline cache
    analytics.rs        event batching, sessions, funnels, retention metrics
    websocket.rs        WebSocket protocol, ping, message queue
  network/
    mod.rs              core network abstractions
  ai/
    mod.rs              high-level AI module coordinator
    pathfinding.rs      A*, Dijkstra, JPS, hierarchical pathfinding, NavMesh queries
    navmesh.rs          NavMesh build from geometry, portal graph, string-pulling
    steering.rs         steering behaviors: seek, flee, pursue, evade, wander, formation
    flowfield.rs        Dijkstra-based flow field generation and agent integration
    utility.rs          UtilityAI: scored actions, consideration curves, inertia
  render/
    compute/mod.rs      GPU compute pipeline, typed buffers, built-in kernels
    render_graph.rs     deferred G-buffer, pass graph, barrier management
    lighting.rs         PBR lighting, area lights, IES, animated lights, CSM, IBL, tone map
    pbr/
      brdf.rs           Cook-Torrance BRDF, GGX, Smith, Schlick, IBL, tone mapping
      atmosphere.rs     atmospheric scattering (Rayleigh + Mie)
      probe.rs          environment probes, SH irradiance, reflection parallax, light probe grids
  terrain/
    heightmap.rs        FBM/ridge/warped noise, hydraulic erosion, thermal erosion
    biome.rs            12 biome types, climate simulation, seasonal factors
    vegetation.rs       tree skeletons, grass clusters, rock scatter, impostor billboards
    streaming.rs        chunk load queue, LRU cache, visibility set, prefetch scheduler
    mod_types.rs        ChunkCoord, ChunkState, TerrainConfig, TerrainChunk
    mod.rs              TerrainManager, TerrainCollider, material layers, marching cubes
  ecs/
    mod.rs              archetype-based ECS world
    entity.rs           entity IDs and generation counters
    storage.rs          component column storage
    query.rs            typed query iteration with filters
    schedule.rs         system topological ordering and parallel groups
  dsp/
    mod.rs              FFT, STFT, spectral analysis, pitch/tempo detection
  render/pbr/
    brdf.rs             full BRDF library (D, G, F terms), IBL, tone mapping
  character/
    stats.rs            base/modified stats, modifiers, leveling
    inventory.rs        items, slots, weight, rarity, affixes
    skills.rs           skill trees, prerequisites, passive/active skills
    quests.rs           objectives, stages, rewards
  game/
    mod.rs              GameManager, state machine, score, session stats, timers
    achievements.rs     unlock trees, progression nodes, reward dispatch
    localization.rs     string table, locale switching, format templates
    menu.rs             menu screen stack, transitions, settings UI
  editor/
    gizmos.rs           3D transform gizmos with screen-space hit testing
    inspector.rs        per-component property editor
    hierarchy.rs        scene node tree view
    console.rs          command input, history, autocomplete
  save/
    serializer.rs       SerializedValue, Serialize/Deserialize traits
    snapshot.rs         WorldSnapshot, EntitySnapshot, SnapshotDiff
    format.rs           SaveFile, SaveHeader, SaveManager, slot management
    checkpoint.rs       Checkpoint, CheckpointManager, RespawnSystem
  math/
    simulation.rs       N-body simulation, octree, Barnes-Hut, fluid grids
    geometry.rs         convex hull, polygon clipping, CSG, distance queries
    statistics.rs       descriptive stats, distributions, hypothesis tests
    numerical.rs        Newton-Raphson, bisection, RK4, matrix decompositions
  scene/
    bvh.rs              BVH construction, sphere/ray/AABB spatial queries
    query.rs            RaycastHit, FrustumQuery, frustum plane extraction
    events.rs           SceneEventQueue, EventKind
    field_manager.rs    FieldManager: TTL, fade, tags, interference queries
    mod.rs              SceneGraph: hierarchy, portals, zones, layers, events
    node.rs             typed scene nodes, parent-child transforms
    spawn_system.rs     WaveManager, SpawnZone, SpawnPattern, BlueprintLibrary
  particle/
    mod.rs              full particle system: shapes, forces, trails, LOD, GPU export
    emitters.rs         30+ preset emitters: explosion, fire, smoke, electric, rain, snow
    flock.rs            full Boids flocking + leader/predator/obstacle
  physics/
    rigid_body.rs       rigid body 2D/3D, AABB, SAT, impulse resolution, friction
    constraints.rs      distance/hinge/prismatic/spring constraints
    joints.rs           joint solver, ragdoll, sequential impulses
    fluid.rs            Navier-Stokes Eulerian grid fluid
    soft_body.rs        mass-spring deformable body simulation
  integration.rs        ProofGame trait -- game-to-engine contract
  lib.rs                ProofEngine, prelude, public API
```

The pipeline:
```
Game State -> Scene Graph -> Mathematical Transform -> Glyph Renderer -> Post-Processing -> Display
```

---

## Quick Start

```rust
use proof_engine::prelude::*;

fn main() {
    let mut engine = ProofEngine::new(EngineConfig::default());

    engine.spawn_glyph(Glyph {
        character: 'A',
        position: Vec3::ZERO,
        color: Vec4::new(1.0, 0.4, 0.8, 1.0),
        emission: 1.2,
        life_function: Some(MathFunction::Breathing { rate: 0.5, depth: 0.1 }),
        ..Default::default()
    });

    engine.add_field(ForceField::Vortex {
        center: Vec3::ZERO,
        strength: 2.0,
        radius: 5.0,
        axis: Vec3::Y,
    });

    engine.run(|_engine, _dt| {
        // game logic
    });
}
```

## Examples

```bash
cargo run --example hello_glyph          # single glyph with breathing animation
cargo run --example chaos_field          # 2000+ glyph mathematical background
cargo run --example particle_demo        # all particle behaviors and interactions
cargo run --example force_fields         # interactive force field playground
cargo run --example amorphous_entity     # entity formation, damage, dissolution
cargo run --example strange_attractors   # Lorenz, Rossler, Chen, Halvorsen with bloom
cargo run --example full_combat          # combat: shake, particles, beams, status effects
```

## Integration (ProofGame trait)

```rust
pub trait ProofGame {
    fn init(&mut self, engine: &mut ProofEngine) -> Scene;
    fn update(&mut self, engine: &mut ProofEngine, input: &InputState, dt: f32);
    fn render(&self, engine: &mut ProofEngine, frame: &mut Frame);
    fn resize(&mut self, width: u32, height: u32);
}
```

## Used By

- [CHAOS RPG](https://github.com/Mattbusel/chaos-rpg) -- A roguelike where every outcome runs through a chain of real mathematical functions. The engine was built for this game.

## Build

```bash
cargo build
cargo test
cargo check
```

Requires Rust stable. OpenGL 3.3 Core context. Tested on Windows 11 with MSVC toolchain.

## Stats

- Compiles clean with no errors
- 15+ major system tiers: rendering, math, physics, audio, AI, scripting, networking, animation, combat, terrain, ECS, DSP, PBR, editor, character/game systems

## License

MIT -- use it, fork it, build on it. Credit appreciated but not required.
