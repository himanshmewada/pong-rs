use macroquad::audio::{PlaySoundParams, Sound, load_sound_from_bytes, play_sound};
use macroquad::prelude::*;
use macroquad::rand::gen_range;

// ------------------------------------------------------------------ config
const SCREEN_W: f32 = 512.0;
const SCREEN_H: f32 = 512.0;

const PADDLE_W: f32 = 10.0;
const PADDLE_H: f32 = 60.0;
const PADDLE_WALL_CLEARANCE: f32 = 20.0;
const PLAYER_SPEED: f32 = 470.0;

const BALL_R: f32 = 6.0; // half-size of the classic square ball
const BALL_START_SPEED: f32 = 330.0;
const BALL_MAX_SPEED: f32 = 950.0;
const BALL_SPEEDUP: f32 = 1.045; // multiplier per paddle hit
const MAX_BOUNCE_ANGLE: f32 = std::f32::consts::FRAC_PI_3; // 60 degrees
const TRAIL_LEN: usize = 10;

const WIN_SCORE: u32 = 11;
const SERVE_DELAY: f32 = 1.1;

const BG_COLOR: Color = WHITE;
const PADDLE_COLOR: Color = RED;
const PADDLE_HIT_COLOR: Color = Color {
    r: 1.0,
    g: 0.55,
    b: 0.45,
    a: 1.0,
};
const BALL_COLOR: Color = Color {
    r: 0.12,
    g: 0.12,
    b: 0.14,
    a: 1.0,
};
const FAINT: Color = Color {
    r: 0.87,
    g: 0.87,
    b: 0.87,
    a: 1.0,
};
const HINT: Color = Color {
    r: 0.70,
    g: 0.70,
    b: 0.72,
    a: 1.0,
};
const TEXT_DIM: Color = Color {
    r: 0.40,
    g: 0.40,
    b: 0.43,
    a: 1.0,
};
const OVERLAY: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.88,
};

fn window_conf() -> Conf {
    Conf {
        window_title: "Pong".to_owned(),
        window_width: SCREEN_W as i32,
        window_height: SCREEN_H as i32,
        window_resizable: false,
        high_dpi: true,
        ..Default::default()
    }
}

// ------------------------------------------------------------------- types
#[derive(Clone, Copy, PartialEq)]
enum State {
    Menu,
    Serve,
    Play,
    Paused,
    GameOver,
}

#[derive(Clone, Copy, PartialEq)]
enum Difficulty {
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "EASY",
            Difficulty::Normal => "NORMAL",
            Difficulty::Hard => "HARD",
        }
    }
    fn cpu_speed(self) -> f32 {
        match self {
            Difficulty::Easy => 230.0,
            Difficulty::Normal => 340.0,
            Difficulty::Hard => 430.0,
        }
    }
    /// Random aiming error in pixels.
    fn aim_error(self) -> f32 {
        match self {
            Difficulty::Easy => 60.0,
            Difficulty::Normal => 26.0,
            Difficulty::Hard => 9.0,
        }
    }
    /// How often the CPU re-evaluates where the ball is going.
    fn reaction(self) -> f32 {
        match self {
            Difficulty::Easy => 0.25,
            Difficulty::Normal => 0.12,
            Difficulty::Hard => 0.05,
        }
    }
    /// Does it predict the bounce point, or just chase the ball?
    fn predicts(self) -> bool {
        !matches!(self, Difficulty::Easy)
    }
    fn next(self) -> Self {
        match self {
            Difficulty::Easy => Difficulty::Normal,
            Difficulty::Normal => Difficulty::Hard,
            Difficulty::Hard => Difficulty::Easy,
        }
    }
    fn prev(self) -> Self {
        match self {
            Difficulty::Easy => Difficulty::Hard,
            Difficulty::Normal => Difficulty::Easy,
            Difficulty::Hard => Difficulty::Normal,
        }
    }
}

#[derive(Clone, Copy)]
struct Paddle {
    rect: Rect,
    vy: f32,    // vertical velocity, used to impart spin
    flash: f32, // hit highlight timer
}

