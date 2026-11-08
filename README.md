# Proof Engine

English | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md)

[![crates.io](https://img.shields.io/crates/v/proof-engine.svg)](https://crates.io/crates/proof-engine) [![CI](https://github.com/Mattbusel/proof-engine/actions/workflows/ci.yml/badge.svg)](https://github.com/Mattbusel/proof-engine/actions/workflows/ci.yml)

Rust engine that draws math in real time.

40,000 Lorenz points in 1.21 ms per frame on one core.

<img src="assets/gifs/galaxy.gif" width="100%" alt="The galaxy demo: about 3,000 glyphs on four spiral arms with a hot core.">

- Moves points by equations: Lorenz and six other attractors.
- Draws them with HDR glow.
- Records a GIF of any demo.
- Ships 21 demos.
- Runs [O'BLOCK](https://store.steampowered.com/app/5357680/?utm_source=github&utm_medium=readme&utm_campaign=proof-engine), free on Steam.

## Install

```bash
cargo add proof-engine
```

No Rust? [Download for Windows (.exe)](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-lorenz.exe).

## Use

```bash
cargo run --release --example lorenz
```

[Site](https://proof-engine-rs.vercel.app/) · [docs.rs](https://docs.rs/proof-engine) · [Demos](docs/DEMOS.md) · [Install notes](docs/INSTALL.md)

License: MIT, see [LICENSE](LICENSE).

More: [docs/DETAILS.md](docs/DETAILS.md)
