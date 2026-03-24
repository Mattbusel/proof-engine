# Proof Engine

A mathematical rendering engine for Rust.

Every visual is the output of a mathematical function. Every animation is a continuous function over time. Every particle follows a real equation. Characters are rendered as textured quads in 3D space with bloom, distortion, motion blur, and force field physics.

## Philosophy

**Proof Engine does not render graphics. It renders mathematics.**

A traditional renderer draws shapes and colors that represent game state. Proof Engine computes mathematical functions and the visual IS the output. A Lorenz attractor doesn't look like a Lorenz attractor because someone drew one. It looks like a Lorenz attractor because 200 particles are following the actual differential equations in real time.

A damage hit doesn't look heavy because someone animated a heavy-looking effect. It looks heavy because a gravitational force field spawned at the impact point and every glyph in the vicinity was physically pulled toward it. The math is not a theme painted on top. The math is the rendering primitive.

### Weight and Life

Every object on screen has mathematical mass. Things with high values move slower, hit harder, and disturb the space around them more. A 3000-damage crit warps the surrounding space, bends nearby characters toward it, and leaves a gravitational afterimage. Stats are not numbers you read. They are forces you see acting on the screen.

The screen is never still. Every element is a living mathematical function that breathes, oscillates, and responds to its neighbors — continuous functions that create organic, fluid motion. A sine wave is alive. A Lorenz attractor is alive. The engine uses real mathematical objects as animation primitives instead of keyframes and tweens.

## Features

- **Glyph Rendering** — ASCII characters as textured quads in 3D space with position, mass, charge, temperature, entropy
- **Mathematical Animation** — Every property driven by continuous math functions: Lorenz, Mandelbrot, Collatz, Fibonacci, Perlin, springs, orbits, spirals, heartbeat, breathing
- **Force Field Physics** — Gravity, flow, vortex, electromagnetic, heat, entropy, strange attractors — fields stack and create emergent visual environments
- **Amorphous Entities** — Visual forms held together by binding forces; HP-linked cohesion degrades form as entities take damage; death dissolves via the killing function's attractor
- **Particle System** — Pre-allocated pool, 13+ mathematical behaviors, flocking, chaining, trailing; damage particles follow strange attractors determined by the killing engine
- **Post-Processing** — Bloom (Gaussian), color grading, screen-space distortion, motion blur, chromatic aberration, film grain, optional CRT scanlines
- **Procedural Audio** — Math-function-driven audio sources spatially mixed in 3D; the same function driving a visual also generates its sound
- **Spring Camera** — Spring-physics camera with momentum, cinematic dolly paths, Perlin shake, depth of field

## Quick Start

```rust
use proof_engine::prelude::*;

fn main() {
    let mut engine = ProofEngine::new(EngineConfig::default());

    let _glyph = engine.spawn_glyph(Glyph {
        character: 'A',
        position: Vec3::ZERO,
        color: Vec4::new(1.0, 1.0, 1.0, 1.0),
        emission: 0.8,
        life_function: Some(MathFunction::Breathing { rate: 0.5, depth: 0.1 }),
        ..Default::default()
    });

    engine.add_field(ForceField::Gravity {
        center: Vec3::ZERO,
        strength: 1.0,
        falloff: Falloff::InverseSquare,
    });

    engine.run(|_engine, _dt| {
        // game logic here
    });
}
```

## Examples

```bash
cargo run --example hello_glyph        # render a single glyph with breathing animation
cargo run --example chaos_field        # the living mathematical background (2000+ glyphs)
cargo run --example particle_demo      # all particle behaviors and interactions
cargo run --example force_fields       # interactive force field playground
cargo run --example amorphous_entity   # entity formation, damage, and dissolution
cargo run --example strange_attractors # Lorenz, Rossler, Chen, Halvorsen with bloom
cargo run --example full_combat        # mock combat with all effects: shake, particles, beams
```

## Architecture

```
proof-engine/
├── src/
│   ├── glyph/          Glyph struct, GlyphPool, font atlas
│   ├── math/           MathFunction, ForceField, attractors, springs, noise
│   ├── entity/         AmorphousEntity, formations, cohesion dynamics
│   ├── particle/       MathParticle, ParticlePool, emitter presets, flocking
│   ├── scene/          SceneGraph, SceneNode, force field manager
│   ├── render/         Camera, pipeline, post-processing shaders
│   ├── audio/          MathAudioSource, spatial mixer, synthesizer
│   ├── input/          input handling, configurable keybindings
│   └── config/         EngineConfig, defaults
```

The pipeline:
```
Game State → Scene Graph → Mathematical Transform → Glyph Renderer → Post-Processing → Display
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

- [CHAOS RPG](https://github.com/Mattbusel/chaos-rpg) — A roguelike where every outcome runs through a chain of real mathematical functions. The engine was built for this game.

## Build Status

Phase 1 (window + glyph rendering) — in progress

## License

MIT — use it, fork it, build on it. Credit appreciated but not required.