impl Paddle {
    fn new(x: f32) -> Self {
        Self {
            rect: Rect::new(x, (SCREEN_H - PADDLE_H) / 2.0, PADDLE_W, PADDLE_H),
            vy: 0.0,
            flash: 0.0,
        }
    }
    fn center_y(&self) -> f32 {
        self.rect.y + self.rect.h * 0.5
    }
    fn move_to(&mut self, y: f32, dt: f32) {
        let y = y.clamp(0.0, SCREEN_H - self.rect.h);
        self.vy = if dt > 0.0 {
            (y - self.rect.y) / dt
        } else {
            0.0
        };
        self.rect.y = y;
    }
    fn move_towards(&mut self, target_center: f32, max_speed: f32, dt: f32) {
        let delta = (target_center - self.rect.h * 0.5) - self.rect.y;
        if delta.abs() < 2.0 {
            self.move_to(self.rect.y, dt); // dead zone: stop the jitter
        } else {
            let step = delta.clamp(-max_speed * dt, max_speed * dt);
            self.move_to(self.rect.y + step, dt);
        }
    }
    fn reset(&mut self) {
        self.rect.y = (SCREEN_H - PADDLE_H) / 2.0;
        self.vy = 0.0;
        self.flash = 0.0;
    }
}

#[derive(Clone)]
struct Ball {
    pos: Vec2,
    vel: Vec2,
    speed: f32,
    trail: Vec<Vec2>,
}

impl Ball {
    fn new() -> Self {
        Self {
            pos: Vec2::new(SCREEN_W / 2.0, SCREEN_H / 2.0),
            vel: Vec2::ZERO,
            speed: BALL_START_SPEED,
            trail: Vec::new(),
        }
    }
    fn park(&mut self) {
        self.pos = Vec2::new(SCREEN_W / 2.0, SCREEN_H / 2.0);
        self.vel = Vec2::ZERO;
        self.speed = BALL_START_SPEED;
        self.trail.clear();
    }
    /// Axis-aligned overlap (the ball is a square of half-size BALL_R).
    fn hits(&self, p: &Rect) -> bool {
        self.pos.x + BALL_R > p.x
            && self.pos.x - BALL_R < p.x + p.w
            && self.pos.y + BALL_R > p.y
            && self.pos.y - BALL_R < p.y + p.h
    }
}

