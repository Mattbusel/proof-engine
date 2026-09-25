# Proof Engine

[![crates.io](https://img.shields.io/crates/v/proof-engine.svg)](https://crates.io/crates/proof-engine)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**A Rust rendering and game engine where every visual is the output of a mathematical function.** Glyphs and particles are moved by real differential equations, force fields and spring systems, not by sprites, meshes or keyframed animation.

A Lorenz attractor on screen looks like a Lorenz attractor because its particles are integrating the Lorenz equations. An entity is a cluster of glyphs held together by force cohesion; when it loses HP the binding weakens and it comes apart into an attractor instead of playing a death animation. If you like generative art, simulation or procedural games and want an engine built around that idea from the start, this is it.

![Two particle-built figures in the convergence demo](Screenshot%202026-03-25%20231046.png)

![Convergence demo, animated](ezgif.com-video-to-gif-converter%20%284%29.gif)

![Supernova demo](assets/supernova-demo.gif)

## What it does

- **OpenGL 3.3 renderer** (glutin, winit, glow) with instanced glyph rendering, bloom, chromatic aberration, film grain, vignette, scanlines and motion blur.
- **Math functions as animation**: Lorenz, Rossler, Chen, Halvorsen, Aizawa and Thomas attractors; sine, Perlin noise, logistic map, Collatz, golden spiral, Lissajous, Mandelbrot escape, spring-damper systems. Any glyph can have a `life_function` that drives its position or color.
- **Composable force fields**: gravity, vortex, electromagnetic, strange attractor, shockwave, tidal, flow, magnetic dipole, entropy and damping, with linear, inverse-square, exponential or Gaussian falloff.
- **Particle-built entities** held together by force cohesion, with HP-linked binding strength.
- **Physics**: 2D rigid bodies with SAT collision, mass-spring soft bodies, Eulerian fluid, constraints and joints.
- **Audio**: 48 kHz synthesis (rodio, cpal), ADSR, FM, music-theory helpers (scales, chords, progressions), stereo panning and reverb.
- **Scripting**: a custom bytecode VM with lexer, parser and compiler, closures and tables.
- **Procedural generation**: tectonics, erosion, climate, biomes, rivers, caves, settlements, history, language and quest generation, plus ecology models (Lotka-Volterra, SIR).
- **Proof Editor**: an egui scene editor for placing glyphs, force fields and entities, with an inspector, hierarchy, post-FX presets, undo/redo and JSON scenes.

The source tree also contains modules for more advanced lighting (a sparse voxel octree GI cone tracer, Nishita sky scattering, tiled and deferred lighting, volumetric fog, a wgpu backend). Those exist as code but are not yet connected to the demos shown above.

## Quick start

Requires a Rust toolchain and an OpenGL 3.3 capable GPU.

```bash
git clone https://github.com/Mattbusel/proof-engine.git
cd proof-engine
cargo run --release --example hello_glyph     # smallest possible program
cargo run --release --example galaxy
cargo run --release --example supernova
cargo run --release --example convergence     # the demo in the screenshots
```

Other examples: `chaos_field`, `particle_demo`, `force_fields`, `amorphous_entity`, `strange_attractors`, `full_combat`, `math_rain`, `heartbeat`, `showcase`, `playground`, `sculptor`, `apotheosis`, `colossus`. Some of the heavier ones (`apotheosis`, `colossus`) allocate millions of GPU particles and need a strong GPU.

Benchmarks: `cargo bench` (Criterion: `particle_bench`, `glyph_bench`).

### Use as a library

```toml
[dependencies]
proof-engine = "0.1"
```

```rust
use proof_engine::prelude::*;

fn main() {
    let mut engine = ProofEngine::new(EngineConfig::default());

    engine.spawn_glyph(Glyph {
        character: '@',
        position: Vec3::ZERO,
        color: Vec4::new(0.0, 1.0, 0.8, 1.0),
        emission: 1.2,
        life_function: Some(MathFunction::Breathing { rate: 0.4, depth: 0.15 }),
        ..Default::default()
    });

    engine.add_field(ForceField::StrangeAttractor {
        attractor_type: AttractorType::Lorenz,
        scale: 0.2,
        strength: 0.4,
        center: Vec3::ZERO,
    });

    engine.run(|_engine, _dt| {});
}
```

## Proof Editor

![Proof Editor](assets/editor-screenshot.png)

Download `proof-editor.exe` (Windows) from the [releases page](https://github.com/Mattbusel/proof-engine/releases), or build it:

```bash
cd proof-engine/editor
cargo run --release
```

| Key | Action |
| --- | --- |
| Click viewport | Place with current tool |
| WASD / arrows | Pan camera |
| V / G / P / F / E / X | Select, move, place glyph, place force field, place entity, particle burst |
| Shift+Click | Multi-select |
| Ctrl+C / Ctrl+V | Copy / paste |
| Ctrl+Z / Ctrl+Y | Undo / redo |
| Ctrl+S / Ctrl+O / Ctrl+N | Save / load / new scene |
| Delete | Remove selection |
| Space | Screen shake |
| F1 | Help |

## Architecture

Roughly 660,000 lines of Rust across the engine (`src/`), the editor (`editor/`) and the examples. The largest engine modules:

| Module | Contents |
| --- | --- |
| `render` | OpenGL pipeline, post-FX, shader graph |
| `math` | attractors, fields, curves, noise, springs, `MathFunction` evaluation |
| `glyph`, `particle`, `entity` | the core primitives and their pools |
| `physics` | rigid body, soft body, fluid, constraints |
| `audio`, `dsp` | synthesis, music theory, effects, spatial audio |
| `ecs` | archetype ECS with generational IDs |
| `scripting` | lexer, parser, compiler, bytecode VM |
| `terrain`, `worldgen`, `ecology`, `narrative` | procedural generation |
| `game` | boss AI, cloth, debris, achievements |
| `svogi`, `nishita_sky`, `volumetric_fog`, `wgpu_backend` | advanced lighting, not yet wired into the demos |

### The `apotheosis` example

`examples/apotheosis.rs` is a standalone showcase of a particle-rendered character built on signed distance fields instead of meshes. A 26-bone capsule skeleton is blended with Inigo Quilez's polynomial smooth minimum, particles are importance-sampled onto the SDF shell, and normals, ambient occlusion and subsurface thickness are all computed from the SDF itself. On top of that it layers per-material shading (Schlick Fresnel, Kajiya-Kay hair and fabric specular, thin-film iridescence), a strand-based hair renderer, inertial lag for loose materials, and a post stack including TAA jitter, spectral bloom, god rays, depth of field bokeh and ACES tonemapping. It targets about 10.8 million GPU particles.

## Related

[chaos-rpg](https://github.com/Mattbusel/chaos-rpg) is a roguelike whose graphical frontend runs on Proof Engine. `CHAOS_RPG_API_CONTRACT.md` documents what the engine has to support for it.

## Status

Early (0.1.x) and moving fast. The public API is not stable, there is no CI workflow in this repository, and some subsystems are further along than others. Contributions: see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT, see [LICENSE](LICENSE).


## Hire the author

**Need this kind of engineering on your product?** I take on a small number of client builds: LLM features, iOS apps and performance work, fixed price. [Services and pricing](https://mattbusel.github.io/) · [Email](mailto:mattbusel@gmail.com) · [LinkedIn](https://www.linkedin.com/in/matthewbusel/)
