<p align="center">
  <a href="https://mattbusel.github.io/proof-engine/"><img src="https://raw.githubusercontent.com/Mattbusel/proof-engine/main/assets/banner.jpg" width="100%" alt="Proof Engine. Left: a sunset over mountains from the sky example, every cell a Rayleigh and Mie scattering integral. Right: 40,000 points on the Lorenz attractor from the lorenz example."></a>
</p>

<p align="center">
  <a href="https://mattbusel.github.io/proof-engine/"><b>Site</b></a> &nbsp;&middot;&nbsp;
  <a href="https://crates.io/crates/proof-engine">crates.io</a> &nbsp;&middot;&nbsp;
  <a href="https://docs.rs/proof-engine">docs.rs</a> &nbsp;&middot;&nbsp;
  <a href="#demos">Demos</a> &nbsp;&middot;&nbsp;
  <a href="https://github.com/Mattbusel/proof-engine/actions/workflows/ci.yml"><img src="https://github.com/Mattbusel/proof-engine/actions/workflows/ci.yml/badge.svg" alt="CI" align="center"></a>
</p>

**Proof Engine is a Rust library for making moving pictures out of math: you write the equations, it draws what they do, in real time, with bloom and HDR light.**

<p align="center"><img src="https://raw.githubusercontent.com/Mattbusel/proof-engine/main/assets/gifs/galaxy.gif" width="100%" alt="The galaxy example: about 3,000 glyphs on four spiral arms, each on its own circular orbit, with a hot core and dim red outer arms, seen at an angle as the camera circles."></p>
<p align="center"><sub>The <code>galaxy</code> example, captured from the engine's own framebuffer with <code>PROOF_HIDDEN=1</code>.</sub></p>

## Install