// ------------------------------------------------------------------- audio
/// Tiny synthesiser: builds a 16-bit mono PCM WAV in memory so the game
/// needs no external asset files. `notes` is a list of (frequency, seconds).
fn synth_wav(notes: &[(f32, f32)], volume: f32) -> Vec<u8> {
    const RATE: u32 = 44_100;
    let mut samples: Vec<i16> = Vec::new();

    for &(freq, dur) in notes {
        let n = (RATE as f32 * dur) as usize;
        if n == 0 {
            continue;
        }
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let square = if (t * freq).fract() < 0.5 { 1.0 } else { -1.0 };
            let sine = (t * freq * std::f32::consts::TAU).sin();
            let p = i as f32 / n as f32;
            let env = (p / 0.05).min(1.0) * ((1.0 - p) / 0.25).min(1.0); // attack/release
            let v = (0.55 * square + 0.45 * sine) * env * volume;
            samples.push((v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
        }
    }

    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

struct Sounds {
    paddle: Option<Sound>,
    wall: Option<Sound>,
    score: Option<Sound>,
    win: Option<Sound>,
    lose: Option<Sound>,
    ui: Option<Sound>,
}

impl Sounds {
    async fn load() -> Self {
        async fn s(notes: &[(f32, f32)], vol: f32) -> Option<Sound> {
            load_sound_from_bytes(&synth_wav(notes, vol)).await.ok()
        }
        Self {
            paddle: s(&[(660.0, 0.05)], 0.35).await,
            wall: s(&[(330.0, 0.04)], 0.30).await,
            score: s(&[(180.0, 0.20)], 0.35).await,
            win: s(&[(523.0, 0.09), (659.0, 0.09), (784.0, 0.18)], 0.35).await,
            lose: s(&[(392.0, 0.12), (311.0, 0.12), (233.0, 0.24)], 0.35).await,
            ui: s(&[(880.0, 0.04)], 0.25).await,
        }
    }
}

fn play(sound: &Option<Sound>, volume: f32) {
    if let Some(s) = sound {
        play_sound(
            s,
            PlaySoundParams {
                looped: false,
                volume,
            },
        );
    }
}

// -------------------------------------------------------------------- game
struct Game {
    player: Paddle,
    opponent: Paddle,
    ball: Ball,
    score_player: u32,
    score_cpu: u32,
    state: State,
    difficulty: Difficulty,
    serve_dir: f32,
    serve_timer: f32,
    rally: u32,
    mouse_control: bool,
    ai_target: f32,
    ai_timer: f32,
}

impl Game {
    fn new() -> Self {
        Self {
            player: Paddle::new(PADDLE_WALL_CLEARANCE),
            opponent: Paddle::new(SCREEN_W - PADDLE_WALL_CLEARANCE - PADDLE_W),
            ball: Ball::new(),
            score_player: 0,
            score_cpu: 0,
            state: State::Menu,
            difficulty: Difficulty::Normal,
            serve_dir: if gen_range(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 },
            serve_timer: SERVE_DELAY,
            rally: 0,
            mouse_control: false,
            ai_target: SCREEN_H / 2.0,
            ai_timer: 0.0,
        }
    }

    fn start_match(&mut self) {
        self.score_player = 0;
        self.score_cpu = 0;
        self.player.reset();
        self.opponent.reset();
        self.ball.park();
        self.serve_dir = if gen_range(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
        self.serve_timer = SERVE_DELAY;
        self.rally = 0;
        self.state = State::Serve;
    }

    fn launch_ball(&mut self) {
        let angle = gen_range(-0.35f32, 0.35); // ~ +/- 20 degrees
        self.ball.speed = BALL_START_SPEED;
        self.ball.vel = Vec2::new(self.serve_dir * angle.cos(), angle.sin()) * self.ball.speed;
        self.ball.trail.clear();
        self.rally = 0;
        self.state = State::Play;
    }

    // ---------------------------------------------------------------- update
    fn update(&mut self, dt: f32, sounds: &Sounds) {
        if is_key_pressed(KeyCode::M) {
            self.mouse_control = !self.mouse_control;
            play(&sounds.ui, 0.6);
        }

        self.player.flash = (self.player.flash - dt).max(0.0);
        self.opponent.flash = (self.opponent.flash - dt).max(0.0);

        match self.state {
            State::Menu => {
                if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
                    self.difficulty = self.difficulty.prev();
                    play(&sounds.ui, 0.6);
                }
                if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
                    self.difficulty = self.difficulty.next();
                    play(&sounds.ui, 0.6);
                }
                if is_key_pressed(KeyCode::Key1) {
                    self.difficulty = Difficulty::Easy;
                }
                if is_key_pressed(KeyCode::Key2) {
                    self.difficulty = Difficulty::Normal;
                }
                if is_key_pressed(KeyCode::Key3) {
                    self.difficulty = Difficulty::Hard;
                }
                if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) {
                    self.start_match();
                    play(&sounds.ui, 0.8);
                }
            }

            State::Serve => {
                self.update_player(dt);
                self.update_cpu(dt);
                self.serve_timer -= dt;
                if self.serve_timer <= 0.0 || is_key_pressed(KeyCode::Space) {
                    self.launch_ball();
                }
                if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
                    self.state = State::Paused;
                }
            }

            State::Play => {
                if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
                    self.state = State::Paused;
                    return;
                }

                self.update_player(dt);
                self.update_cpu(dt);

                if let Some(conceded) = self.step_physics(dt, sounds) {
                    // Serve towards whoever just conceded the point.
                    if conceded == Side::Left {
                        self.score_cpu += 1;
                        self.serve_dir = -1.0;
                    } else {
                        self.score_player += 1;
                        self.serve_dir = 1.0;
                    }
                    self.ball.park();
                    play(&sounds.score, 0.8);

                    if self.score_player >= WIN_SCORE || self.score_cpu >= WIN_SCORE {
                        self.state = State::GameOver;
                        if self.score_player > self.score_cpu {
                            play(&sounds.win, 0.9);
                        } else {
                            play(&sounds.lose, 0.9);
                        }
                    } else {
                        self.serve_timer = SERVE_DELAY;
                        self.state = State::Serve;
                    }
                } else {
                    self.ball.trail.push(self.ball.pos);
                    if self.ball.trail.len() > TRAIL_LEN {
                        self.ball.trail.remove(0);
                    }
                }
            }

            State::Paused => {
                if is_key_pressed(KeyCode::P)
                    || is_key_pressed(KeyCode::Escape)
                    || is_key_pressed(KeyCode::Space)
                {
                    self.state = State::Play;
                }
                if is_key_pressed(KeyCode::Q) {
                    self.state = State::Menu;
                }
            }

            State::GameOver => {
                if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) {
                    self.start_match();
                }
                if is_key_pressed(KeyCode::Q) || is_key_pressed(KeyCode::Escape) {
                    self.state = State::Menu;
                }
            }
        }
    }

    fn update_player(&mut self, dt: f32) {
        if self.mouse_control {
            let (_, my) = mouse_position();
            self.player.move_to(my - PADDLE_H * 0.5, dt);
        } else {
            let mut dir = 0.0;
            if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
                dir -= 1.0;
            }
            if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
                dir += 1.0;
            }
            let y = self.player.rect.y + dir * PLAYER_SPEED * dt;
            self.player.move_to(y, dt);
        }
    }

    fn update_cpu(&mut self, dt: f32) {
        self.ai_timer -= dt;
        if self.ai_timer <= 0.0 {
            self.ai_timer = self.difficulty.reaction();
            let err = self.difficulty.aim_error();
            if self.state == State::Play && self.ball.vel.x > 0.0 {
                let mut target = if self.difficulty.predicts() {
                    predict_ball_y(&self.ball, self.opponent.rect.x - BALL_R)
                } else {
                    self.ball.pos.y
                };
                target += gen_range(-err, err);
                self.ai_target = target;
            } else {
                // Idle: drift back towards the middle of the court.
                self.ai_target = SCREEN_H / 2.0 + gen_range(-20.0, 20.0);
            }
        }
        let speed = self.difficulty.cpu_speed();
        self.opponent.move_towards(self.ai_target, speed, dt);
    }

    /// Reflect the ball off a paddle; `dir` is +1 for the left paddle, -1 for the right.
    fn bounce(&mut self, paddle: &Paddle, dir: f32) {
        let rel = ((self.ball.pos.y - paddle.center_y()) / (paddle.rect.h * 0.5)).clamp(-1.0, 1.0);
        let spin = (paddle.vy / PLAYER_SPEED).clamp(-1.0, 1.0) * 0.15;
        let angle = (rel + spin).clamp(-1.0, 1.0) * MAX_BOUNCE_ANGLE;

        self.ball.speed = (self.ball.speed * BALL_SPEEDUP).min(BALL_MAX_SPEED);
        self.ball.vel = Vec2::new(dir * angle.cos(), angle.sin()) * self.ball.speed;
        self.rally += 1;
    }

    /// Sub-stepped integration so a fast ball can never skip through a paddle.
    /// Returns which side conceded, if anyone did.
    fn step_physics(&mut self, dt: f32, sounds: &Sounds) -> Option<Side> {
        let travel = self.ball.vel.length() * dt;
        let steps = (travel / (BALL_R.min(PADDLE_W) * 0.5)).ceil().max(1.0) as usize;
        let sdt = dt / steps as f32;

        for _ in 0..steps {
            self.ball.pos += self.ball.vel * sdt;

            // Top / bottom walls.
            if self.ball.pos.y - BALL_R < 0.0 {
                self.ball.pos.y = BALL_R;
                self.ball.vel.y = self.ball.vel.y.abs();
                play(&sounds.wall, 0.7);
            } else if self.ball.pos.y + BALL_R > SCREEN_H {
                self.ball.pos.y = SCREEN_H - BALL_R;
                self.ball.vel.y = -self.ball.vel.y.abs();
                play(&sounds.wall, 0.7);
            }

            // Paddles (only when travelling towards them).
            if self.ball.vel.x < 0.0 && self.ball.hits(&self.player.rect) {
                let p = self.player;
                self.bounce(&p, 1.0);
                self.ball.pos.x = p.rect.x + p.rect.w + BALL_R + 0.01;
                self.player.flash = 0.12;
                play(&sounds.paddle, 0.9);
            } else if self.ball.vel.x > 0.0 && self.ball.hits(&self.opponent.rect) {
                let p = self.opponent;
                self.bounce(&p, -1.0);
                self.ball.pos.x = p.rect.x - BALL_R - 0.01;
                self.opponent.flash = 0.12;
                play(&sounds.paddle, 0.9);
            }

            // Goals.
            if self.ball.pos.x + BALL_R < 0.0 {
                return Some(Side::Left);
            }
            if self.ball.pos.x - BALL_R > SCREEN_W {
                return Some(Side::Right);
            }
        }
        None
    }

    // ------------------------------------------------------------------ draw
    fn draw(&self) {
        clear_background(BG_COLOR);
        self.draw_court();
        self.draw_scores();
        self.draw_ball();
        self.draw_paddle(&self.player);
        self.draw_paddle(&self.opponent);
        self.draw_hud();

        match self.state {
            State::Menu => self.draw_menu(),
            State::Serve => {
                let n = self.serve_timer.ceil().max(1.0) as i32;
                draw_text_centered(
                    &n.to_string(),
                    SCREEN_W / 2.0,
                    SCREEN_H / 2.0 - 40.0,
                    64,
                    FAINT,
                );
            }
            State::Paused => self.draw_paused(),
            State::GameOver => self.draw_game_over(),
            State::Play => {}
        }
    }

    fn draw_court(&self) {
        let (dash, gap) = (12.0f32, 10.0f32);
        let mut y = 0.0;
        while y < SCREEN_H {
            draw_rectangle(SCREEN_W / 2.0 - 2.0, y, 4.0, dash.min(SCREEN_H - y), FAINT);
            y += dash + gap;
        }
    }

    fn draw_scores(&self) {
        draw_text_centered(
            &self.score_player.to_string(),
            SCREEN_W * 0.25,
            92.0,
            72,
            FAINT,
        );
        draw_text_centered(
            &self.score_cpu.to_string(),
            SCREEN_W * 0.75,
            92.0,
            72,
            FAINT,
        );
    }

    fn draw_paddle(&self, p: &Paddle) {
        let color = if p.flash > 0.0 {
            PADDLE_HIT_COLOR
        } else {
            PADDLE_COLOR
        };
        draw_rectangle(p.rect.x, p.rect.y, p.rect.w, p.rect.h, color);
    }

    fn draw_ball(&self) {
        let n = self.ball.trail.len();
        for (i, t) in self.ball.trail.iter().enumerate() {
            let f = (i + 1) as f32 / (n + 1) as f32;
            let r = BALL_R * (0.35 + 0.55 * f);
            let c = Color {
                r: 0.6,
                g: 0.6,
                b: 0.62,
                a: 0.22 * f,
            };
            draw_rectangle(t.x - r, t.y - r, r * 2.0, r * 2.0, c);
        }
        // Classic Pong ball is a square; swap for draw_circle(x, y, BALL_R, ..) if you prefer.
        draw_rectangle(
            self.ball.pos.x - BALL_R,
            self.ball.pos.y - BALL_R,
            BALL_R * 2.0,
            BALL_R * 2.0,
            BALL_COLOR,
        );
    }

    fn draw_hud(&self) {
        let mode = if self.mouse_control { "MOUSE" } else { "KEYS" };
        draw_text(
            &format!("P PAUSE   M {}", mode),
            12.0,
            SCREEN_H - 12.0,
            16.0,
            HINT,
        );
        let rally = format!("RALLY {}", self.rally);
        let w = measure_text(&rally, None, 16, 1.0).width; // measure_text still wants u16
        draw_text(&rally, SCREEN_W - w - 12.0, SCREEN_H - 12.0, 16.0, HINT);
    }

    fn draw_menu(&self) {
        draw_rectangle(0.0, 0.0, SCREEN_W, SCREEN_H, OVERLAY);
        draw_text_centered("PONG", SCREEN_W / 2.0, 150.0, 84, PADDLE_COLOR);
        draw_text_centered(
            &format!("FIRST TO {} WINS", WIN_SCORE),
            SCREEN_W / 2.0,
            190.0,
            22,
            TEXT_DIM,
        );
        draw_text_centered(
            &format!("< {} >", self.difficulty.name()),
            SCREEN_W / 2.0,
            262.0,
            34,
            BALL_COLOR,
        );
        draw_text_centered("LEFT / RIGHT or 1 - 2 - 3", SCREEN_W / 2.0, 290.0, 18, HINT);
        draw_text_centered(
            "W / S or UP / DOWN to move",
            SCREEN_W / 2.0,
            336.0,
            20,
            TEXT_DIM,
        );
        draw_text_centered(
            "M toggles mouse control",
            SCREEN_W / 2.0,
            362.0,
            20,
            TEXT_DIM,
        );
        draw_text_centered("P pauses the game", SCREEN_W / 2.0, 388.0, 20, TEXT_DIM);
        draw_text_centered(
            "PRESS SPACE TO START",
            SCREEN_W / 2.0,
            452.0,
            26,
            PADDLE_COLOR,
        );
    }

    fn draw_paused(&self) {
        draw_rectangle(0.0, 0.0, SCREEN_W, SCREEN_H, OVERLAY);
        draw_text_centered("PAUSED", SCREEN_W / 2.0, 240.0, 56, BALL_COLOR);
        draw_text_centered(
            "P / ESC / SPACE to resume",
            SCREEN_W / 2.0,
            290.0,
            20,
            TEXT_DIM,
        );
        draw_text_centered("Q to quit to menu", SCREEN_W / 2.0, 318.0, 20, TEXT_DIM);
    }

    fn draw_game_over(&self) {
        draw_rectangle(0.0, 0.0, SCREEN_W, SCREEN_H, OVERLAY);
        let won = self.score_player > self.score_cpu;
        draw_text_centered(
            if won { "YOU WIN!" } else { "CPU WINS" },
            SCREEN_W / 2.0,
            220.0,
            56,
            if won { PADDLE_COLOR } else { BALL_COLOR },
        );
        draw_text_centered(
            &format!("{}  -  {}", self.score_player, self.score_cpu),
            SCREEN_W / 2.0,
            276.0,
            40,
            TEXT_DIM,
        );
        draw_text_centered(
            &format!("DIFFICULTY: {}", self.difficulty.name()),
            SCREEN_W / 2.0,
            312.0,
            18,
            HINT,
        );
        draw_text_centered("SPACE to play again", SCREEN_W / 2.0, 372.0, 22, TEXT_DIM);
        draw_text_centered("Q / ESC for menu", SCREEN_W / 2.0, 400.0, 22, TEXT_DIM);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Left,
    Right,
}

