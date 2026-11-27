//! Launch splash — a faithful port of `frontend/boot.html`, `css/splash.css`
//! and the timing rules in `frontend/js/boot.js`.

use std::time::Instant;

use egui::{Align2, Color32, Pos2, Rect, Sense, Vec2};

use crate::theme::*;

/// `MIN_SPLASH_MS` in boot.js — the splash never leaves before 10 s.
const MIN_MS: u128 = 10_000;
/// Escape hatch: after 25 s the splash stops waiting for the backend.
const TIMEOUT_MS: u128 = 25_000;
/// `PHASES` in boot.js.
const PHASES: &[(u128, &str)] = &[
    (0, "Loading browser assets"),
    (3_400, "Starting model"),
    (6_800, "Getting ready"),
];

/// `rgba(64, 132, 235, …)` — the blue used by the splash glow / logo shadow.
const GLOW_BLUE: Color32 = Color32::from_rgb(0x40, 0x84, 0xeb);

const LOGO_SIDE: f32 = 68.0;
const WORDMARK: &str = "Automata";
const WORDMARK_SIZE: f32 = 58.0;
const TOP_LINE: &str = "OOP LAB PROJECT";
const SLOGAN: &str = "A browser agent, built on simple rules.";

/* ============================================================
   BOOT TIMELINE
   ============================================================ */

pub struct Splash {
    started: Instant,
    finished: bool,
}

impl Default for Splash {
    fn default() -> Self {
        Self::new()
    }
}

impl Splash {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            finished: false,
        }
    }

    /// One step of the boot timeline. Returns `(status line, finished?)`.
    ///
    /// The splash holds for a minimum of 10 s, cycling the three phase lines,
    /// then either finishes ("Ready") or keeps waiting for the backend until
    /// the 25 s escape hatch fires.
    pub fn tick(&mut self, backend_ok: bool) -> (String, bool) {
        if self.finished {
            return ("Ready".to_string(), true);
        }

        let elapsed = self.started.elapsed().as_millis();

        if elapsed < MIN_MS {
            let mut text = PHASES[0].1;
            for (at, phase) in PHASES {
                if elapsed >= *at {
                    text = phase;
                }
            }
            return (text.to_string(), false);
        }

        if !backend_ok && elapsed < TIMEOUT_MS {
            return ("Waiting for backend".to_string(), false);
        }

        self.finished = true;
        ("Ready".to_string(), true)
    }

    pub fn active(&self) -> bool {
        !self.finished
    }
}

/* ============================================================
   PAINTING
   ============================================================ */

pub fn show(ui: &mut egui::Ui, status: &str) {
    let rect = ui.available_rect_before_wrap();
    let _ = ui.allocate_rect(rect, Sense::hover());
    if rect.width() < 80.0 || rect.height() < 120.0 {
        return;
    }
    let painter = ui.painter();
    let cx = rect.center().x;

    // Everything outside the gradient ellipse stays the outermost stop.
    painter.rect_filled(rect, 0.0, SPLASH_EDGE);

    // background: radial-gradient(120% 90% at 50% 0%, #16407f 0%, #0b2450 46%, #071733 100%)
    fill_radial(
        &painter,
        Pos2::new(cx, rect.top()),
        Vec2::new(rect.width() * 1.2, rect.height() * 0.9),
        96,
        |t| {
            if t <= 0.46 {
                lerp(SPLASH_TOP, SPLASH_MID, t / 0.46)
            } else {
                lerp(SPLASH_MID, SPLASH_EDGE, (t - 0.46) / 0.54)
            }
        },
    );

    // .splash::before — radial-gradient(52% 46% at 50% 44%, rgba(64,132,235,.22), transparent 70%)
    fill_radial(
        &painter,
        Pos2::new(cx, rect.top() + rect.height() * 0.44),
        Vec2::new(rect.width() * 0.52, rect.height() * 0.46),
        64,
        |t| {
            if t >= 0.7 {
                Color32::TRANSPARENT
            } else {
                with_alpha(GLOW_BLUE, ((1.0 - t / 0.7) * 0.22 * 255.0).round() as u8)
            }
        },
    );

    // ---- content box: padding 34px 46px 30px, space-between ----
    let content = Rect::from_min_max(
        Pos2::new(rect.left() + 46.0, rect.top() + 34.0),
        Pos2::new(rect.right() - 46.0, rect.bottom() - 30.0),
    );
    if content.width() < 80.0 || content.height() < 140.0 {
        return;
    }

    const TOP_H: f32 = 13.0;
    const CENTER_H: f32 = 156.0;
    const BOTTOM_H: f32 = 47.0;

    let gap = (content.height() - TOP_H - CENTER_H - BOTTOM_H).max(0.0) * 0.5;
    let center_top = content.top() + TOP_H + gap;

    // ---- topline ----
    paint_tracked(
        &painter,
        Align2::CENTER_CENTER,
        Pos2::new(cx, content.top() + TOP_H * 0.5),
        TOP_LINE,
        mono_font(10.0),
        SPLASH_TOPLINE,
        10.0 * 0.28,
    );

    // ---- centre: logo / wordmark / release ----
    let logo_rect = Rect::from_center_size(
        Pos2::new(cx, center_top + LOGO_SIDE * 0.5),
        Vec2::splat(LOGO_SIDE),
    );
    fill_radial(
        &painter,
        logo_rect.center(),
        Vec2::splat(LOGO_SIDE * 0.5 + 18.0),
        32,
        |t| with_alpha(GLOW_BLUE, ((1.0 - t).max(0.0) * 0.35 * 255.0).round() as u8),
    );
    paint_logo(&painter, logo_rect, SPLASH_LOGO);

    let wordmark_h = WORDMARK_SIZE * 0.95;
    let wordmark_y = center_top + LOGO_SIDE + 10.0 + wordmark_h * 0.5;
    paint_tracked(
        &painter,
        Align2::CENTER_CENTER,
        Pos2::new(cx, wordmark_y),
        WORDMARK,
        display_font(WORDMARK_SIZE),
        SPLASH_WORDMARK,
        WORDMARK_SIZE * -0.02,
    );

    let release_y = center_top + LOGO_SIDE + 10.0 + wordmark_h + 10.0 + 6.5;
    paint_tracked(
        &painter,
        Align2::CENTER_CENTER,
        Pos2::new(cx, release_y),
        "(PRE RELEASE)",
        mono_font(10.0),
        AMBER,
        10.0 * 0.24,
    );

    // ---- bottom: slogan + status ----
    let status_h = 16.0;
    let bottom_gap = 12.0;
    let slogan_h = 15.0 * 1.3;
    let status_y = content.bottom() - status_h * 0.5;
    let slogan_y = content.bottom() - status_h - bottom_gap - slogan_h * 0.5;

    // .splash-slogan is italic; our display face has no italic cut, so let
    // epaint shear it (`TextFormat::italics`).
    paint_tracked_format(
        &painter,
        Align2::CENTER_CENTER,
        Pos2::new(cx, slogan_y),
        SLOGAN,
        egui::text::TextFormat {
            font_id: display_font(15.0),
            color: SPLASH_SLOGAN,
            italics: true,
            ..Default::default()
        },
    );

    paint_tracked(
        &painter,
        Align2::CENTER_CENTER,
        Pos2::new(cx, status_y),
        &status.to_uppercase(),
        mono_font(10.0),
        SPLASH_STATUS,
        10.0 * 0.18,
    );
}
