# pong-rs

![Rust](https://img.shields.io/badge/rust-2024_edition-orange)
![macroquad](https://img.shields.io/badge/macroquad-0.4-blue)
![License](https://img.shields.io/badge/license-MIT-green)

A classic Pong clone in a single Rust file — square ball, spin, accelerating rallies,
and a CPU that actually predicts where the ball will land. **No asset files:** every
sound effect is synthesised into a WAV in memory at startup.

<!-- Drop a screenshot at docs/screenshot.png and uncomment:
![Screenshot](docs/screenshot.png)
-->

## Features

- 🎮 **Three CPU difficulties** — Easy chases the ball, Normal and Hard predict the
  bounce point, each with its own speed, reaction time and aiming error.
- 🏓 **Angle + spin control** — where the ball meets the paddle sets the return angle
  (up to ±60°), and a moving paddle adds spin.
- ⚡ **Escalating rallies** — the ball speeds up 4.5% per hit, from 330 up to 950 px/s.
- 🧱 **Tunnel-proof physics** — sub-stepped integration, so a fast ball can never skip
  through a paddle.
- 🔊 **Procedural chiptune audio** — paddle, wall, score, win, lose and UI blips built
  from square + sine waves; missing audio never crashes the game.
- ✨ **Small touches** — fading ball trail, paddle hit flash, dashed centre line,
  serve countdown, rally counter, pause overlay.
- 🖱️ **Keyboard or mouse** control, toggled at any time with `M`.

## Quick start

```bash
git clone https://github.com/himanshmewada/pong-rs.git
cd pong-rs
cargo run --release
```

Requires **Rust 1.85+** (edition 2024). A release build is recommended.

<details>
<summary>Linux build dependencies</summary>

macroquad talks to X11/OpenGL and ALSA directly. If the build complains about missing
system libraries:

```bash
# Debian / Ubuntu
sudo apt install libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev

# Fedora
sudo dnf install libX11-devel libXi-devel mesa-libGL-devel alsa-lib-devel
```
</details>

## Controls

| Context | Key | Action |
|---|---|---|
| Menu | `←` / `→` or `A` / `D` | Cycle difficulty |
| Menu | `1` / `2` / `3` | Pick Easy / Normal / Hard directly |
| Menu | `Space` / `Enter` | Start match |
| Playing | `W` / `S` or `↑` / `↓` | Move paddle |
| Playing | `M` | Toggle mouse control |
| Playing | `Space` | Skip the serve countdown |
| Playing | `P` / `Esc` | Pause |
| Paused | `P` / `Esc` / `Space` | Resume |
| Paused | `Q` | Quit to menu |
| Game over | `Space` / `Enter` | Rematch |
| Game over | `Q` / `Esc` | Back to menu |

## Rules

- First to **11 points** wins.
- After a point the ball is served **towards whoever conceded**, following a ~1.1 s countdown.
- Each paddle hit multiplies ball speed by **1.045**, capped at **950 px/s**; speed resets on every serve.
- The `RALLY` counter in the corner tracks hits in the current exchange.

## Difficulty

| Level | CPU speed | Aim error | Reaction time | Predicts bounces? |
|---|---|---|---|---|
| Easy | 230 px/s | ±60 px | 0.25 s | ❌ chases the ball |
| Normal | 340 px/s | ±26 px | 0.12 s | ✅ |
| Hard | 430 px/s | ±9 px | 0.05 s | ✅ |

The CPU also has a 2 px dead zone so it doesn't jitter when it's already on target,
and drifts back to mid-court while the ball is travelling away.

## How it works

Everything lives in `src/main.rs`, organised into labelled sections:
`config → types → audio → game → main`.

**State machine**

```
Menu ──start──▶ Serve ──countdown/space──▶ Play ⇄ Paused
                  ▲                        │
                  └────── point scored ────┤
                                           └── 11 points ──▶ GameOver ──▶ Menu / Serve
```

**Bounce prediction.** Instead of simulating the ball forward step by step, the AI
folds the projected `y` into the court with a triangle wave:

```rust
let mut m = (pos.y + vel.y * t - lo).rem_euclid(2.0 * span);
if m > span { m = 2.0 * span - m; }
```

That gives the exact wall-bounce landing point in O(1), then difficulty-specific noise
is added so the CPU stays beatable.

**Collision.** Each frame the ball's travel is split into steps of at most 3 px
(`min(BALL_R, PADDLE_W) * 0.5`), and paddles are only tested when the ball is moving
towards them. `dt` is clamped to 50 ms so a stall or window drag can't teleport the ball.

**Audio.** `synth_wav()` renders `(frequency, duration)` note lists into a 16-bit mono
44.1 kHz PCM WAV — a 55/45 square/sine mix with a short attack and release — and hands
the bytes straight to `load_sound_from_bytes`. No `.wav` files on disk, ever.

## Tuning

All gameplay knobs are constants at the top of `src/main.rs`:

| Constant | Default | Meaning |
|---|---|---|
| `SCREEN_W` / `SCREEN_H` | 512 | Court size (fixed, non-resizable window) |
| `PADDLE_W` / `PADDLE_H` | 10 / 60 | Paddle dimensions |
| `PLAYER_SPEED` | 470 | Keyboard paddle speed (px/s) |
| `BALL_R` | 6 | Half-size of the square ball |
| `BALL_START_SPEED` | 330 | Serve speed |
| `BALL_MAX_SPEED` | 950 | Speed cap |
| `BALL_SPEEDUP` | 1.045 | Multiplier per paddle hit |
| `MAX_BOUNCE_ANGLE` | 60° | Steepest return angle |
| `TRAIL_LEN` | 10 | Trail samples |
| `WIN_SCORE` | 11 | Points to win |
| `SERVE_DELAY` | 1.1 s | Countdown between points |

Colours (`BG_COLOR`, `PADDLE_COLOR`, `BALL_COLOR`, …) live in the same block — swapping
to a dark court is a two-line change.

## Project layout

```
pong-rs/
├── Cargo.toml
├── LICENSE
├── README.md
└── src/
    └── main.rs      # config, types, synth audio, game loop, rendering
```

## Known quirks

- **Pausing during the serve countdown** resumes into `Play` with a parked (zero-velocity)
  ball, so the rally never starts. Fix: return to `Serve` when resuming if
  `ball.vel == Vec2::ZERO`, or remember the pre-pause state.
- Difficulty can only be changed from the menu.
- The window is intentionally fixed at 512×512 with `high_dpi` enabled.

## Roadmap ideas

- [ ] Local two-player mode (`W`/`S` vs `↑`/`↓`)
- [ ] Gamepad support
- [ ] Resizable window with a scaled virtual court
- [ ] WebAssembly build (`wasm32-unknown-unknown`)
- [ ] Remember difficulty and input mode between sessions
- [ ] Screen shake / particle burst on scores

## License

MIT © 2026 Himansh Mewada — see [LICENSE](LICENSE).

## Acknowledgements

Built with [macroquad](https://github.com/not-fl3/macroquad). Inspired by the 1972 original.
