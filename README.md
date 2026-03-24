# Proof Engine

A mathematical rendering engine for Rust. 57,000+ lines of fully implemented systems across 108 source files.

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
- Flocking simulation: Craig Reynolds Boids rules extended with leader following, predator flee, obstacle avoidance, altitude bands, turbulence, subgroup formation
- Emitter presets: explosion, death dissolve, trail, orbit ring, vortex pull, confetti, sparkle, rain, smoke, beam, attractor swarm

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
- FieldManager: permanent and TTL fields, fade-in/out curves, tag-based bulk operations, field interference and resonance queries
- Spawn system: WaveManager driving SpawnWave sequences; SpawnGroup with rate control and delay; 7 SpawnZone types (Point, Box, Sphere, SphereSurface, Disc, Line, Ring, AroundPlayer); 7 SpawnPattern types (Random, Ring, Grid, V-Formation, Line, Burst, Escort); BlueprintLibrary with default enemy set
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
    utility.rs          UtilityAI: scored actions, consideration curves, inertia
  render/
    compute/mod.rs      GPU compute pipeline, typed buffers, built-in kernels
    render_graph.rs     deferred G-buffer, pass graph, barrier management
    lighting.rs         PBR lighting, shadow maps, ambient occlusion, IBL
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

- 57,000+ lines of Rust across 108 source files
- Zero stubs — every function is fully implemented
- Compiles clean with no errors
- 10 major system tiers: rendering, math, physics, audio, AI, scripting, networking, animation, combat, tooling

## License

MIT -- use it, fork it, build on it. Credit appreciated but not required.
