<h1 align="center">Proof Engine</h1>

<p align="center"><a href="README.md">English</a> | 简体中文 | <a href="README.ja.md">日本語</a> | <a href="README.ko.md">한국어</a></p>

<p align="center"><b>写下方程，Proof Engine 就把它们的运动画出来：由数学生成的动态画面，实时渲染，带有发光的 HDR 光效。</b></p>

<p align="center">一个 Rust 图形引擎，外加一组开箱即用的演示程序。适合创意编程爱好者、生成艺术创作者，以及任何想看奇异吸引子或物理天空动起来的人。</p>

<p align="center">
  <a href="https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-lorenz.exe"><b>下载 Windows 版 (.exe)</b></a> &nbsp;&middot;&nbsp;
  <a href="#download">macOS 和 Linux</a> &nbsp;&middot;&nbsp;
  <a href="https://proof-engine-rs.vercel.app/">官网</a> &nbsp;&middot;&nbsp;
  <a href="https://docs.rs/proof-engine">docs.rs</a> &nbsp;&middot;&nbsp;
  <a href="https://crates.io/crates/proof-engine"><img src="https://img.shields.io/crates/v/proof-engine.svg" alt="crates.io 版本" align="center"></a>
</p>

<p align="center"><img src="assets/gifs/galaxy.gif" width="100%" alt="galaxy 示例运行画面：约 3,000 个字形分布在四条旋臂上，每个都沿各自的圆形轨道运行，核心炽热，外侧旋臂呈暗红色，镜头环绕时从斜角观看。"></p>
<p align="center"><sub><code>galaxy</code> 演示，直接从引擎自身的帧缓冲录制。</sub></p>

Proof Engine 是一个用 Rust 基于 OpenGL 编写的实时生成艺术与数学可视化引擎。点和字形由微分方程（洛伦兹吸引子以及另外六种奇异吸引子）、力场和物理驱动，再经过半精度浮点 HDR 渲染管线绘制，带有泛光（bloom）和 ACES 色调映射。

<a id="download"></a>
## 下载

