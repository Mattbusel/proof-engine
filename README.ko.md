<h1 align="center">Proof Engine</h1>

<p align="center"><a href="README.md">English</a> | <a href="README.zh-CN.md">简体中文</a> | <a href="README.ja.md">日本語</a> | 한국어</p>

<p align="center"><b>방정식을 쓰면 Proof Engine이 그 움직임을 그려 줍니다. 수학으로 만든 움직이는 그림을, 빛나는 HDR 조명으로, 실시간으로.</b></p>

<p align="center">Rust로 만든 그래픽 엔진과 바로 실행해 볼 수 있는 데모 모음입니다. 크리에이티브 코더, 제너러티브 아트 작가, 그리고 이상한 끌개(스트레인지 어트랙터)나 물리 기반 하늘이 움직이는 모습을 보고 싶은 모든 분을 위해 만들었습니다.</p>

<p align="center">
  <a href="https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-lorenz.exe"><b>Windows용 다운로드 (.exe)</b></a> &nbsp;&middot;&nbsp;
  <a href="#download">macOS 및 Linux</a> &nbsp;&middot;&nbsp;
  <a href="https://proof-engine-rs.vercel.app/">사이트</a> &nbsp;&middot;&nbsp;
  <a href="https://docs.rs/proof-engine">docs.rs</a> &nbsp;&middot;&nbsp;
  <a href="https://crates.io/crates/proof-engine"><img src="https://img.shields.io/crates/v/proof-engine.svg" alt="crates.io 버전" align="center"></a>
</p>

<p align="center"><img src="assets/gifs/galaxy.gif" width="100%" alt="galaxy 예제 실행 화면. 약 3,000개의 글리프가 네 개의 나선팔 위에서 각자 원형 궤도를 돌고 있다. 중심은 뜨겁게 빛나고 바깥쪽 팔은 어두운 붉은색이며, 카메라가 주위를 돌며 비스듬히 내려다본다."></p>
<p align="center"><sub><code>galaxy</code> 데모. 엔진 자체의 프레임버퍼에서 녹화했습니다.</sub></p>

Proof Engine은 Rust와 OpenGL로 작성한 실시간 제너러티브 아트 및 수학 시각화 엔진입니다. 점과 글리프를 미분방정식(로렌츠 어트랙터와 그 밖의 여섯 가지 스트레인지 어트랙터), 힘의 장(force field), 물리 시뮬레이션으로 움직인 다음, 블룸과 ACES 톤 매핑을 갖춘 반정밀도(half-float) HDR 파이프라인으로 렌더링합니다.

<a id="download"></a>
## 다운로드

