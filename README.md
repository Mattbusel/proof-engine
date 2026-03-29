# Proof Engine


## Live Demo

![Proof Engine ~ Convergence](https://github.com/Mattbusel/proof-engine/blob/main/Screenshot%202026-03-25%20231046.png?raw=true)

![Proof Engine ~ Convergence](https://github.com/Mattbusel/proof-engine/blob/main/ezgif.com-video-to-gif-converter%20(4).gif?raw=true)

**What you're looking at:** Two humanoid entities rendered entirely from particles. No meshes. No skeletons. No sprites. Every figure is millions of independent particles held together by spring-force physics, the same way real matter holds its shape through intermolecular forces. Each particle is its own light source with emission, color, temperature, and physical mass. The engine does not distinguish between geometry and lighting. The matter IS the light.

When an entity takes damage, it doesn't play a death animation. It physically disintegrates because the forces holding it together are overcome. Destruction, deformation, cloth, fluid, fog, and soft-body behavior all emerge from the same particle system with zero additional engineering — just different spring constants on the same substrate. There is no polygon budget. There is no pre-fractured mesh. Destruction resolution is infinite because particles don't have polygon limits.

**What you're NOT seeing:** This demo is running with no lighting pipeline, no shaders, no post-processing, and no material system connected. The engine's full rendering stack — clean-room SVOGI (Sparse Voxel Octree Global Illumination rebuilt from the published SIGGRAPH papers that powered CryEngine), spherical harmonics, Nishita atmospheric scattering, deferred caustics, PBR materials, and volumetric fog — exists in the codebase but has not been turned on yet. When it is, every particle becomes a light emitter whose glow bounces off every surface through voxel light propagation. The lighting doesn't approximate the scene. The scene IS the light field.

**What matters:** 50 million particles is not the ceiling. It's a development parameter. The architecture has no hard limit on particle count. Visual fidelity scales by turning one number up, more particles means smoother surfaces, denser matter, higher-resolution destruction, and richer light fields. No other engine scales fidelity with a single parameter because no other engine uses continuous matter as its rendering primitive.

Every other game engine renders polygons and then fakes destruction, fakes fluid, fakes cloth, fakes volumetric light, and fakes material behavior through separate engineered systems. This engine doesn't fake anything. The physics are real. The matter is real. The light emission is real. The visual output is what the mathematics produces.

Particles are not an effect. Particles are the rendering primitive. Everything in the scene is made of them.

![Supernova Demo](assets/supernova-demo.gif)

## What is this?

Proof Engine renders mathematics, not graphics. A Lorenz attractor looks like a Lorenz attractor because particles follow the actual differential equations in real time. Entities are held together by force fields and dissolve into strange attractors when they die. Audio is synthesized from music theory, not audio files.

This is not a traditional game engine. It is a system where the math IS the visual.

## Proof Editor

A visual staging environment for building scenes, placing force fields, and tweaking every parameter in real time. Built with egui on top of the engine viewport.

![Editor Screenshot](assets/editor-screenshot.png)

**Download the editor:** [Releases page](https://github.com/Mattbusel/proof-engine/releases)

### Editor features

- Place glyphs, force fields, and entities by clicking in the viewport
- 10 force field types: Gravity, Vortex, Lorenz, Rossler, Chen, Thomas, Flow, Shockwave, and more
- Live property inspector with position, color, emission, glow sliders
- Hierarchy panel with search, filter, and collapsible tree structure
- Post-processing panel: bloom, chromatic aberration, film grain with preset buttons (Cinematic, Neon, Retro, Clean)
- Asset browser with prefab spawning (Lorenz Cluster, Vortex Ring, etc.)
- Console with command input and color-coded log
- Full undo/redo across all operations
- Save/load scenes to JSON
- Copy/paste, duplicate, box select, multi-select

## Getting started

### Run the editor

Download `proof-editor.exe` from the [Releases page](https://github.com/Mattbusel/proof-engine/releases) and double-click it.

Or build from source:

```
git clone https://github.com/Mattbusel/proof-engine.git
cd proof-engine/editor
cargo run --release
```

### Run the demos

```
cd proof-engine
cargo run --release --example galaxy
cargo run --release --example supernova
cargo run --release --example math_rain
cargo run --release --example heartbeat
```

### Use as a library

```toml
[dependencies]
proof-engine = { git = "https://github.com/Mattbusel/proof-engine.git" }
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

## Editor controls

| Key | Action |
|-----|--------|
| Click viewport | Place with current tool |
| WASD / Arrows | Pan camera |
| V | Select tool |
| G | Move tool (drag to reposition) |
| P | Place glyph tool |
| F | Place force field tool |
| E | Place entity tool |
| X | Particle burst tool |
| Shift+Click | Multi-select |
| Ctrl+C / Ctrl+V | Copy / Paste |
| Ctrl+Z / Ctrl+Y | Undo / Redo |
| Ctrl+S / Ctrl+O | Save / Load |
| Ctrl+N | New scene |
| Delete | Remove selection |
| Space | Screen shake |
| F1 | Help |

## Engine capabilities

**Rendering:** OpenGL 3.3, glyph instancing, bloom, chromatic aberration, film grain, vignette, scanlines, motion blur

**Math functions:** Lorenz, Rossler, Chen, Halvorsen, Aizawa, Thomas attractor integration. Sine, cosine, Perlin noise, logistic map, Collatz, golden spiral, Lissajous, Mandelbrot escape, spring-damper systems

**Force fields:** Gravity, vortex, electromagnetic, strange attractor, shockwave, tidal, flow, magnetic dipole. Composable with falloff (linear, inverse square, exponential, Gaussian)

**Physics:** 2D rigid body with SAT collision, soft body mass-spring, Eulerian fluid simulation, constraints and joints

**Audio:** 48kHz synthesis, ADSR envelopes, waveform oscillators, FM synthesis, music theory (scales, chords, progressions), spatial audio with stereo panning and room reverb

**Entities:** Amorphous glyph formations held together by force cohesion. HP-linked binding strength. Dissolve into attractors on death

**Scripting:** Custom bytecode VM with lexer, parser, compiler. Dynamic typing, closures, tables, metatables

**Procedural generation:** Tectonic plates, hydraulic/thermal erosion, climate simulation, biome classification, river networks, cave systems, settlement placement, civilization history, language generation, mythology, genetics

**Ecology:** Lotka-Volterra dynamics, food webs, migration, evolution, SIR disease models

**Narrative:** Story grammars, character motivation, dialogue generation, quest generation, drama management, NPC memory, procedural poetry

## Architecture

460,000+ lines of Rust across the engine, editor, and game frontend.

| Module | Lines | Description |
|--------|-------|-------------|
| game | 28,891 | Boss AI, fluids, cloth, debris, achievements |
| render | 26,849 | OpenGL pipeline, PBR, post-FX, shader graph |
| math | 12,626 | Attractors, fields, curves, noise, springs |
| terrain | 12,505 | Heightmaps, erosion, biomes, streaming |
| physics | 9,018 | Rigid body, soft body, fluid, constraints |
| audio | 8,870 | Synth, music, effects, spatial |
| editor (engine) | 6,883 | State, inspector, hierarchy, console, gizmos |
| ecs | 7,187 | Archetype ECS, generational IDs, queries |
| scripting | 6,933 | Lexer, parser, compiler, bytecode VM |
| worldgen | 3,272 | Tectonics, climate, rivers, caves, history |
| + 45 more modules | ... | ... |

## Kit System

The apotheosis rendering pipeline is composed of eight kits. Each kit is a self-contained rendering subsystem that runs once per base particle and can be toggled independently.

| Kit | Description |
|-----|-------------|
| **BoneKit** | 26-bone skeleton defining Leon's body as axis-aligned capsule descriptors with per-bone particle weight and aspect ratio |
| **ModelKit** | Per-bone anatomical cross-section profile via piecewise-cosine knot tables; joint continuity enforced at every bone junction |
| **MaterialKit** | Classifies each surface point into one of six material tags (Skin, Hair, Jacket, Boot, Metal, Eye) and computes Schlick Fresnel edge brightening |
| **LightingKit** | Warm directional key + three coloured fills + squared rim + hemisphere ambient; ACES filmic tonemap and S-curve contrast |
| **ClothingKit** | Identifies fabric-carrying bones; adds radial garment push via clothing_offset(), seam darkening at material boundaries, and contact shadow at skin/fabric transitions |
| **HairKit** | 500-strand curtain-cut renderer across five scalp zones; gravity-curved spline chains with wind sway and Kajiya-Kay anisotropic specular |
| **PhysicsKit** | Per-particle inertial lag cache; loose materials (hair, jacket hem) trail behind on character movement using per-material smoothing weights |
| **RenderKit** | Depth-of-field jitter, n_copies particle scatter to fill sub-pixel gaps, and per-copy alpha/emission scaling to preserve total luminance |

## SDF Architecture

The engine uses signed distance fields (SDF) as its geometry representation instead of mesh geometry. Every body surface — torso, arms, legs, face — is defined by an analytic implicit function whose value at any point in space gives the exact Euclidean distance to the nearest surface.

**Primitives:**
- `sdf_torso` — superellipsoid body with piecewise-linear ax/az cross-section profiles along Y (broad shoulders, cinched waist, hip flare)
- `sdf_arm_r` / `sdf_forearm_r` — tapered elliptic capsules for each arm segment
- `sdf_leg_r` — smooth union of thigh + shin/boot capsules
- `sdf_face` — face topology with eye sockets, nose bridge, lip geometry, iris/sclera layers

**Smooth minimum blending at joints:** All primitives are combined via Inigo Quilez's polynomial `smin(a, b, k)` function, which rounds the hard `min()` discontinuity into an organic blend zone. Shoulders merge into the torso, knees merge thigh into shin — all without visible seams or capsule gaps.

**Analytical normals from SDF gradient:** Surface normals are computed as the gradient of the SDF field at each surface point using finite differences of `sdf_body`. This gives numerically exact normals with zero polygon faceting or normal-map baking.

**Mathematically provable ambient occlusion:** AO is computed by marching a short ray along the outward surface normal and measuring how much the SDF value at each step falls below the step distance. When geometry is nearby, the SDF value is small — the body occludes its own escape horizon. The result matches the exact geometry (armpits, waist creases, inner elbows) with no pre-baked textures or screen-space approximations.

**Subsurface scattering from SDF thickness:** An inward `-n` thickness probe marches through the body until the SDF becomes positive (surface exit). The probe depth gives tissue thickness; Beer-Lambert attenuation then produces physically accurate SSS — thin regions (ears, lip edges) transmit warm light; thick regions (chest, thigh) are opaque.

**Importance sampling (near-100% acceptance):** Rather than rejecting uniform box samples that miss the surface (typically 92%+ rejection), each candidate particle is placed directly on the expected ellipse/capsule surface and perturbed ±SHELL in the outward radial direction. The SDF then verifies shell placement — acceptance rate is ~100% with zero wasted evaluations.

## Post-Processing Pipeline

23 post-processing techniques are applied across three stages: CPU particle shading, GPU compute shader, and the billboard fragment shader.

| # | Technique | Description |
|---|-----------|-------------|
| 2 | SDF analytical reflection trace | Ray-marches the reflected view vector against `sdf_body` — no BVH, no mesh required |
| 3 | Atmospheric depth scattering | Back-of-body particles (pz < 0) scatter toward cool deep blue for volumetric depth |
| 4 | TAA sub-pixel jitter | Halton(2,n) × Halton(3,n) per-frame offset covers the full pixel footprint for smooth edges |
| 5 | Spectral bloom dispersion | Per-channel radial disk offsets cause the engine's Gaussian blur to produce prismatic rainbow fringes |
| 6 | Chromatic depth separation | Particles farther from the near plane get per-channel disk radius shifts simulating lens dispersion |
| 7 | SSR via SDF surface normal | Analytical SDF gradient normals importance-sample an implicit dungeon environment map |
| 8 | Volumetric light shafts / god rays | Beer-Lambert transmittance along the key-light ray reveals lit corridors between arm and torso |
| 9 | Cross-material GI color bleed | Jacket spills warm orange onto adjacent skin; pants bleed cool green into jacket hem |
| 10 | Luminance-based film grain | Shadow regions receive heavier grain amplitude matching real photographic film stock |
| 11 | Vignette + edge desaturation | Screen-space darkening and desaturation anchored to monitor frame via gl_FragCoord |
| 12 | ACES filmic tonemapping | Narkowicz 2015 approximation; richer shadows, cleaner highlights than Reinhard |
| 13 | Eye adaptation | Per-frame auto-exposure scalar applied to final composite color |
| 14 | Edge sharpen | Ring boost at d ≈ 0.65 steepens the transition between adjacent particles |
| 15 | Heat haze | High-emission particles distort their disk edge with a sine ripple — impossible in triangle rendering |
| 16 | Kajiya-Kay anisotropic specular | SDF tangent derived analytically; horizontal weft for fabric, along-strand for hair |
| 17 | SDF micro-displacement pore shadow | Two SDF differential samples along key-light direction produce concave pore shadows on skin |
| 18 | DoF bokeh ring | Per-particle annular ring grows as depth diverges from focal plane, mimicking fast-lens bokeh |
| 19 | Iridescent thin-film | 180 nm thin-film interference shifts leather from warm olive to faint teal at glancing angles |
| 20 | SDF-curvature Fresnel rim | Hessian Laplacian amplifies rim light on convex geometry (knuckles, cheekbones, collar edge) |
| 21 | Thickness-modulated SSS | Beer-Lambert skin transmission gated on SDF thickness probe depth |
| 22 | SDF-analytical ambient occlusion | IQ 5-step geometric-decay AO march along surface normal; exact to SDF precision |
| 23 | Eye spectral refraction | Per-wavelength IOR dispersion through the cornea SDF sphere; real chromatic aberration of the eye |

## License

MIT
