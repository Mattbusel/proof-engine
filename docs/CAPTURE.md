# Capture frames from any program

[README](../README.md) · [Install](INSTALL.md) · [Demos](DEMOS.md) · [How it works](ARCHITECTURE.md) · [Capture](CAPTURE.md) · [Editor](EDITOR.md)


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

## How the stage-by-stage picture was made

[`img/frame-stages.jpg`](img/frame-stages.jpg) in [ARCHITECTURE.md](ARCHITECTURE.md) is three captures of the `lorenz` example at frame 600 (`PROOF_HIDDEN=1 PROOF_FIXED_DT=60 PROOF_SHOT_AT=600`), from a copy of `examples/lorenz.rs` whose `RenderConfig` was changed per capture and whose HUD text was left out:

| Panel | `RenderConfig` changes from the example's own |
| --- | --- |
| 1. Glyph pass only | `bloom_enabled: false, persistence: 0.0, tonemap: 0.0, exposure: 1.0, vignette: 0.0, contrast: 1.0` |
| 2. + trails and bloom | `tonemap: 0.0, exposure: 1.0, vignette: 0.0, contrast: 1.0` |
| 3. + tonemap and grade | none |

With a fixed time step the simulation is deterministic, so all three show the same 40,000 points. Each 1280x720 frame was centre-cropped and scaled to one third of the image.