| 운영체제 | 다운로드 |
| --- | --- |
| **Windows** | [**proof-lorenz.exe**](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-lorenz.exe): 더블클릭하면 됩니다. 또는 [데모 10종 전체와 에디터 (.zip)](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-engine-demos-windows-x64.zip). |
| **macOS** (Apple 실리콘 / Intel) | [arm64용 데모](https://gitlab.com/mattbusel/proof-engine/-/releases) / [x64용 데모](https://gitlab.com/mattbusel/proof-engine/-/releases) |
| **Linux** (x86_64) | [proof-engine-demos-linux-x64.tar.gz](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-engine-demos-linux-x64.tar.gz) |
| **직접 만드는 Rust 프로그램** | `cargo add proof-engine` |

다운로드 버전은 Rust 없이 실행됩니다. OpenGL 3.3 이상을 지원하는 GPU만 있으면 됩니다. 서명되지 않은 파일이므로 Windows에서는 *추가 정보*를 누른 다음 *실행*을 누르세요. macOS와 Linux 관련 안내는 [docs/INSTALL.md](docs/INSTALL.md)에 있습니다.

## 수학 화면 보호기

엔진에 들어 있는 스트레인지 어트랙터 중 열 가지를 Windows 화면 보호기로 하나씩 차례로 보여 줍니다. 모든 점이 방정식에 따라 실시간으로 움직입니다. [**proof-screensaver.scr**](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-screensaver.scr)을 내려받아 계속 둘 수 있는 곳에 놓은 뒤, 마우스 오른쪽 버튼으로 클릭해 **설치**를 선택하세요(또는 `C:\Windows\System32`에 복사한 다음 화면 보호기 설정에서 *proof-screensaver*를 고르세요). 그 화면의 **설정...** 버튼을 누르면 자체 옵션 창이 열립니다. 제거 방법을 포함한 자세한 내용: [docs/SCREENSAVER.md](docs/SCREENSAVER.md)

## 작동 원리

<img src="docs/img/how-math-becomes-a-frame.svg" width="100%" alt="lorenz 예제의 한 프레임을 예로, 수학이 어떻게 프레임이 되는지 보여 주는 애니메이션 다이어그램. 1 수학: CPU에서 RK4로 40,000개의 로렌츠 상태를 한 단계씩 진행한다. 2 글리프: 각 상태가 글리프 인스턴스 하나가 되며, 위치는 x와 z로 정하고 색은 속도로 정한 뒤, 모두 한 번의 인스턴스드 드로우 콜로 그린다. 3 빛: 반정밀도 HDR 버퍼에서 겹친 부분은 1.0을 넘어서까지 더해지고, 잔상은 이전 프레임의 55퍼센트를 유지하며, 블룸이 빛 번짐을 흐리게 퍼뜨린다. 4 프레임: 노출, ACES 톤 매핑, 비네트를 적용한 뒤, HUD 텍스트를 그 위에 선명하게 그린다.">

매 프레임마다 여러분의 코드가 평범한 수학으로 점을 움직이고, 엔진이 각 점을 글리프로 바꾸고, GPU가 그 빛을 HDR 버퍼에 더하고, 마지막 합성 단계가 그 빛을 그림으로 만듭니다. 아래는 같은 실제 프레임을 각 단계 직후에 캡처한 것입니다.

<img src="docs/img/frame-stages.jpg" width="100%" alt="엔진이 같은 프레임을 세 가지 방식으로 캡처한 lorenz 예제. 글리프 패스만 있으면 거친 점으로 된 나비 모양. 잔상과 블룸을 더하면 고리가 채워지며 빛난다. 톤 매핑과 색 보정을 더하면 검은색은 더 깊어지고 하이라이트는 더 따뜻해진다.">

## 예제

모두 `cargo run --release --example <name>`(또는 그에 맞는 `proof-<name>.exe`)을 실제로 캡처한 것입니다.

| `lorenz` | `strange_attractors` |
| --- | --- |
| <img src="assets/gifs/lorenz.gif" width="100%" alt="로렌츠 어트랙터의 두 날개를 도는 40,000개의 점. 느린 곳은 청록색, 빠른 곳은 호박색."> | <img src="assets/gifs/strange_attractors.gif" width="100%" alt="Lorenz, Rossler, Chen, Halvorsen, Aizawa, Thomas, Dadras 일곱 가지 스트레인지 어트랙터가 각각 1,500개의 점으로 천천히 회전한다."> |
| 로렌츠 방정식을 타고 움직이는 40,000개의 점. 날개는 아무도 그리지 않았습니다. 방정식이 점을 그곳에 놓았을 뿐입니다. | 일곱 개의 카오스 계를 나란히, 각각 1,500개의 점으로. 색은 속도를 나타냅니다. |
| **`sky`** | **`math_rain`** |
| <img src="assets/gifs/sky.gif" width="100%" alt="늦은 오후의 sky 예제. 파란 하늘이 해 근처에서 흰색으로 바뀌고, 아래로 갈색 산이 보인다."> | <img src="assets/gifs/math_rain.gif" width="100%" alt="초록색 수학 기호 백 줄이 서로 다른 속도로 떨어진다. 각 줄 맨 앞의 글리프는 밝은 흰빛 초록색."> |
| 하루가 지나가는 모습. 하늘의 모든 셀이 매 프레임 레일리 산란과 미 산란 적분입니다. | 떨어지는 수학 기호. 속도는 사인 함수에서, 깜빡임은 로지스틱 사상에서 나옵니다. |

데모 21종 전체와 조작법: [docs/DEMOS.md](docs/DEMOS.md)

## 3단계로 시작하기

**1. 프로젝트를 만들고 엔진을 추가합니다.**

```bash
cargo new lorenz-demo && cd lorenz-demo
cargo add proof-engine
```

**2. `src/main.rs`를 아래 코드로 바꿉니다**(저장소에도 [`examples/quickstart.rs`](examples/quickstart.rs)로 들어 있습니다).

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

**3. `cargo run --release`로 실행합니다.** 점들은 10초쯤 한 줄기로 뭉쳐서 함께 나아가다가, 카오스에 의해 흩어지며 로렌츠 나비 모양이 됩니다.

<img src="assets/quickstart-steps.jpg" width="100%" alt="quickstart 프로그램의 실제 프레임 네 장. 1초에는 아주 작은 점, 13.5초에는 가는 호, 16초에는 점들이 여러 고리로 갈라지고, 18.5초에는 로렌츠 나비의 두 날개를 가득 채운다.">

Linux에서는 먼저 ALSA 헤더가 필요합니다(`sudo apt install libasound2-dev pkg-config`). 빌드는 항상 `--release`로 하세요.

## 창을 띄우지 않고도 할 수 있는 것들

**어떤 데모든 GIF로 녹화할 수 있습니다.** 화면 녹화 프로그램이 필요 없습니다. 엔진이 자기 프레임을 다시 읽어 직접 GIF를 씁니다. 그동안 창은 숨겨진 상태로 유지됩니다.

```bash
PROOF_HIDDEN=1 PROOF_FIXED_DT=30 PROOF_SHOT=lorenz.gif PROOF_SHOT_AT=300 \
PROOF_SHOT_COUNT=120 PROOF_SHOT_WIDTH=480 cargo run --release --example lorenz
```

사진 한 장만 필요하면 대신 `.png`나 `.jpg`를 쓰세요. 모든 설정은 [docs/CAPTURE.md](docs/CAPTURE.md)에 있습니다.

**다시 컴파일하지 않고 직접 만든 방정식을 시험해 볼 수 있습니다.** 작은 스크립트 언어인 [Rhai](https://rhai.rs)로 몇 줄만 쓰면 됩니다. 아래가 Thomas 어트랙터 전체입니다.

```text
let b = 0.208186;
[sin(y) - b * x, sin(z) - b * y, sin(x) - b * z]
```

그런 다음 이를 그리고, 파일을 저장할 때마다 다시 그리게 합니다.

```bash
cargo run --release --example scripted_attractor -- examples/scripts/thomas.rhai thomas.png --dt 0.05 --map turbo --watch
```

오타가 있는 채로 저장하면 어디가 틀렸는지 알려 주고, 마지막으로 제대로 그려진 그림은 그대로 유지합니다. 직접 만드는 프로그램에서는 `math::scripted::ScriptedSystem`이 프레임 루프 안에서 같은 일을 합니다.

**제대로 된 컬러맵으로 색을 입힐 수 있습니다.** `math::color::preset_gradient("viridis")`로 표준 컬러맵 38종(viridis, magma, turbo, cubehelix, ColorBrewer 세트) 중 무엇이든 쓸 수 있고, `Gradient::from_css("#000, deeppink 40%, gold")`에는 CSS 그라디언트를 그대로 넣을 수 있습니다.

**사운드를 파일로 렌더링할 수 있습니다.** `audio::OfflineRenderer`는 사운드 카드 없이 엔진의 신시사이저를 돌리고, `audio::wav::write_wav`가 그 결과를 저장합니다. `cargo run --release --example audio_bounce`를 실행해 보세요.

**모든 코어를 활용할 수 있습니다.** `math::attractors::rk4_step_all(kind, &mut points, dt)`는 슬라이스 전체를 한 번에 진행합니다. `parallel` 기능을 켜면 rayon으로 모든 코어에서 실행됩니다. i7-13700KF에서 40,000개의 로렌츠 점은 코어 하나로 프레임당 1.21 ms, `parallel`을 켜면 0.10 ms가 걸립니다(`cargo bench --bench attractor_bench`). 어느 쪽이든 결과는 비트 단위까지 동일합니다.

## Cargo 기능(features)

| 기능 | 기본값 | 추가되는 것 |
| --- | --- | --- |
| `rhai-scripts` | 켜짐 | `math::scripted`: Rhai로 작성한 계, 핫 리로드 지원 |
| `parallel` | 꺼짐 | 멀티코어 `rk4_step_all`(rayon) |
| `http` | 꺼짐 | `networking::http`, 리더보드, 애널리틱스 클라이언트의 실제 요청(ureq, rustls) |
| `websocket` | 꺼짐 | `networking::websocket`의 실제 `ws://` / `wss://` 연결(tungstenite, rustls) |
| `net` | 꺼짐 | `http` + `websocket` |

`http` / `websocket`이 없으면 네트워킹 클라이언트는 네트워크에 접근하지 않고 오류를 보고합니다. OpenSSL을 끌어오는 기능은 하나도 없습니다.

## 다른 라이브러리와 비교

- **[nannou](https://crates.io/crates/nannou)는** 범용 크리에이티브 코딩 프레임워크입니다(wgpu, 드로잉 API, 오디오, OSC). 일반적인 스케치에는 이쪽을 고르세요.
- **[macroquad](https://crates.io/crates/macroquad)는** 웹에서도 돌아가는 작은 게임 라이브러리입니다. 2D 게임, 특히 브라우저 게임이라면 이쪽을 고르세요.
- **Proof Engine**은 범위가 더 좁습니다. 장면은 방정식으로 움직이는 글리프와 파티클로 이루어지고 HDR 블룸 파이프라인으로 그려지며, 어트랙터, 힘의 장, 컬러맵, 스크립트 계, 프레임 캡처가 기본으로 들어 있습니다. 방정식 자체는 테스트 스위트에서 [ode_solvers](https://crates.io/crates/ode_solvers) 크레이트와 대조해 검증합니다(Lorenz, Rossler, Thomas, Aizawa, Chen을 각각 독립적으로 작성).

## 문서

| 문서 | 내용 |
| --- | --- |
| [설치와 실행](docs/INSTALL.md) | 모든 다운로드, macOS/Linux 안내, 소스에서 빌드하기, 프로젝트 현황 |
| [데모](docs/DEMOS.md) | 예제 21종 전체의 그림과 조작법, sky와 Lorenz 해설 |
| [작동 원리](docs/ARCHITECTURE.md) | 프레임 파이프라인, `engine.fx`, 조명, GPU 밀도, 사운드, 각 모듈의 내용 |
| [프레임 캡처](docs/CAPTURE.md) | `PROOF_SHOT`과 `PROOF_HIDDEN`: 창을 띄우지 않고 어떤 프로그램의 프레임이든 녹화 |
| [Proof Editor](docs/EDITOR.md) | 장면 에디터, 다운로드와 단축키 |
| [화면 보호기](docs/SCREENSAVER.md) | Proof Saver: 설치, 설정, 라이브 아트 모드, .scr 빌드 |
| [docs.rs](https://docs.rs/proof-engine) | 전체 API |
| [변경 내역](CHANGELOG.md) · [기여 안내](CONTRIBUTING.md) | 릴리스 기록과 변경 사항을 보내는 방법 |

[chaos-rpg](https://gitlab.com/mattbusel/chaos-rpg)는 그래픽 프런트엔드가 Proof Engine 위에서 돌아가는 로그라이크 게임입니다. 이 게임이 필요로 하는 것은 [`CHAOS_RPG_API_CONTRACT.md`](CHAOS_RPG_API_CONTRACT.md)에 정리되어 있습니다.

## 사용한 오픈소스

창과 OpenGL은 [winit](https://crates.io/crates/winit), [glutin](https://crates.io/crates/glutin), [glow](https://crates.io/crates/glow), 수학은 [glam](https://crates.io/crates/glam), 텍스트는 [ab_glyph](https://crates.io/crates/ab_glyph), 사운드 출력은 [cpal](https://crates.io/crates/cpal)을 사용합니다. 이미지 파일과 GIF는 [image](https://crates.io/crates/image), WAV 파일은 [hound](https://crates.io/crates/hound), 컬러맵은 [colorgrad](https://crates.io/crates/colorgrad), 스크립트는 [rhai](https://crates.io/crates/rhai), 저장 파일은 [serde_json](https://crates.io/crates/serde_json)이 처리합니다.

## 라이선스

MIT. [LICENSE](LICENSE)를 참고하세요.

## 개발자에게 의뢰하기

**여러분의 제품에도 이런 엔지니어링이 필요하신가요?** 소수의 클라이언트 프로젝트를 맡고 있습니다. LLM 기능, iOS 앱, 성능 개선 작업을 고정 가격으로 진행합니다. [서비스와 가격](https://mattbusel.vercel.app/) · [이메일](mailto:mattbusel@gmail.com) · [LinkedIn](https://www.linkedin.com/in/matthewbusel/)