| 系统 | 下载 |
| --- | --- |
| **Windows** | [**proof-lorenz.exe**](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-lorenz.exe)：双击即可运行。或者下载[全部 10 个演示和编辑器 (.zip)](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-engine-demos-windows-x64.zip)。 |
| **macOS**（Apple 芯片 / Intel） | [arm64 版演示](https://gitlab.com/mattbusel/proof-engine/-/releases) / [x64 版演示](https://gitlab.com/mattbusel/proof-engine/-/releases) |
| **Linux** (x86_64) | [proof-engine-demos-linux-x64.tar.gz](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-engine-demos-linux-x64.tar.gz) |
| **在你自己的 Rust 程序中使用** | `cargo add proof-engine` |

下载版无需安装 Rust，只要有支持 OpenGL 3.3 或更高版本的 GPU 即可。文件未经签名：在 Windows 上点击 *更多信息*，再点 *仍要运行*；macOS 和 Linux 的说明见 [docs/INSTALL.md](docs/INSTALL.md)。

## 数学屏保

把引擎中的十种奇异吸引子做成 Windows 屏幕保护程序，轮流播放，每个点都由方程实时驱动。下载 [**proof-screensaver.scr**](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-screensaver.scr)，放到一个不会被删除的位置，然后右键选择**安装**（或者复制到 `C:\Windows\System32`，再在屏幕保护程序设置里选择 *proof-screensaver*）。在那里点击 **设置...** 可以打开它自己的选项。更多内容（包括卸载方法）：[docs/SCREENSAVER.md](docs/SCREENSAVER.md)。

## 工作原理

<img src="docs/img/how-math-becomes-a-frame.svg" width="100%" alt="动画示意图：以 lorenz 示例的一帧为例，展示数学如何变成画面。1 数学：在 CPU 上用 RK4 推进 40,000 个洛伦兹状态。2 字形：每个状态变成一个字形实例，位置取自 x 和 z，颜色取决于速度，全部在一次实例化绘制调用中完成。3 光：半精度浮点 HDR 缓冲区，重叠处的亮度可累加超过 1.0，拖尾保留上一帧的 55%，泛光把辉光模糊开。4 帧：曝光、ACES 色调映射和暗角，最后在上方清晰地绘制 HUD 文字。">

每一帧：你的代码用普通的数学运算移动这些点，引擎把每个点变成一个字形，GPU 在 HDR 缓冲区中累加它们的光，最后合成阶段把这些光变成画面。下面是同一真实帧在每个阶段之后的截图：

<img src="docs/img/frame-stages.jpg" width="100%" alt="引擎在同一帧以三种方式截取的 lorenz 示例：仅字形渲染时，是一只颗粒感很强的点状蝴蝶；加上拖尾和泛光后，环线被填满并开始发光；加上色调映射和调色后，黑色更深，高光更暖。">

## 示例

每一个都是 `cargo run --release --example <name>`（或对应的 `proof-<name>.exe`）的真实录屏。

| `lorenz` | `strange_attractors` |
| --- | --- |
| <img src="assets/gifs/lorenz.gif" width="100%" alt="40,000 个点绕着洛伦兹吸引子的两翼旋转，慢处为青绿色，快处为琥珀色。"> | <img src="assets/gifs/strange_attractors.gif" width="100%" alt="七种奇异吸引子：Lorenz、Rossler、Chen、Halvorsen、Aizawa、Thomas 和 Dadras，每种 1,500 个点，缓慢旋转。"> |
| 40,000 个点沿洛伦兹方程运动。两翼不是谁画出来的，是方程把点放到了那里。 | 七个混沌系统并排展示，每个 1,500 个点。颜色代表速度。 |
| **`sky`** | **`math_rain`** |
| <img src="assets/gifs/sky.gif" width="100%" alt="sky 示例的傍晚时分：蓝天在太阳附近渐变为白色，下方是棕色山脉。"> | <img src="assets/gifs/math_rain.gif" width="100%" alt="一百列绿色数学符号以不同速度落下，每列最前面的字形呈明亮的白绿色。"> |
| 一天的流逝。每一帧里，天空的每个格子都是一次瑞利散射和米氏散射积分。 | 不断落下的数学符号；速度来自正弦函数，闪烁来自 logistic 映射。 |

全部 21 个演示及其操作方式：[docs/DEMOS.md](docs/DEMOS.md)。

## 三步上手

**1. 新建项目并添加引擎。**

```bash
cargo new lorenz-demo && cd lorenz-demo
cargo add proof-engine
```

**2. 用下面的代码替换 `src/main.rs`**（仓库中也有这份代码：[`examples/quickstart.rs`](examples/quickstart.rs)）：

```rust
use proof_engine::math::attractors::rk4_step;
use proof_engine::prelude::*;
use proof_engine::render::ui_layer::UiParticle;

fn main() {
    let mut engine = ProofEngine::new(EngineConfig::default());

    // 5,000 points in a line 2 units long, each 0.0004 from the next.
    let mut points: Vec<Vec3> = (0..5000)
        .map(|i| Vec3::new(1.0 + i as f32 * 4e-4, 1.0, 1.0))
        .collect();

    engine.run_ui(move |engine, dt| {
        // Advance every point along the Lorenz equations.
        for p in points.iter_mut() {
            *p = rk4_step(AttractorType::Lorenz, *p, dt);
        }
        // Draw them: x across, z up, centred in the window.
        let (w, h) = engine.render_size();
        let (cx, cy, s) = (w as f32 / 2.0, h as f32 / 2.0, h as f32 / 60.0);
        let color = Vec4::new(0.5, 1.2, 1.6, 1.0);
        let dots = points
            .iter()
            .map(|p| UiParticle::new(cx + p.x * s, cy - (p.z - 25.0) * s, 3.0, 3.0, '●', color))
            .collect();
        engine.ui.draw_particles(dots);
    });
}
```

**3. 运行** `cargo run --release`。这些点会先聚成一道光束一起前进大约十秒，然后混沌把它们拉开，形成洛伦兹蝴蝶：

<img src="assets/quickstart-steps.jpg" width="100%" alt="quickstart 程序的四个真实帧。第 1 秒是一个小点；第 13.5 秒是一条细弧；第 16 秒点已经分裂成多个环；第 18.5 秒它们填满了洛伦兹蝴蝶的两翼。">

Linux 需要先安装 ALSA 头文件（`sudo apt install libasound2-dev pkg-config`）。请始终使用 `--release` 构建。

## 无需打开窗口也能做的事

**为任意演示录制 GIF。** 不需要录屏软件：引擎会读回自己的帧，直接写出 GIF。录制期间窗口保持隐藏。

```bash
PROOF_HIDDEN=1 PROOF_FIXED_DT=30 PROOF_SHOT=lorenz.gif PROOF_SHOT_AT=300 \
PROOF_SHOT_COUNT=120 PROOF_SHOT_WIDTH=480 cargo run --release --example lorenz
```

如果只要一张图片，改用 `.png` 或 `.jpg` 即可。全部设置见 [docs/CAPTURE.md](docs/CAPTURE.md)。

**无需重新编译就能试验你自己的方程。** 用 [Rhai](https://rhai.rs)（一种小巧的脚本语言）写几行就能定义一个系统。下面就是完整的 Thomas 吸引子：

```text
let b = 0.208186;
[sin(y) - b * x, sin(z) - b * y, sin(x) - b * z]
```

然后把它画出来，并且每次保存文件时自动重绘：

```bash
cargo run --release --example scripted_attractor -- examples/scripts/thomas.rhai thomas.png --dt 0.05 --map turbo --watch
```

如果保存时有拼写错误，它会告诉你出错的位置，并保留上一张正确的画面。在你自己的程序里，`math::scripted::ScriptedSystem` 可以在帧循环中做同样的事。

**用真正的色彩映射上色。** `math::color::preset_gradient("viridis")` 提供 38 种标准色彩映射中的任意一种（viridis、magma、turbo、cubehelix、ColorBrewer 系列），`Gradient::from_css("#000, deeppink 40%, gold")` 则可以直接使用 CSS 渐变。

**把声音渲染到文件。** `audio::OfflineRenderer` 在没有声卡的情况下运行引擎的合成器，`audio::wav::write_wav` 负责保存。试试 `cargo run --release --example audio_bounce`。

**用满所有 CPU 核心。** `math::attractors::rk4_step_all(kind, &mut points, dt)` 一次推进整个切片；开启 `parallel` feature 后，它会通过 rayon 在所有核心上运行。在 i7-13700KF 上，40,000 个洛伦兹点单核每帧耗时 1.21 ms，开启 `parallel` 后为 0.10 ms（`cargo bench --bench attractor_bench`）。两种方式的结果逐位一致。

## Cargo features

| Feature | 默认 | 新增功能 |
| --- | --- | --- |
| `rhai-scripts` | 开 | `math::scripted`：用 Rhai 编写的系统，支持热重载 |
| `parallel` | 关 | 多核 `rk4_step_all`（rayon） |
| `http` | 关 | 为 `networking::http`、排行榜和数据分析客户端提供真实的网络请求（ureq、rustls） |
| `websocket` | 关 | 为 `networking::websocket` 提供真实的 `ws://` / `wss://` 连接（tungstenite、rustls） |
| `net` | 关 | `http` + `websocket` |

未开启 `http` / `websocket` 时，网络客户端会直接报错，而不会访问网络。任何 feature 都不会引入 OpenSSL。

## 与其他库的比较

- **[nannou](https://crates.io/crates/nannou)** 是一个通用的创意编程框架（wgpu、绘图 API、音频、OSC）。做一般的草图创作选它。
- **[macroquad](https://crates.io/crates/macroquad)** 是一个小巧的游戏库，也能在网页上运行。做 2D 游戏，尤其是浏览器游戏，选它。
- **Proof Engine** 的定位更聚焦：场景由方程驱动的字形和粒子组成，经过 HDR 泛光管线绘制，内置吸引子、力场、色彩映射、脚本系统和帧捕获。方程本身在测试套件中与 [ode_solvers](https://crates.io/crates/ode_solvers) crate 进行了对照校验（Lorenz、Rossler、Thomas、Aizawa 和 Chen，均为独立实现）。

## 文档

| 文档 | 内容 |
| --- | --- |
| [安装与运行](docs/INSTALL.md) | 所有下载、macOS/Linux 说明、从源码构建、项目状态 |
| [演示](docs/DEMOS.md) | 全部 21 个示例的图片和操作方式，以及 sky 和 Lorenz 的详细说明 |
| [工作原理](docs/ARCHITECTURE.md) | 帧管线、`engine.fx`、光源、GPU 密度、声音、各模块的内容 |
| [帧捕获](docs/CAPTURE.md) | `PROOF_SHOT` 和 `PROOF_HIDDEN`：录制任意程序的帧，且不弹出窗口 |
| [Proof Editor](docs/EDITOR.md) | 场景编辑器、下载和快捷键 |
| [屏保](docs/SCREENSAVER.md) | Proof Saver：安装、设置、实时艺术模式、构建 .scr |
| [docs.rs](https://docs.rs/proof-engine) | 完整 API |
| [更新日志](CHANGELOG.md) · [贡献指南](CONTRIBUTING.md) | 版本历史，以及如何提交修改 |

[chaos-rpg](https://gitlab.com/mattbusel/chaos-rpg) 是一款 Roguelike 游戏，它的图形前端运行在 Proof Engine 上；[`CHAOS_RPG_API_CONTRACT.md`](CHAOS_RPG_API_CONTRACT.md) 记录了它所需的接口。

## 依赖的开源库

窗口和 OpenGL 来自 [winit](https://crates.io/crates/winit)、[glutin](https://crates.io/crates/glutin) 和 [glow](https://crates.io/crates/glow)；数学库来自 [glam](https://crates.io/crates/glam)；文字来自 [ab_glyph](https://crates.io/crates/ab_glyph)；声音输出来自 [cpal](https://crates.io/crates/cpal)。图片文件和 GIF 由 [image](https://crates.io/crates/image) 处理，WAV 文件由 [hound](https://crates.io/crates/hound) 处理，色彩映射由 [colorgrad](https://crates.io/crates/colorgrad) 处理，脚本由 [rhai](https://crates.io/crates/rhai) 处理，存档文件由 [serde_json](https://crates.io/crates/serde_json) 处理。

## 许可证

MIT，见 [LICENSE](LICENSE)。

## 聘请作者

**你的产品需要这类工程能力吗？** 我承接少量客户项目：LLM 功能、iOS 应用和性能优化，固定报价。[服务与价格](https://mattbusel.vercel.app/) · [邮件](mailto:mattbusel@gmail.com) · [LinkedIn](https://www.linkedin.com/in/matthewbusel/)