| You want to | Do this |
| --- | --- |
| Use it in your own Rust program | `cargo add proof-engine` |
| Watch the demos first | `git clone https://github.com/Mattbusel/proof-engine && cd proof-engine && cargo run --release --example galaxy` |
| Read the API | [docs.rs/proof-engine](https://docs.rs/proof-engine) |

You need stable Rust and a GPU with OpenGL 3.3 or newer.

- **Windows:** nothing else.
- **macOS:** nothing else. macOS stops at OpenGL 4.1, so `apotheosis` (which uses 4.3 compute shaders) will not run there; every other demo does.
- **Linux:** the audio backend needs the ALSA headers: `sudo apt install libasound2-dev pkg-config` (Debian/Ubuntu) or `sudo dnf install alsa-lib-devel` (Fedora).

The first build compiles the whole engine and takes a few minutes. Always use `--release`; a debug build is far too slow for tens of thousands of particles a frame.

## Use it in 3 steps

**1. Make a project and add the engine.**

```bash
cargo new lorenz-demo && cd lorenz-demo
cargo add proof-engine
```

**2. Replace `src/main.rs` with this.** 5,000 points start on one short line and follow the Lorenz equations. (It is also in the repo as [`examples/quickstart.rs`](https://github.com/Mattbusel/proof-engine/blob/main/examples/quickstart.rs).)

```rust
use proof_engine::math::attractors::rk4_step;
use proof_engine::prelude::*;
use proof_engine::render::ui_layer::UiParticle;

fn main() {
    let mut engine = ProofEngine::new(EngineConfig::default());

    // 5,000 points in a line 2 units long, each 0.0004 from the next.
    let mut points: Vec<Vec3> = (0..5000)
        .map(|i| Vec3::new(1.0 + i as f32 * 4e-4, 1.0, 1.0))
        .collect();

    engine.run_ui(move |engine, dt| {
        // Advance every point along the Lorenz equations.
        for p in points.iter_mut() {
            *p = rk4_step(AttractorType::Lorenz, *p, dt);
        }
        // Draw them: x across, z up, centred in the window.
        let (w, h) = engine.render_size();
        let (cx, cy, s) = (w as f32 / 2.0, h as f32 / 2.0, h as f32 / 60.0);
        let color = Vec4::new(0.5, 1.2, 1.6, 1.0);
        let dots = points
            .iter()
            .map(|p| UiParticle::new(cx + p.x * s, cy - (p.z - 25.0) * s, 3.0, 3.0, '●', color))
            .collect();
        engine.ui.draw_particles(dots);
    });
}
```

**3. Run it.**

```bash
cargo run --release
```

A window opens. For about ten seconds the points travel together as one short streak. Then chaos pulls them apart, and by twenty seconds they have drawn the Lorenz butterfly on their own. Close the window to quit.

## Results

These are real frames from that program, written by the engine itself (`PROOF_HIDDEN=1 PROOF_FIXED_DT=60 PROOF_SHOT=...`, see [Capture frames](#capture-frames-from-any-program)), cropped to the centre:

<img src="https://raw.githubusercontent.com/Mattbusel/proof-engine/main/assets/quickstart-steps.jpg" width="100%" alt="Four frames of the quickstart program. At 1 second a tiny speck; at 13.5 seconds a thin arc; at 16 seconds the points have split into loops; at 18.5 seconds they fill both wings of the Lorenz butterfly.">

The same idea at a larger scale, one command each:

| `strange_attractors` | `math_rain` |
| --- | --- |
| <img src="https://raw.githubusercontent.com/Mattbusel/proof-engine/main/assets/gifs/strange_attractors.gif" width="100%" alt="Seven strange attractors, Lorenz, Rossler, Chen, Halvorsen, Aizawa, Thomas and Dadras, 1,500 points each, turning slowly."> | <img src="https://raw.githubusercontent.com/Mattbusel/proof-engine/main/assets/gifs/math_rain.gif" width="100%" alt="A hundred columns of green mathematical symbols falling at different speeds, the leading glyph of each bright white-green."> |
| Seven chaotic systems, 1,500 RK4-integrated points each. Colour is speed along the flow. | A hundred columns at speeds from `abs(sin(0.13 c))`; each column's symbols flicker by its own logistic map. |
| **`lorenz`** | **`sky`** |
| <img src="https://raw.githubusercontent.com/Mattbusel/proof-engine/main/assets/gifs/lorenz.gif" width="100%" alt="40,000 points circling the two wings of the Lorenz attractor, teal where slow and amber where fast."> | <img src="https://raw.githubusercontent.com/Mattbusel/proof-engine/main/assets/gifs/sky.gif" width="100%" alt="The sky example in late afternoon: a blue sky fading to white near the sun over brown mountains."> |
| 40,000 points on the Lorenz attractor, drawn into the HDR pass so overlaps add up to real light. | Every sky cell is a Rayleigh and Mie scattering integral, recomputed every frame as the sun moves. |

## The sky is an integral

<img src="https://raw.githubusercontent.com/Mattbusel/proof-engine/main/assets/fig-sky-day.jpg" width="100%" alt="Six frames from the sky example between 11:22 and 18:05: blue midday, a white sun low on the right, orange mountains at sunset, and dark teal dusk.">

The `sky` example divides the sky into 120 by 56 cells. For each cell, every frame, `nishita_sky::compute_sky_color` integrates Rayleigh and Mie single scattering along the view ray and along a second ray toward the sun. The blue at noon, the orange band at sunset and the dark teal after it all come out of the same integral as the sun moves. The mountains are sums of sines, lit by the sky just above the horizon. Space pauses, Up/Down change the speed of the day.

## Forty thousand points, one equation

<img src="https://raw.githubusercontent.com/Mattbusel/proof-engine/main/assets/fig-lorenz.jpg" width="100%" alt="The Lorenz attractor drawn by 40,000 points, cool teal where they move slowly and amber where they move fast, with the equations printed at the top left.">

Every point in the `lorenz` example is a state `(x, y, z)` advanced each frame by the engine's own RK4 integrator. Nobody drew the two wings; that is where the equations send the points. Colour is speed along the flow, and the points are drawn into the HDR world pass, so overlapping points add up to real light before bloom and the tonemap.

```rust
use proof_engine::math::attractors::rk4_step;

for p in points.iter_mut() {
    for _ in 0..SUBSTEPS {
        *p = rk4_step(AttractorType::Lorenz, *p, h);
    }
}
```

## What it does

| Area | What is there |
| --- | --- |
| **Math functions as animation** | Lorenz, Rossler, Chen, Halvorsen, Aizawa, Thomas and Dadras attractors; sine, Perlin noise, logistic map, Collatz, golden spiral, Lissajous, Mandelbrot escape, spring-damper systems. Any glyph can have a `life_function` that drives its position or color. |
| **Composable force fields** | Gravity, vortex, electromagnetic, strange attractor, shockwave, tidal, flow, magnetic dipole, entropy and damping, with linear, inverse-square, exponential or Gaussian falloff. |
| **Particle-built entities** | Held together by force cohesion, with HP-linked binding strength. |
| **OpenGL 3.3 HDR renderer** | glutin, winit and glow; instanced glyph rendering, half-float scene buffers, bloom, ACES, chromatic aberration, film grain, vignette, scanlines and motion blur. |
| **Physics** | 2D rigid bodies with SAT collision, mass-spring soft bodies, Eulerian fluid, constraints and joints. |
| **Audio** | 48 kHz synthesis (rodio, cpal), ADSR, FM, music-theory helpers (scales, chords, progressions), stereo panning and reverb. |
| **Scripting** | A custom bytecode VM with lexer, parser and compiler, closures and tables. |
| **Procedural generation** | Tectonics, erosion, climate, biomes, rivers, caves, settlements, history, language and quest generation, plus ecology models (Lotka-Volterra, SIR). |
| **Proof Editor** | An egui scene editor for placing glyphs, force fields and entities, with an inspector, hierarchy, post-FX presets, undo/redo and JSON scenes. |

The source tree also contains modules for more advanced lighting (a sparse voxel octree GI cone tracer, Nishita sky scattering, tiled and deferred lighting, volumetric fog, a wgpu backend). The Nishita sky model drives the `sky` example; the others exist as code but are not yet connected to any demo.

## Demos

Every demo is `cargo run --release --example <name>`. Close the window (or press Esc where noted) to quit.

<img src="https://raw.githubusercontent.com/Mattbusel/proof-engine/main/assets/fig-convergence.jpg" width="100%" alt="The convergence demo: a blue particle-built fighter on the left, and a red one on the right coming apart into loose particles and rings after a hit.">

| Example | What you see |
| --- | --- |
| `sky` | A day passing over a mountain range. Every sky cell is the Nishita Rayleigh + Mie scattering integral for its view direction, recomputed each frame. Space pauses, Up/Down change speed, Esc quits. |
| `lorenz` | 40,000 points on the Lorenz attractor, integrated with RK4 and coloured by speed. Space pauses, Left/Right turn the view, Esc quits. |
| `galaxy` | About 3,000 glyphs on four spiral arms, each on its own orbit, with a hot core, red outer arms and drifting nebula dust. The camera circles slowly. Esc quits. |
| `strange_attractors` | Lorenz, Rossler, Chen, Halvorsen, Aizawa, Thomas and Dadras side by side, 1,500 points each. Space pauses, Esc quits. |
| `math_rain` | Digital rain made of mathematical symbols; column speeds from a sine, symbol flicker from a logistic map. Esc quits. |
| `quickstart` | The program from [Use it in 3 steps](#use-it-in-3-steps): 5,000 points that chaos tears apart into the Lorenz butterfly. |
| `convergence` | Two particle-built fighters in a circular arena with an orbiting camera; combat loops forever and hits knock matter loose. |
| `supernova` | A star pulses, collapses under a gravity field, explodes into debris and settles into a Lorenz-attractor nebula. |
| `hello_glyph` | The smallest program: one breathing `@` and a gravity field. |
| `playground` | Interactive sandbox: place glyphs, fields and entities with the mouse, cycle attractors and palettes. |
| `colossus` | GPU density entities: millions of particles derived in the vertex shader from sixteen bones. Needs a strong GPU. |
| `apotheosis` | A particle-rendered character built on signed distance fields, about 10.8 million GPU particles. Needs OpenGL 4.3 and a strong GPU. |

Also: `chaos_field`, `particle_demo`, `force_fields`, `amorphous_entity`, `particle_entity`, `full_combat`, `heartbeat`, `showcase` and `sculptor`.

<details>
<summary><b>More captures</b>: <code>supernova</code>, and an older <code>convergence</code> recording (4 MB)</summary>

<img src="https://raw.githubusercontent.com/Mattbusel/proof-engine/main/assets/gifs/supernova.gif" width="70%" alt="The supernova example: a ring of hot glyphs, white, yellow and magenta, expanding after the explosion.">

<img src="https://raw.githubusercontent.com/Mattbusel/proof-engine/main/assets/convergence-demo.gif" width="100%" alt="An older recording of the convergence demo: two particle-built fighters trading blows.">

</details>

## Capture frames from any program

Any program built on the engine, every example included, can write its own frames to disk without a line of code changing. Frames are read back off the GPU after post-processing, so what lands on disk is exactly what the pipeline drew. The images on this page were made this way.

```bash
PROOF_HIDDEN=1 PROOF_FIXED_DT=60 PROOF_SHOT='frames/f_{n}.bmp' \
PROOF_SHOT_AT=560 PROOF_SHOT_EVERY=2 PROOF_SHOT_COUNT=240 \
cargo run --release --example sky
```

| Variable | Meaning |
| --- | --- |
| `PROOF_SHOT` | Output path. `{n}` becomes the capture index, zero padded to four digits. Files are 24-bit BMP. |
| `PROOF_SHOT_AT` | Frame of the first capture (default 120). |
| `PROOF_SHOT_COUNT` / `PROOF_SHOT_EVERY` | How many captures, and how many frames apart (defaults 1 and 1). |
| `PROOF_SHOT_KEEP` | `1` keeps running after the last capture instead of exiting. |
| `PROOF_FIXED_DT` | Step the simulation at this many frames per simulated second, so a sequence plays back at true speed however long each frame took. |
| `PROOF_HIDDEN` | `1` creates the window hidden and unfocused, so capturing does not interrupt whoever is using the machine. |
| `PROOF_WINDOW` | Override the window size, as `WIDTHxHEIGHT`. |

## The screen pipeline

<img src="https://raw.githubusercontent.com/Mattbusel/proof-engine/main/assets/pipeline.svg" width="100%" alt="The screen pipeline: the 3D glyph pass and the UI world pass draw into a half-float scene buffer; emission feeds a bloom pyramid; colour and bloom meet in the composite, then optional FXAA, the screen, and the UI HUD pass painted sharp on top.">

The scene buffers are half-float, so a few hundred thousand overlapping emissive particles accumulate real light instead of clipping at white. The composite is the one place the range comes down, through ACES.

<details>
<summary><b>Pipeline reference</b>: UI passes, the composite, <code>engine.fx</code>, lights, GPU density, sound</summary>

**Two UI passes.** `engine.ui` routes every command to a pass. Particle
clouds and filled rectangles default to the world pass, which is painted into
the HDR buffer before post-processing; text, outlines, bars and sprites
default to the HUD pass, painted sharp on top afterwards. A panel splits: fill
to the world, border to the HUD. `ui.begin_world()`, `ui.begin_hud()` and
`ui.end_pass()` override the default for a run of commands. A game that draws
its whole picture as screen-space matter gets bloom, grade, lens and grain on
all of it, and a readable interface over that.

**What the composite does, in order:** shockwave refraction, heat haze, barrel
lens, chromatic aberration, unsharp mask, floor reflection, exposure,
screen-space indirect light (matter near a lit thing is lit by it), bloom,
halation, light shafts, lens flare, flash, ACES tonemap, lift/gain grade,
tint, contrast, saturation, vignette, shadow-weighted grain, ordered dither,
scanlines. Every standing parameter is a field on `RenderConfig`; the moments
are on `engine.fx`.

**`engine.fx` (ScreenFx).** Fire-and-forget effects that decay on their own:
`shockwave(x, y, strength)`, `flash(color, strength)`,
`light_shaft_at(x, y, strength)` or `auto_shafts = true` to stream from
whatever is brightest on screen, `reflect_at(y, strength, fade)` for a glossy
floor, and `haze` for heat shimmer. Coordinates are UI pixels.

**Lights and shadows.** `engine.fx.light(x, y, radius, color, intensity)`
and `engine.fx.ambient`. The glyph pass writes an occluder buffer (matter,
never floors or panel fills); the light pass marches shadows through it
from every light and the composite multiplies the scene by the result.
Emissive matter lights itself. `config.persistence` keeps a decaying copy
of last frame's scene under this one, for motion trails.

**GPU density entities.** `engine.init_gpu_density(n)` and
`engine.queue_gpu_density_entity(data)`: sixteen bones become millions of
particles derived in the vertex shader from the instance index, with
breathing, jitter and matter that comes loose as `hp` falls. Nothing per
particle ever leaves the GPU. See `examples/colossus.rs`.

**Sound.** A `MathAudioSource` carries a pitch envelope, a second
partial, a noise mix, biquad or comb filters, drive, a reverb send, and a
start delay, and the output thread honours all of it, with separate music
and effects buses, ducking, a master reverb and a soft limiter. A blow is a
crack, a thud and a ring.

**Also:** `render_scale` renders the scene at a fraction of the window and
upsamples; `fxaa` runs a real FXAA 3.11 pass between the composite and the
HUD; `shake_pixels` moves the world pass with camera trauma while the HUD
stays put; `vsync` waits for the display.

</details>

## Proof Editor

![Proof Editor](https://raw.githubusercontent.com/Mattbusel/proof-engine/main/assets/editor-screenshot.png)

Download [`proof-editor.exe`](https://github.com/Mattbusel/proof-engine/releases/download/v0.1.0/proof-editor.exe) (Windows, attached to the v0.1.0 release), or build it:

```bash
cd proof-engine/editor
cargo run --release
```

<details>
<summary><b>Editor keys</b></summary>

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

</details>

## Architecture

Roughly 660,000 lines of Rust across the engine (`src/`), the editor (`editor/`) and the examples. Benchmarks: `cargo bench` (Criterion: `particle_bench`, `glyph_bench`).

<details>
<summary><b>The largest engine modules</b></summary>

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
| `nishita_sky` | physical sky scattering, used by the `sky` example |
| `svogi`, `volumetric_fog`, `tiled_lighting`, `wgpu_backend` | advanced lighting, not yet wired into the demos |

</details>

<details>
<summary><b>The <code>apotheosis</code> example</b></summary>

`examples/apotheosis.rs` is a standalone showcase of a particle-rendered character built on signed distance fields instead of meshes. A 26-bone capsule skeleton is blended with Inigo Quilez's polynomial smooth minimum, particles are importance-sampled onto the SDF shell, and normals, ambient occlusion and subsurface thickness are all computed from the SDF itself. On top of that it layers per-material shading (Schlick Fresnel, Kajiya-Kay hair and fabric specular, thin-film iridescence), a strand-based hair renderer, inertial lag for loose materials, and a post stack including TAA jitter, spectral bloom, god rays, depth of field bokeh and ACES tonemapping. It targets about 10.8 million GPU particles.

</details>

## Related

[chaos-rpg](https://github.com/Mattbusel/chaos-rpg) is a roguelike whose graphical frontend runs on Proof Engine. `CHAOS_RPG_API_CONTRACT.md` documents what the engine has to support for it.

## Status

Early (0.2) and moving fast. The public API is not stable and some subsystems are further along than others. CI builds every target and runs the unit, integration and doc tests on Linux, and builds the examples on Windows and macOS. About 4,800 library unit tests pass; the ones that do not yet are listed by name in [`ci/known-failing-tests.txt`](https://github.com/Mattbusel/proof-engine/blob/main/ci/known-failing-tests.txt), and each one fixed is a line deleted from that file. Contributions: see [CONTRIBUTING.md](https://github.com/Mattbusel/proof-engine/blob/main/CONTRIBUTING.md).

## License

MIT, see [LICENSE](https://github.com/Mattbusel/proof-engine/blob/main/LICENSE).

## Hire the author

**Need this kind of engineering on your product?** I take on a small number of client builds: LLM features, iOS apps and performance work, fixed price. [Services and pricing](https://mattbusel.github.io/) · [Email](mailto:mattbusel@gmail.com) · [LinkedIn](https://www.linkedin.com/in/matthewbusel/)
