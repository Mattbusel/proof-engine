<h1 align="center">Proof Engine</h1>

<p align="center"><a href="README.md">English</a> | <a href="README.zh-CN.md">简体中文</a> | 日本語 | <a href="README.ko.md">한국어</a></p>

<p align="center"><b>数式を書けば、その動きを Proof Engine が描き出します。数学から生まれる動く映像を、光り輝く HDR ライティングでリアルタイムに。</b></p>

<p align="center">Rust 製のグラフィックスエンジンと、すぐに動かせるデモのセットです。クリエイティブコーダー、ジェネラティブアートの作家、そしてストレンジアトラクタや物理ベースの空が動く様子を眺めてみたいすべての人へ。</p>

<p align="center">
  <a href="https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-lorenz.exe"><b>Windows 版をダウンロード (.exe)</b></a> &nbsp;&middot;&nbsp;
  <a href="#download">macOS / Linux</a> &nbsp;&middot;&nbsp;
  <a href="https://proof-engine-rs.vercel.app/">公式サイト</a> &nbsp;&middot;&nbsp;
  <a href="https://docs.rs/proof-engine">docs.rs</a> &nbsp;&middot;&nbsp;
  <a href="https://crates.io/crates/proof-engine"><img src="https://img.shields.io/crates/v/proof-engine.svg" alt="crates.io のバージョン" align="center"></a>
</p>

<p align="center"><img src="assets/gifs/galaxy.gif" width="100%" alt="galaxy サンプルの実行画面。約 3,000 個のグリフが 4 本の渦状腕に並び、それぞれが独自の円軌道を回っている。中心は高温に輝き、外側の腕は暗い赤。カメラが周回しながら斜めから見下ろしている。"></p>
<p align="center"><sub><code>galaxy</code> デモ。エンジン自身のフレームバッファから録画しています。</sub></p>

Proof Engine は、Rust と OpenGL で書かれたリアルタイムのジェネラティブアート／数学ビジュアライゼーションエンジンです。点やグリフを微分方程式（ローレンツアトラクタとほか 6 種類のストレンジアトラクタ）、力場、物理演算で動かし、ブルームと ACES トーンマッピングを備えた半精度浮動小数点の HDR パイプラインで描画します。

<a id="download"></a>
## ダウンロード

