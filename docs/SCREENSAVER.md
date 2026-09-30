# Proof Saver: the engine as a Windows screensaver

`proof-saver` is a small program built on Proof Engine that runs its strange attractors as a Windows screensaver, or as live art in a window. Every point on screen is a state `(x, y, z)` advanced each frame by the engine's own RK4 integrator (`proof_engine::math::attractors::rk4_step`). Nothing is a video or a precomputed path.

## Install

1. Download [`proof-screensaver.scr`](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-screensaver.scr) and put it somewhere it can stay (Windows runs the file from where it is; if you move it later, install it again).
2. Right-click `proof-screensaver.scr` and choose **Install**. Windows opens Screen Saver Settings with it selected (listed as *proof-screensaver*). Click **OK**. (Or copy it into `C:\Windows\System32`; it then appears in the list in Screen Saver Settings.)
3. **Settings...** in that dialog opens Proof Saver's own settings.

The file is not code-signed, so SmartScreen may warn the first time. The source is all in this repository under [`saver/`](../saver/).

To remove it: pick another screensaver (or None) in Screen Saver Settings, then delete `proof-screensaver.scr` and the folder `%APPDATA%\ProofSaver`.

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
proof-screensaver.scr --window        # Esc quits, F11 fullscreen, Right arrow skips
proof-screensaver.scr --fullscreen    # the mouse does not end it; Esc does
proof-screensaver.scr --scene aizawa --window
```

## Frame capture without a window

```text
proof-screensaver.scr --record frames --scene thomas --size 1080x1080 --frames 240
```

writes `frames/frame_0000.bmp` onwards without showing anything, using the engine's `PROOF_HIDDEN` and `PROOF_SHOT` capture. `proof-screensaver.scr --help` lists every option.

## Testing without a window

Started with `PROOF_HIDDEN=1` already set, the program never shows anything: `/s` renders hidden and does not end on input (stop it with a timeout, or add `PROOF_SHOT` to write frames and exit), `/c` builds the settings dialog hidden and closes it straight away, and `/p <hwnd>` draws inside the given window, so a hidden parent keeps it invisible. `/p` with a handle that is not a window exits at once with code 0.

```bash
PROOF_HIDDEN=1 PROOF_SHOT='frames/s_{n}.bmp' PROOF_SHOT_AT=60 PROOF_SHOT_COUNT=3 proof-screensaver.scr /s
proof-screensaver.scr --preview-test preview-frames
```

## Build it

```bash
cd proof-engine/saver
cargo build --release
copy target\release\proof-saver.exe proof-screensaver.scr
```

A `.scr` file is an ordinary Windows program with a different extension. It implements the screensaver arguments: `/s` runs fullscreen until a key, a click or a mouse movement of more than 12 pixels; `/p <hwnd>` draws into the preview box by making its window a child of the one Windows passes, and exits when that window goes away (or at once if it cannot become its child); `/c` and no arguments open the settings dialog.
