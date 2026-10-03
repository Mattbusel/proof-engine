# Capture frames from any program

[README](../README.md) · [Install](INSTALL.md) · [Demos](DEMOS.md) · [How it works](ARCHITECTURE.md) · [Capture](CAPTURE.md) · [Editor](EDITOR.md)


Any program built on the engine, every example included, can write its own frames to disk without a line of code changing. Frames are read back off the GPU after post-processing, so what lands on disk is exactly what the pipeline drew. The images on this page were made this way.

```bash
PROOF_HIDDEN=1 PROOF_FIXED_DT=60 PROOF_SHOT='frames/f_{n}.png' \
PROOF_SHOT_AT=560 PROOF_SHOT_EVERY=2 PROOF_SHOT_COUNT=240 \
cargo run --release --example sky
```

## Record a GIF

Give `PROOF_SHOT` a `.gif` path with no `{n}` in it and a count above 1, and the whole run goes into one looping GIF. This records four seconds of the Lorenz demo at 30 frames per second, 480 pixels wide:

```bash
PROOF_HIDDEN=1 PROOF_FIXED_DT=30 PROOF_SHOT=lorenz.gif \
PROOF_SHOT_AT=300 PROOF_SHOT_COUNT=120 PROOF_SHOT_WIDTH=480 \
cargo run --release --example lorenz
```

Each frame gets its own 256 colour palette, so smooth glows survive better than with one palette for the whole file. Frames are written as they arrive, so a long recording does not fill memory. The frame delay comes from `PROOF_FIXED_DT` and `PROOF_SHOT_EVERY`, so the GIF plays at the speed the simulation ran.

To do the same from code, `proof_engine::export::GifRecorder` takes RGBA frames (from `ProofEngine::frame_pixels`, or anything else) and `export::save_rgba` writes single images.

## Settings

| Variable | Meaning |
| --- | --- |
| `PROOF_SHOT` | Output path. The extension picks the format: `.png`, `.jpg`, `.bmp`, `.tga` or `.gif`; anything else is written as BMP. `{n}` becomes the capture index, zero padded to four digits. |
| `PROOF_SHOT_AT` | Frame of the first capture (default 120). |
| `PROOF_SHOT_COUNT` / `PROOF_SHOT_EVERY` | How many captures, and how many frames apart (defaults 1 and 1). A `.gif` path without `{n}` and a count above 1 makes one animated GIF. |
| `PROOF_SHOT_WIDTH` | Scale every capture down to this many pixels wide, keeping the shape. |
| `PROOF_SHOT_KEEP` | `1` keeps running after the last capture instead of exiting. |
| `PROOF_FIXED_DT` | Step the simulation at this many frames per simulated second, so a sequence plays back at true speed however long each frame took. Also sets the GIF frame delay. |
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