| OS | ダウンロード |
| --- | --- |
| **Windows** | [**proof-lorenz.exe**](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-lorenz.exe)：ダブルクリックするだけ。または [全 10 デモとエディタ (.zip)](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-engine-demos-windows-x64.zip)。 |
| **macOS**（Apple シリコン / Intel） | [arm64 版デモ](https://gitlab.com/mattbusel/proof-engine/-/releases) / [x64 版デモ](https://gitlab.com/mattbusel/proof-engine/-/releases) |
| **Linux** (x86_64) | [proof-engine-demos-linux-x64.tar.gz](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-engine-demos-linux-x64.tar.gz) |
| **自分の Rust プログラムで使う** | `cargo add proof-engine` |

ダウンロード版の実行に Rust は不要です。OpenGL 3.3 以降に対応した GPU があれば動きます。ファイルは署名されていません。Windows では *詳細情報* をクリックしてから *実行* を選んでください。macOS と Linux での注意点は [docs/INSTALL.md](docs/INSTALL.md) にあります。

## 数学スクリーンセーバー

エンジンに収録されたストレンジアトラクタのうち 10 種類を、Windows のスクリーンセーバーとして次々に表示します。すべての点がその場で方程式に従って動きます。[**proof-screensaver.scr**](https://gitlab.com/mattbusel/proof-engine/-/releases/permalink/latest/downloads/proof-screensaver.scr) をダウンロードして消さない場所に置き、右クリックして **インストール** を選んでください（または `C:\Windows\System32` にコピーし、スクリーンセーバーの設定で *proof-screensaver* を選択）。そこで **設定...** を押すと専用のオプション画面が開きます。アンインストール方法など詳しくは [docs/SCREENSAVER.md](docs/SCREENSAVER.md) へ。

## 仕組み

<img src="docs/img/how-math-becomes-a-frame.svg" width="100%" alt="lorenz サンプルの 1 フレームを例に、数学がどのようにフレームになるかを示すアニメーション図。1 数学：CPU 上で RK4 により 40,000 個のローレンツ状態を進める。2 グリフ：各状態が 1 つのグリフインスタンスになり、x と z から位置を決め、速度で色を付け、すべてを 1 回のインスタンス描画で描く。3 光：半精度浮動小数点の HDR バッファで、重なった部分は 1.0 を超えて加算される。トレイルは前フレームの 55 パーセントを残し、ブルームが輝きをぼかす。4 フレーム：露出、ACES トーンマッピング、ビネットをかけ、最後に HUD テキストをくっきりと上に描く。">

毎フレームの流れはこうです。あなたのコードが普通の数式で点を動かし、エンジンが各点をグリフに変え、GPU がその光を HDR バッファに積み上げ、最後のコンポジットがその光を絵に仕上げます。以下は、同じ実フレームを各ステージの後でキャプチャしたものです。

<img src="docs/img/frame-stages.jpg" width="100%" alt="エンジンが同じフレームを 3 通りにキャプチャした lorenz サンプル。グリフパスだけだと、ざらついた点の蝶。トレイルとブルームを加えるとループが埋まって光り出す。トーンマッピングとカラーグレーディングを加えると、黒はより深く、ハイライトはより暖かくなる。">

## サンプル

どれも `cargo run --release --example <name>`（または対応する `proof-<name>.exe`）を実際にキャプチャしたものです。

| `lorenz` | `strange_attractors` |
| --- | --- |
| <img src="assets/gifs/lorenz.gif" width="100%" alt="ローレンツアトラクタの 2 枚の翼を周回する 40,000 個の点。遅い所はティール、速い所はアンバー。"> | <img src="assets/gifs/strange_attractors.gif" width="100%" alt="Lorenz、Rossler、Chen、Halvorsen、Aizawa、Thomas、Dadras の 7 種類のストレンジアトラクタが、それぞれ 1,500 個の点でゆっくり回転している。"> |
| ローレンツ方程式に乗って動く 40,000 個の点。翼は誰も描いていません。方程式が点をそこへ運んだのです。 | 7 つのカオス系を並べて表示、各 1,500 点。色は速度を表します。 |
| **`sky`** | **`math_rain`** |
| <img src="assets/gifs/sky.gif" width="100%" alt="夕方の sky サンプル。青空が太陽の近くで白へと移り変わり、その下に茶色の山並み。"> | <img src="assets/gifs/math_rain.gif" width="100%" alt="緑色の数学記号が 100 列、それぞれ異なる速さで落ちていく。各列の先頭のグリフは明るい白緑色。"> |
| 一日が過ぎていく様子。空のセル 1 つ 1 つが、毎フレーム、レイリー散乱とミー散乱の積分です。 | 降り注ぐ数学記号。速度は正弦関数から、ちらつきはロジスティック写像から。 |

全 21 デモと操作方法：[docs/DEMOS.md](docs/DEMOS.md)

## 3 ステップで使ってみる

**1. プロジェクトを作ってエンジンを追加します。**

```bash
cargo new lorenz-demo && cd lorenz-demo
cargo add proof-engine
```

**2. `src/main.rs` を次の内容に置き換えます**（リポジトリにも [`examples/quickstart.rs`](examples/quickstart.rs) として入っています）。

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

**3. `cargo run --release` で実行します。** 点は 10 秒ほど 1 本の筋としてまとまって進み、やがてカオスによって引き離されて、ローレンツの蝶の形になります。

<img src="assets/quickstart-steps.jpg" width="100%" alt="quickstart プログラムの実際の 4 フレーム。1 秒では小さな点。13.5 秒では細い弧。16 秒では点がいくつものループに分かれ、18.5 秒ではローレンツの蝶の両翼を埋め尽くしている。">

Linux では先に ALSA のヘッダーが必要です（`sudo apt install libasound2-dev pkg-config`）。ビルドは必ず `--release` で行ってください。

## ウィンドウを開かずにできること

**どのデモでも GIF を録画できます。** 画面録画ソフトは不要です。エンジンが自分のフレームを読み戻し、GIF を直接書き出します。その間ウィンドウは非表示のままです。

```bash
PROOF_HIDDEN=1 PROOF_FIXED_DT=30 PROOF_SHOT=lorenz.gif PROOF_SHOT_AT=300 \
PROOF_SHOT_COUNT=120 PROOF_SHOT_WIDTH=480 cargo run --release --example lorenz
```

静止画 1 枚なら `.png` か `.jpg` を指定してください。設定項目はすべて [docs/CAPTURE.md](docs/CAPTURE.md) にあります。

**再コンパイルなしで自分の方程式を試せます。** 小さなスクリプト言語 [Rhai](https://rhai.rs) で、系を数行で書くだけです。次が Thomas アトラクタの全体です。

```text
let b = 0.208186;
[sin(y) - b * x, sin(z) - b * y, sin(x) - b * z]
```

あとはこれを描画し、ファイルを保存するたびに再描画させます。

```bash
cargo run --release --example scripted_attractor -- examples/scripts/thomas.rhai thomas.png --dt 0.05 --map turbo --watch
```

タイプミスしたまま保存しても、エラーの場所を教えてくれて、最後にうまく描けた絵はそのまま残ります。自分のプログラムの中では、`math::scripted::ScriptedSystem` がフレームループ内で同じことをします。

**本物のカラーマップで色付けできます。** `math::color::preset_gradient("viridis")` で 38 種類の標準カラーマップ（viridis、magma、turbo、cubehelix、ColorBrewer の各セット）のどれでも使え、`Gradient::from_css("#000, deeppink 40%, gold")` には CSS のグラデーションをそのまま渡せます。

**サウンドをファイルにレンダリングできます。** `audio::OfflineRenderer` はサウンドカードなしでエンジンのシンセサイザーを動かし、`audio::wav::write_wav` で保存します。`cargo run --release --example audio_bounce` を試してみてください。

**全コアを使えます。** `math::attractors::rk4_step_all(kind, &mut points, dt)` はスライス全体を一度に進めます。`parallel` フィーチャーを有効にすると rayon で全コアを使って動きます。i7-13700KF では、40,000 個のローレンツ点の計算が 1 コアで 1 フレームあたり 1.21 ms、`parallel` 有効時は 0.10 ms です（`cargo bench --bench attractor_bench`）。どちらでも結果はビット単位で一致します。

## Cargo フィーチャー

| フィーチャー | デフォルト | 追加されるもの |
| --- | --- | --- |
| `rhai-scripts` | オン | `math::scripted`：Rhai で書いた系をホットリロード |
| `parallel` | オフ | マルチコア版 `rk4_step_all`（rayon） |
| `http` | オフ | `networking::http`、リーダーボード、アナリティクスの各クライアントで実際の通信を行う（ureq、rustls） |
| `websocket` | オフ | `networking::websocket` で実際の `ws://` / `wss://` 接続を行う（tungstenite、rustls） |
| `net` | オフ | `http` + `websocket` |

`http` / `websocket` がない場合、ネットワーククライアントはネットワークに触れずにエラーを返します。OpenSSL を引き込むフィーチャーはありません。

## ほかのライブラリとの比較

- **[nannou](https://crates.io/crates/nannou)** は汎用のクリエイティブコーディングフレームワークです（wgpu、描画 API、オーディオ、OSC）。一般的なスケッチならこちら。
- **[macroquad](https://crates.io/crates/macroquad)** は Web でも動く小さなゲームライブラリです。2D ゲーム、特にブラウザ向けならこちら。
- **Proof Engine** はもっと用途を絞っています。シーンは方程式で動くグリフとパーティクルで構成され、HDR ブルームパイプラインで描かれます。アトラクタ、力場、カラーマップ、スクリプトによる系、フレームキャプチャを最初から備えています。方程式そのものは、テストスイートで [ode_solvers](https://crates.io/crates/ode_solvers) クレートと突き合わせて検証しています（Lorenz、Rossler、Thomas、Aizawa、Chen を、それぞれ独立に書き下したもの）。

## ドキュメント

| ドキュメント | 内容 |
| --- | --- |
| [インストールと実行](docs/INSTALL.md) | すべてのダウンロード、macOS/Linux での注意点、ソースからのビルド、プロジェクトの状況 |
| [デモ](docs/DEMOS.md) | 全 21 サンプルの画像と操作方法、sky と Lorenz の解説 |
| [仕組み](docs/ARCHITECTURE.md) | フレームパイプライン、`engine.fx`、ライト、GPU 密度、サウンド、各モジュールの中身 |
| [フレームキャプチャ](docs/CAPTURE.md) | `PROOF_SHOT` と `PROOF_HIDDEN`：ウィンドウを出さずに任意のプログラムのフレームを録画 |
| [Proof Editor](docs/EDITOR.md) | シーンエディタ、そのダウンロードとキー操作 |
| [スクリーンセーバー](docs/SCREENSAVER.md) | Proof Saver：インストール、設定、ライブアートモード、.scr のビルド |
| [docs.rs](https://docs.rs/proof-engine) | API の全容 |
| [変更履歴](CHANGELOG.md) · [コントリビュート](CONTRIBUTING.md) | リリース履歴と、変更を送る方法 |

[chaos-rpg](https://gitlab.com/mattbusel/chaos-rpg) は、グラフィカルなフロントエンドが Proof Engine 上で動くローグライクゲームです。それが必要とするものは [`CHAOS_RPG_API_CONTRACT.md`](CHAOS_RPG_API_CONTRACT.md) にまとめています。

## 使用しているライブラリ

ウィンドウと OpenGL は [winit](https://crates.io/crates/winit)、[glutin](https://crates.io/crates/glutin)、[glow](https://crates.io/crates/glow)、数学は [glam](https://crates.io/crates/glam)、テキストは [ab_glyph](https://crates.io/crates/ab_glyph)、サウンド出力は [cpal](https://crates.io/crates/cpal) を使っています。画像ファイルと GIF は [image](https://crates.io/crates/image)、WAV ファイルは [hound](https://crates.io/crates/hound)、カラーマップは [colorgrad](https://crates.io/crates/colorgrad)、スクリプトは [rhai](https://crates.io/crates/rhai)、セーブファイルは [serde_json](https://crates.io/crates/serde_json) で扱っています。

## ライセンス

MIT。[LICENSE](LICENSE) を参照してください。

## 作者に依頼する

**あなたのプロダクトにもこうしたエンジニアリングが必要ですか？** 少数のクライアント案件をお受けしています。LLM 機能、iOS アプリ、パフォーマンス改善を固定価格で。[サービスと料金](https://mattbusel.vercel.app/) · [メール](mailto:mattbusel@gmail.com) · [LinkedIn](https://www.linkedin.com/in/matthewbusel/)
