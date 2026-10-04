# Changelog

## 0.3.0

This release swaps several pieces the engine had written by hand for well-known open-source crates, fixes the bugs that turned up along the way, and adds what those crates make easy.

### Added
- **Real networking, behind features.** `networking::http::HttpClient` used to answer every request with a made-up empty `200 OK` after 50 ms, and `networking::websocket::WsClient` pretended to connect to any URL and dropped every message. With the new `http` feature requests go out through [ureq](https://crates.io/crates/ureq) on background threads (retries with backoff, ETag / Last-Modified revalidation, rate limits); with `websocket` the client opens real `ws://` / `wss://` connections through [tungstenite](https://crates.io/crates/tungstenite). `net` turns on both. Without them the clients report an error. Tested against servers the tests start on 127.0.0.1.
- **`math::attractors::rk4_step_all`** steps a whole slice of points; the new `parallel` feature spreads it over all cores with rayon. 40,000 Lorenz points: 1.21 ms per frame on one core, 0.10 ms with `parallel` (i7-13700KF, `cargo bench --bench attractor_bench`), with bit-identical results.
- **Cross-check against ode_solvers.** `tests/ode_cross_check.rs` writes the Lorenz, Rossler, Thomas, Aizawa and Chen equations out again in f64 and integrates them with the `ode_solvers` crate's RK4; `rk4_step` agrees to better than 1e-4 relative error.
- `benches/attractor_bench.rs`. Against 0.2.3 on the same machine, `rk4_step` and the particle and glyph pool benches are unchanged within noise (the maths did not change); the speed-up is only through `rk4_step_all`.
- GitLab CI now runs the library tests, the integration and doc tests, clippy (`-D warnings`, all features) and `cargo doc -D warnings` on every push.
- **Animated GIFs straight from the engine.** Set `PROOF_SHOT=out.gif` with `PROOF_SHOT_COUNT` above 1 and any program records one looping GIF, timed from `PROOF_FIXED_DT`. `PROOF_SHOT_WIDTH` scales captures down, so a README-sized GIF needs no other tool.
- **Captures in any common format.** `PROOF_SHOT` and `ProofEngine::save_frame` pick PNG, JPEG, BMP, TGA or GIF from the file extension. New `export` module (`save_rgba`, `load_rgba`, `scale_to_width`, `GifRecorder`) for saving pixel buffers of your own, and `ProofEngine::frame_pixels` to get the drawn frame as RGBA.
- **Equations in a script, reloaded live.** `math::scripted::ScriptedSystem` runs a 3D system whose equations are a short [Rhai](https://rhai.rs) script, with RK4 stepping and `reload_if_changed` for editing while it runs. A broken edit is reported and the last good version keeps running. New headless example `scripted_attractor` draws any such script to a PNG and redraws on every save; sample scripts for Lorenz, Thomas and Aizawa are in `examples/scripts`.
- **38 named colour maps.** `math::color::preset_gradient("turbo")` and friends: viridis, magma, inferno, plasma, cividis, turbo, sinebow, cubehelix and the ColorBrewer sets. `Gradient::from_css("#000, deeppink 40%, gold")` reads CSS gradient syntax.
- **Sound without a sound card.** `audio::OfflineRenderer` runs the real synthesiser and hands back the samples; `audio::wav` reads and writes WAV files. New headless example `audio_bounce` renders a short piece to a WAV.
- `SerializedValue::to_json_value` and `from_json_value` for working with save data as a `serde_json::Value`.

### Fixed
- 76 of the 79 library tests that had never run (listed in `ci/known-failing-tests.txt`) now pass. 56 were real bugs, 20 were wrong expectations in the tests. The bugs, by area:
  - **Maths and physics:** skinning used the bind matrix instead of its inverse; Verlet cloth used `a*dt` for `a*dt^2`; the SPH kernel had twice the intended support; Lotka-Volterra predators grew with no prey; a Schroeder allpass was not an allpass (58% too much energy); the FIR bandpass had the wrong gain; Biot-Savart was 33% low; the Mandelbulb SDF was NaN at the origin; Black-Scholes mixed two CDF formulas; a 3x3 determinant applied the cofactor sign twice; modularity left out the expected-edge terms; Gaussian curvature paired the wrong triangles; alpha decay took `acos` of values above 1; the Doppler factor was the wavelength ratio; the Lorentz boost mixed `ct` and `t`; Terrell positions ignored retarded time; the Schrodinger energy scan skipped low levels; coupled oscillators started in a stable twisted state.
  - **Editors and tools:** undo did nothing in the audio mixer and material editor, and could not undo the first change in colour grading; redo lost added tracks in the cinematic sequencer; visibility fades jumped; the HRTF panner delayed the far ear even straight ahead; the limiter let transients through; AASHTO stopping distance had the grade term's sign wrong; terrain patch LOD used a lifted centre; render-graph hot reload reported every pass as changed and depth-to-NDC had both signs wrong.
  - **Content generation:** open Catmull-Rom splines collapsed to the origin at the ends; A* skipped every node after the start; caves filled in solid; rivers never formed on even slopes; biome blending ran over half its range; a wave spawned nothing on its first tick; dialogue choices never appeared after skipping the typewriter; a spring easing ended at 1.0022; SDF generation zeroed one-pixel shapes; death fades snapped back.
  - **Scripting VM:** calls kept only one return value, functions returning nothing ate a local, errors left a dead frame, `t.x = v` compiled the value below the table, and table constructors stored items under the key `""`.
- The three still listed: `procedural::world` (two tests: seed 42 never produces land; needs a decision on sea level) and `physics::constraints::ball_socket_cone_limit` (the cone limit needs a proper velocity-level solve).
- `ml::OnnxLoader::load_onnx` loaded element-wise Add and Mul nodes as ReLU, silently zeroing negative values; they are now rejected. Its docs now say plainly that it reads the engine's own format, not ONNX protobuf.
- `networking::leaderboard`: fetched boards were always empty (the parser was a stub), names with quotes produced invalid JSON, `checksum` / `replay_id` / `min_score` / `max_score` were never sent, query values were not URL-encoded, and timeouts or cancellations never reached the caller. All fixed and tested.
- `networking::http`: `UseCache` served stale entries forever, and `retry_after` was set but never checked, so retries went out at once.
- Two tests asserted nothing (`x <= 255` on a `u8`, `pending_count >= 0` on an unsigned count); they now check real values.
- Module docs that overstated things: `networking` claimed TCP sockets it did not have, `network` is in-memory message types, and `save::cloud` "encryption" is XOR obfuscation.
- `cargo doc` produced 136 warnings (bracketed maths like `[0,1]` read as links, stray HTML); now zero. Clippy is clean with `-D warnings` on Rust 1.91 and 1.99.
- Image assets: the loader claimed PNG, JPEG, BMP and TGA but returned a 1x1 magenta square for all of them. It now decodes them (and GIF), and a file it cannot read is a load error instead of a silent placeholder.
- Sound assets: the loader claimed WAV, OGG, MP3 and FLAC but returned a second of silence for all of them. WAV now loads for real; other formats are reported as errors.
- Save files: text with any non-ASCII character (an accented name, CJK, emoji) came back garbled, because the JSON reader handled one byte at a time. Malformed files with trailing junk were also accepted.
- The headless renderer's `render_to_png` wrote a TGA whatever the name. It writes the format the extension asks for, and `try_render_to_file` returns write errors.
- `gradient_viridis`, `gradient_plasma` and `gradient_inferno` were rough five-stop approximations; they now follow the published maps.

### Changed
- `rust-version = "1.88"` (image 0.25.10 needs it); docs.rs builds with all features.
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
