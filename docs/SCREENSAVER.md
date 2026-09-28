# Proof Saver: the engine as a Windows screensaver

`proof-saver` is a small program built on Proof Engine that runs its strange attractors as a Windows screensaver, or as live art in a window. Every point on screen is a state `(x, y, z)` advanced each frame by the engine's own RK4 integrator (`proof_engine::math::attractors::rk4_step`). Nothing is a video or a precomputed path.

![Proof Saver cycling from the Lorenz attractor to the Aizawa attractor](img/saver-cycle.gif)

## Install

1. Download `proof-saver.zip` from the [releases page](https://github.com/Mattbusel/proof-engine/releases) and unzip it somewhere it can stay (Windows runs the file from where it is; if you move it later, install it again).
2. Right-click `proof-saver.scr` and choose **Install**. Windows opens Screen Saver Settings with Proof Saver selected. Click **OK**.
3. **Settings...** in that dialog opens Proof Saver's own settings.

The file is not code-signed, so SmartScreen may warn the first time. The source is all in this repository under [`saver/`](../saver/).

To remove it: pick another screensaver (or None) in Screen Saver Settings, then delete `proof-saver.scr` and the folder `%APPDATA%\ProofSaver`.

## What it shows

Ten attractors from `proof_engine::math::attractors`, one at a time, fading from one to the next: Lorenz, Aizawa, Thomas, Halvorsen, Rössler, Dadras, Chen, Rabinovich-Fabrikant, Sprott B and Burke-Shaw. Each starts with 8,000 to 48,000 points (scaled to the monitor's size) spread along one settled trajectory, and from then on the equations move them. Colour is speed along the flow. The view turns slowly. For a few seconds after each fade in, the attractor's name and equations show in the corner (this can be turned off).

With more than one monitor, one borderless window covers them all and each monitor gets its own attractor (or the same one, or black: a setting).

## Settings

| Setting | Choices |
| --- | --- |
| Attractor | Cycle through all ten, or one only |
| Time on each | 20 s, 45 s, 90 s, 3 min, 10 min |
| Motion speed | Slow, Normal, Fast |
| Frame rate cap | 24, 30 (default) or 60 fps; it sleeps between frames |
| Other monitors | Own attractor, same attractor, black |
| Equations | Show the name and equations for a few seconds, or never |

Saved to `%APPDATA%\ProofSaver\settings.toml`. **Try it in a window** opens the live-art window with the chosen settings.

## As live art

```text
proof-saver.exe --window        # Esc quits, F11 fullscreen, Right arrow skips
proof-saver.exe --fullscreen    # the mouse does not end it; Esc does
proof-saver.exe --scene aizawa --window
```

## Frame capture without a window

```text
proof-saver.exe --record frames --scene thomas --size 1080x1080 --frames 240
```

writes `frames/frame_0000.bmp` onwards without showing anything, using the engine's `PROOF_HIDDEN` and `PROOF_SHOT` capture. `proof-saver.exe --help` lists every option.

## Build it

```bash
cd proof-engine/saver
cargo build --release
copy target\release\proof-saver.exe proof-saver.scr
```

A `.scr` file is an ordinary Windows program with a different extension. It implements the screensaver arguments: `/s` runs fullscreen until a key, a click or a mouse movement of more than 12 pixels; `/p <hwnd>` draws into the preview box by making its window a child of the one Windows passes; `/c` and no arguments open the settings dialog.
