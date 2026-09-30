# Install and run

[README](../README.md) · [Install](INSTALL.md) · [Demos](DEMOS.md) · [How it works](ARCHITECTURE.md) · [Capture](CAPTURE.md) · [Editor](EDITOR.md)

## Just watch it run (no Rust needed)

Every [release](https://gitlab.com/mattbusel/proof-engine/-/releases) carries prebuilt demos and the Proof Editor. The links below always point at the newest one.

| System | Download | Then |
| --- | --- | --- |
| Windows 10/11 (64-bit) | [proof-engine-demos-windows-x64.zip](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-engine-demos-windows-x64.zip) | Unzip, double-click `proof-lorenz.exe` (or any other `proof-*.exe`). |
| Windows, one file | [proof-lorenz.exe](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-lorenz.exe) · [proof-galaxy.exe](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-galaxy.exe) · [proof-sky.exe](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-sky.exe) · [proof-strange_attractors.exe](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-strange_attractors.exe) · [proof-editor.exe](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-editor.exe) | Double-click it. |
| macOS, Apple silicon | [proof-engine-demos-macos-arm64.tar.gz](https://gitlab.com/mattbusel/proof-engine/-/releases) | See the macOS note below. |
| macOS, Intel | [proof-engine-demos-macos-x64.tar.gz](https://gitlab.com/mattbusel/proof-engine/-/releases) | See the macOS note below. |
| Linux x86_64 | [proof-engine-demos-linux-x64.tar.gz](https://gitlab.com/mattbusel/proof-engine/-/releases) | `tar xzf` it and run `./proof-lorenz`. Needs ALSA (`libasound2`), present on most desktops. |

The archive holds `proof-sky`, `proof-lorenz`, `proof-galaxy`, `proof-strange_attractors`, `proof-math_rain`, `proof-quickstart`, `proof-supernova`, `proof-convergence`, `proof-playground`, `proof-hello_glyph` and `proof-editor`, plus a `README.txt` saying what each one shows. [DEMOS.md](DEMOS.md) has the same list with pictures and controls. Esc or the window's close button quits.

**Windows SmartScreen** may say it "protected your PC" because the files are not code-signed. Click **More info**, then **Run anyway**.

**macOS** blocks unsigned downloads. After unpacking, run this once in Terminal from the folder you unpacked into:

```bash
xattr -dr com.apple.quarantine proof-engine-demos-macos-*
```

macOS stops at OpenGL 4.1, so `apotheosis` (which uses 4.3 compute shaders) will not run there; every other demo does. `SHA256SUMS.txt` on each release lists the checksum of every file.

## Use it in your own Rust program

| You want to | Do this |
| --- | --- |
| Use it in your own Rust program | `cargo add proof-engine` |
| Run the demos from source | `git clone https://gitlab.com/mattbusel/proof-engine && cd proof-engine && cargo run --release --example galaxy` |
| Read the API | [docs.rs/proof-engine](https://docs.rs/proof-engine) |

You need stable Rust and a GPU with OpenGL 3.3 or newer.

- **Windows:** nothing else.
- **macOS:** nothing else.
- **Linux:** the audio backend needs the ALSA headers: `sudo apt install libasound2-dev pkg-config` (Debian/Ubuntu) or `sudo dnf install alsa-lib-devel` (Fedora).

The first build compiles the whole engine and takes a few minutes. Always use `--release`; a debug build is far too slow for tens of thousands of particles a frame.

## Status

Early (0.2) and moving fast. The public API is not stable and some subsystems are further along than others. CI builds every target and runs the unit, integration and doc tests on Linux, and builds the examples on Windows and macOS. About 4,800 library unit tests pass; the ones that do not yet are listed by name in [`ci/known-failing-tests.txt`](../ci/known-failing-tests.txt), and each one fixed is a line deleted from that file. Contributions: see [CONTRIBUTING.md](../CONTRIBUTING.md).
