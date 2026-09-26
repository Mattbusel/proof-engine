# Changelog

## 0.2.1

### Fixed
- `galaxy`, `math_rain` and `strange_attractors` no longer draw a blown-out first second and then an empty screen ([#7](https://github.com/Mattbusel/proof-engine/issues/7)). They used `life_function` values of 5 to 25 as glyph scale, and fed strong inverse-square and flow fields to glyphs of mass 0.01 to 0.1, so everything was huge at first and then flung off screen. The three demos now compute positions from their equations every frame (orbits, falling columns, RK4 on each attractor) and run stable indefinitely.
- `initial_state(AttractorType::Dadras)` started on the invariant line y = z = 0 and decayed to the origin; `initial_state(AttractorType::Rabinovich)` escaped to infinity within 2,000 steps. Both now start from standard seeds, and a unit test checks every attractor settles onto a finite, non-trivial state.
- `ProofEngine::run` now honours `input.quit_requested`, as `run_ui` already did, so Esc handlers in `run` demos work.

### Added
- `quickstart` example: the README's three-step program.
- Crate docs open with a runnable doc test and the quickstart program; README and crate page show real GIFs captured with `PROOF_HIDDEN=1`.

## 0.2.0

Engine work that was developed alongside [chaos-rpg](https://github.com/Mattbusel/chaos-rpg) and had not been pushed, plus CI and a new demo.

### Added
- `ProofEngine::run_ui` for UI-driven games (update before draw, UI layer cleared each frame), `render_size`, and `save_frame` for reading the framebuffer back to a BMP.
- HDR scene buffers and a two-pass UI layer (`UiPass::World` painted before post-processing, `UiPass::Hud` painted sharp after it).
- `engine.fx` (`ScreenFx`): shockwaves, flashes, light shafts, floor reflection, heat haze, screen-space lights with shadows.
- Bloom pyramid, FXAA, `render_scale`, motion-trail persistence, camera shake that leaves the HUD still.
- GPU density entities (`init_gpu_density`, `queue_gpu_density_entity`) and particle skinning (`anim::particle_skin`).
- Synthesiser voices on `MathAudioSource`: pitch envelope, second partial, noise, filters, drive, reverb send, start delay; music and effects buses with ducking and a limiter.
- `sky` example: a day cycle lit by the Nishita sky model (`nishita_sky`), the first of the advanced lighting modules used by a demo.
- CI on GitHub Actions: build of every target, library, integration and doc tests on Ubuntu; example builds on Windows and macOS.

### Fixed
- Bloom: each pyramid level had one shared half-resolution scratch texture, so smaller levels were blurred with three quarters of the clear colour and the bloom showed a visible seam at the screen's centre lines. Each level now has its own scratch target.
- Integer overflow panics in debug builds in the hash functions of `particle::density_entity`, `volumetric_fog`, `editor::terrain` and `editor::map_editor` loot rolls (16 unit tests now pass).
- `particle_bench` did not compile after `MathParticle` gained fields.
- Five doc examples (`ai::steering`, `animation`, `debug::console`, `game::transitions`, `replay`) did not compile against the current API.

### Changed
- `MathParticle` has new public fields; struct literals need `..Default::default()`.

### Known issues
- 79 library unit tests fail and are listed in `ci/known-failing-tests.txt`; CI skips exactly those.

## 0.1.1

Initial crates.io release.
