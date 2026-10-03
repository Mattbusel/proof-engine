# Changelog

## 0.3.0

This release swaps several pieces the engine had written by hand for well-known open-source crates, fixes the bugs that turned up along the way, and adds what those crates make easy.

### Added
- **Animated GIFs straight from the engine.** Set `PROOF_SHOT=out.gif` with `PROOF_SHOT_COUNT` above 1 and any program records one looping GIF, timed from `PROOF_FIXED_DT`. `PROOF_SHOT_WIDTH` scales captures down, so a README-sized GIF needs no other tool.
- **Captures in any common format.** `PROOF_SHOT` and `ProofEngine::save_frame` pick PNG, JPEG, BMP, TGA or GIF from the file extension. New `export` module (`save_rgba`, `load_rgba`, `scale_to_width`, `GifRecorder`) for saving pixel buffers of your own, and `ProofEngine::frame_pixels` to get the drawn frame as RGBA.
- **Equations in a script, reloaded live.** `math::scripted::ScriptedSystem` runs a 3D system whose equations are a short [Rhai](https://rhai.rs) script, with RK4 stepping and `reload_if_changed` for editing while it runs. A broken edit is reported and the last good version keeps running. New headless example `scripted_attractor` draws any such script to a PNG and redraws on every save; sample scripts for Lorenz, Thomas and Aizawa are in `examples/scripts`.
- **38 named colour maps.** `math::color::preset_gradient("turbo")` and friends: viridis, magma, inferno, plasma, cividis, turbo, sinebow, cubehelix and the ColorBrewer sets. `Gradient::from_css("#000, deeppink 40%, gold")` reads CSS gradient syntax.
- **Sound without a sound card.** `audio::OfflineRenderer` runs the real synthesiser and hands back the samples; `audio::wav` reads and writes WAV files. New headless example `audio_bounce` renders a short piece to a WAV.
- `SerializedValue::to_json_value` and `from_json_value` for working with save data as a `serde_json::Value`.

### Fixed
- Image assets: the loader claimed PNG, JPEG, BMP and TGA but returned a 1x1 magenta square for all of them. It now decodes them (and GIF), and a file it cannot read is a load error instead of a silent placeholder.
- Sound assets: the loader claimed WAV, OGG, MP3 and FLAC but returned a second of silence for all of them. WAV now loads for real; other formats are reported as errors.
- Save files: text with any non-ASCII character (an accented name, CJK, emoji) came back garbled, because the JSON reader handled one byte at a time. Malformed files with trailing junk were also accepted.
- The headless renderer's `render_to_png` wrote a TGA whatever the name. It writes the format the extension asks for, and `try_render_to_file` returns write errors.
- `gradient_viridis`, `gradient_plasma` and `gradient_inferno` were rough five-stop approximations; they now follow the published maps.

### Changed
- Built on [image](https://crates.io/crates/image), [serde_json](https://crates.io/crates/serde_json), [hound](https://crates.io/crates/hound), [colorgrad](https://crates.io/crates/colorgrad) and [rhai](https://crates.io/crates/rhai), all MIT or Apache-2.0. About 300 lines of hand-written BMP, TGA and JSON code are gone.
- Removed the `noise` and `rodio` dependencies, which nothing used.
- Rhai is behind a new default feature, `rhai-scripts`. Rhai adds `Add` impls for `String`, so in a crate that links it, `String + &String` needs `.as_str()` on the right. If that breaks your build, use `default-features = false`.
- `RawSoundLoader` no longer lists `ogg`, `mp3` or `flac`, and `RawImageLoader` adds `gif`.

## 0.2.3

### Added
- Math screensaver: `proof-screensaver.scr` on every release, ten of the engine's strange attractors as a Windows screensaver (right-click it, **Install**). Source in `saver/` (not published to crates.io); details in [docs/SCREENSAVER.md](docs/SCREENSAVER.md). With `PROOF_HIDDEN=1` set it never shows a window, so `/s`, `/c` and `/p` can be tested on a machine someone is using.
- Linux demos: a GitLab CI job builds the ten demos and the editor on Debian bullseye for every `v*` tag and attaches `proof-engine-demos-linux-x64.tar.gz` to the release.

### Changed
- With no display to open a window on (a Linux shell without X11 or Wayland, a CI runner), programs print `proof-engine: cannot open a window: ...` and exit with status 1 instead of panicking.
- Windows downloads rebuilt at 0.2.3.

## 0.2.2

### Added
- Prebuilt demos on every GitHub release: `proof-lorenz.exe`, `proof-galaxy.exe`, `proof-sky.exe`, `proof-strange_attractors.exe` and `proof-editor.exe` as single Windows files, plus all ten demos and the editor zipped for Windows and packed for macOS and Linux. No Rust needed to watch them.

### Changed
- README cut to one screen: a direct Windows download, a "how math becomes a frame" diagram and the same frame captured at each render stage, four real examples and the three-step program. Install, demos, architecture, capture and editor reference moved to `docs/` unchanged.
- Project site: search title and description, canonical URL, Open Graph and Twitter cards, JSON-LD, a download button and the new diagrams.

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
