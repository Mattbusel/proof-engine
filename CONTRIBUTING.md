# Contributing to Proof Engine

Thank you for your interest in contributing to Proof Engine! This document provides guidelines and instructions for contributing.

## Getting Started

1. **Fork** the repository on GitHub
2. **Clone** your fork locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/proof-engine.git
   cd proof-engine
   ```
3. **Build** the project:
   ```bash
   cargo check
   cargo test
   ```

## Development Setup

- **Rust**: Stable toolchain (edition 2021)
- **Dependencies**: OpenGL 3.3+, system audio (via cpal/rodio)
- **IDE**: Any editor with rust-analyzer support recommended

## How to Contribute

### Reporting Bugs

- Open an issue on GitHub with a clear description
- Include the Rust version (`rustc --version`) and OS
- Provide a minimal reproduction if possible

### Suggesting Features

- Open an issue with the `enhancement` label
- Describe the use case and expected behavior
- Explain how it fits the engine's mathematical rendering philosophy

### Submitting Code

1. **Create a branch** from `main`:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Write your code** following the project conventions:
   - All rendering primitives should be mathematically driven
   - Prefer `f32` math with `glam` types (`Vec2`, `Vec3`, `Vec4`, `Mat4`, `Quat`)
   - Use `std` only for new subsystems unless an external crate is truly necessary
   - Keep modules self-contained with clear public APIs

3. **Ensure it compiles cleanly**:
   ```bash
   cargo check
   cargo test
   ```

4. **Commit** with a clear message:
   ```bash
   git commit -m "Add: brief description of what was added"
   ```

5. **Push** and open a Pull Request against `main`

### Code Style

- Follow standard Rust formatting (`cargo fmt`)
- Use `snake_case` for functions and variables
- Use `CamelCase` for types and traits
- Keep functions focused and reasonably sized
- Add doc comments (`///`) for public APIs
- Avoid over-engineering: simple and direct is preferred

### Architecture

The engine is organized into self-contained modules under `src/`:

```
src/
  math/         Core mathematical functions, noise, curves, attractors
  render/       OpenGL pipeline, shaders, post-processing
  particle/     Particle systems, emitters, flocking
  entity/       Amorphous entities, formations, cohesion
  physics/      Rigid body, soft body, fluids, constraints
  scene/        Scene graph, spatial indexing, portals
  audio/        Synthesis, DSP, spatial mixing
  scripting/    Lua-like VM, compiler, stdlib
  ecs/          Entity Component System
  ui/           Widget toolkit, layout, panels
  ...and more
```

Each module should:
- Have a `mod.rs` with public re-exports
- Be independently compilable (no circular dependencies)
- Include unit tests in `#[cfg(test)]` blocks

### Testing

- Add tests for new functionality in the same file or a `tests` submodule
- Run the full test suite before submitting:
  ```bash
  cargo test
  ```
- Performance-sensitive code should have benchmarks in `benches/`

### What We're Looking For

- **New mathematical primitives**: curves, surfaces, fractals, simulations
- **Rendering improvements**: new post-processing effects, shader techniques
- **Performance**: SIMD optimizations, better algorithms, GPU compute kernels
- **Game systems**: AI, networking, procedural generation
- **Documentation**: examples, tutorials, API docs
- **Bug fixes**: correctness, edge cases, platform compatibility

### What to Avoid

- Breaking changes to public APIs without discussion
- Adding heavy external dependencies without justification
- Code that doesn't follow the mathematical rendering philosophy
- Placeholder or stub implementations (write real logic)

## Code of Conduct

Be respectful, constructive, and welcoming. We're all here to build something cool with math.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

## Questions?

Open an issue or start a discussion on the repository. We're happy to help you get started!