/// Where will the ball cross `target_x`, accounting for wall bounces?
/// Uses a triangle-wave fold instead of simulating step by step.
fn predict_ball_y(ball: &Ball, target_x: f32) -> f32 {
    if ball.vel.x <= 0.0 {
        return SCREEN_H / 2.0;
    }
    let t = (target_x - ball.pos.x) / ball.vel.x;
    if t <= 0.0 {
        return ball.pos.y;
    }
    let lo = BALL_R;
    let hi = SCREEN_H - BALL_R;
    let span = hi - lo;
    let mut m = (ball.pos.y + ball.vel.y * t - lo).rem_euclid(2.0 * span);
    if m > span {
        m = 2.0 * span - m;
    }
    lo + m
}

fn draw_text_centered(text: &str, center_x: f32, baseline_y: f32, size: u16, color: Color) {
    let w = measure_text(text, None, size, 1.0).width;
    draw_text(text, center_x - w / 2.0, baseline_y, size as f32, color);
}

// -------------------------------------------------------------------- main
#[macroquad::main(window_conf)]
async fn main() {
    let sounds = Sounds::load().await;
    let mut game = Game::new();

    loop {
        // Clamp dt so a stall / window drag can't teleport the ball.
        let dt = get_frame_time().min(0.05);
        game.update(dt, &sounds);
        game.draw();
        next_frame().await; // must be INSIDE the loop
    }
}
