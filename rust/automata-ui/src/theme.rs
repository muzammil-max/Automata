//! Design tokens + paint helpers.
//! Every value here mirrors `frontend/css/theme.css`.

use std::sync::Arc;

pub use egui::Color32;
use egui::{FontFamily, FontId, Rect, Sense, Stroke, Ui, Vec2};

/* ---------------- solid palette ---------------- */
pub const INK: Color32 = Color32::from_rgb(0x0c, 0x1a, 0x30);
pub const INK_2: Color32 = Color32::from_rgb(0x08, 0x14, 0x26);
pub const PAPER: Color32 = Color32::from_rgb(0x0d, 0x1a, 0x2e);
pub const PAPER_2: Color32 = Color32::from_rgb(0x14, 0x2a, 0x48);
pub const PAPER_3: Color32 = Color32::from_rgb(0x1a, 0x36, 0x60);
pub const USER_BUBBLE: Color32 = Color32::from_rgb(0x1d, 0x3a, 0x5f);
pub const NAVY: Color32 = Color32::from_rgb(0xcf, 0xe0, 0xf5);
pub const NAVY_SOFT: Color32 = Color32::from_rgb(0x9d, 0xb8, 0xdd);
pub const CREAM: Color32 = Color32::from_rgb(0xdc, 0xe8, 0xf8);
pub const MUTED: Color32 = Color32::from_rgb(0x8b, 0xa3, 0xc4);
pub const AMBER: Color32 = Color32::from_rgb(0xf2, 0xb1, 0x34);
pub const GREEN: Color32 = Color32::from_rgb(0x4f, 0xbf, 0x8b);
pub const RED: Color32 = Color32::from_rgb(0xe0, 0x5c, 0x5c);
pub const DARK_TEXT: Color32 = Color32::from_rgb(0x12, 0x14, 0x17);

/* ---------------- alpha tokens (premultiplied, exactly as in CSS) ---------------- */
pub const INK_LINE: Color32 = Color32::from_rgba_premultiplied(22, 28, 38, 41); // rgba(138,178,240,.16)
pub const INK_MUTED: Color32 = Color32::from_rgba_premultiplied(107, 119, 139, 153); // rgba(178,198,232,.6)
pub const LINE: Color32 = Color32::from_rgba_premultiplied(22, 29, 38, 41); // rgba(140,180,240,.16)
pub const LINE_STRONG: Color32 = Color32::from_rgba_premultiplied(36, 47, 62, 66); // rgba(140,180,240,.26)
pub const AMBER_SOFT: Color32 = Color32::from_rgba_premultiplied(39, 28, 8, 41); // rgba(242,177,52,.16)
pub const AMBER_LINE: Color32 = Color32::from_rgba_premultiplied(109, 79, 24, 115); // rgba(242,177,52,.45)
pub const AMBER_TINT: Color32 = Color32::from_rgba_premultiplied(19, 14, 4, 20); // rgba(242,177,52,.08)
pub const HEADER_FILL: Color32 = Color32::from_rgb(0x08, 0x13, 0x24);
pub const CHIP_FILL: Color32 = Color32::from_rgb(0x1e, 0x21, 0x24);
pub const HINT: Color32 = Color32::from_rgba_premultiplied(86, 87, 89, 102); // rgba(214,217,222,.4)
pub const PLACEHOLDER: Color32 = Color32::from_rgba_premultiplied(81, 83, 85, 97); // rgba(214,217,222,.38)
pub const SESSION_TITLE: Color32 = Color32::from_rgba_premultiplied(197, 200, 204, 235); // rgba(214,217,222,.92)
pub const PILL_TEXT: Color32 = Color32::from_rgba_premultiplied(167, 169, 173, 199); // rgba(214,217,222,.78)
pub const WHITE_04: Color32 = Color32::from_rgba_premultiplied(10, 10, 10, 10); // rgba(255,255,255,.04)
pub const WHITE_05: Color32 = Color32::from_rgba_premultiplied(13, 13, 13, 13); // rgba(255,255,255,.05)
pub const WHITE_07: Color32 = Color32::from_rgba_premultiplied(18, 18, 18, 18); // rgba(255,255,255,.07)
pub const WHITE_08: Color32 = Color32::from_rgba_premultiplied(17, 17, 18, 20); // rgba(214,217,222,.08)
pub const USER_BORDER: Color32 = Color32::from_rgba_premultiplied(20, 20, 20, 20); // rgba(255,255,255,.08)
pub const STOP_FILL: Color32 = Color32::from_rgba_premultiplied(22, 9, 9, 26); // rgba(224,92,92,.10)
pub const STOP_LINE: Color32 = Color32::from_rgba_premultiplied(54, 22, 22, 61); // rgba(224,92,92,.24)
pub const DANGER_TINT: Color32 = Color32::from_rgba_premultiplied(31, 13, 13, 36); // rgba(224,92,92,.14)
pub const BACKDROP: Color32 = Color32::from_rgba_premultiplied(7, 8, 10, 166); // rgba(10,12,15,.65)

/* ---------------- splash / gradient tokens ---------------- */
pub const SPLASH_TOP: Color32 = Color32::from_rgb(0x16, 0x40, 0x7f);
pub const SPLASH_MID: Color32 = Color32::from_rgb(0x0b, 0x24, 0x50);
pub const SPLASH_EDGE: Color32 = Color32::from_rgb(0x07, 0x17, 0x33);
pub const SPLASH_LOGO: Color32 = Color32::from_rgb(0xe9, 0xef, 0xfc);
pub const SPLASH_WORDMARK: Color32 = Color32::from_rgb(0xf2, 0xf5, 0xfc);
pub const SPLASH_STATUS: Color32 = Color32::from_rgb(0xe2, 0xea, 0xfb);
pub const SPLASH_TOPLINE: Color32 = Color32::from_rgba_premultiplied(131, 143, 163, 168); // rgba(198,216,247,.66)
pub const SPLASH_SLOGAN: Color32 = Color32::from_rgba_premultiplied(150, 162, 184, 189); // rgba(203,219,248,.74)
/// `rgba(88, 140, 240, …)` — the blue behind the sidebar / chat glows.
pub const GLOW_BLUE: Color32 = Color32::from_rgb(0x58, 0x8c, 0xf0);

/* ---------------- fonts ---------------- */

pub fn display_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("display".into()))
}

pub fn mono_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

pub fn body_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// A dialog-button label: uppercase mono with the reference's 0.08em tracking.
///
/// `.dialog-btn` in `frontend/css/app.css` is `text-transform: uppercase` with
/// `letter-spacing: 0.08em`, which egui has no direct equivalent for, so the
/// glyphs are uppercased and the tracking is baked into the text format.
pub fn dialog_label(text: &str, size: f32, color: Color32) -> egui::WidgetText {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        egui::text::TextFormat {
            font_id: mono_font(size),
            color,
            extra_letter_spacing: size * 0.08,
            ..Default::default()
        },
    );
    job.into()
}

pub fn install_theme(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "automata-display".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/InstrumentSerif-Regular.ttf"
        ))),
    );
    fonts.families.insert(
        FontFamily::Name("display".into()),
        vec!["automata-display".to_owned()],
    );
    fonts.font_data.insert(
        "automata-mono".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/IBMPlexMono-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "automata-mono-medium".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/IBMPlexMono-Medium.ttf"
        ))),
    );
    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
        mono.insert(0, "automata-mono".to_owned());
        mono.insert(1, "automata-mono-medium".to_owned());
    }
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = PAPER;
    style.visuals.window_fill = PAPER_2;
    style.visuals.extreme_bg_color = INK_2;
    style.visuals.faint_bg_color = PAPER_3;
    style.visuals.override_text_color = Some(NAVY);
    style.visuals.window_stroke = Stroke::new(1.0, LINE_STRONG);
    style.visuals.selection.bg_fill = Color32::from_rgba_premultiplied(61, 44, 13, 85); // rgba(61,44,13,.33) selection
    style.visuals.selection.stroke = Stroke::new(1.0, AMBER);
    style.visuals.widgets.noninteractive.bg_fill = PAPER_2;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, NAVY);
    style.visuals.widgets.inactive.bg_fill = PAPER_2;
    style.visuals.widgets.inactive.weak_bg_fill = PAPER_2;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, NAVY_SOFT);
    style.visuals.widgets.hovered.bg_fill = PAPER_3;
    style.visuals.widgets.hovered.weak_bg_fill = PAPER_3;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, LINE_STRONG);
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, CREAM);
    style.visuals.widgets.active.bg_fill = PAPER_3;
    style.visuals.widgets.active.weak_bg_fill = PAPER_3;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, AMBER);
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, CREAM);
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 7.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.scroll.bar_width = 8.0;
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::new(14.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        FontId::new(13.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        FontId::new(11.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(20.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        FontId::new(11.0, FontFamily::Monospace),
    );
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
    ctx.set_theme(egui::ThemePreference::Dark);
}

/* ---------------- colour maths ---------------- */

pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let [ar, ag, ab, aa] = a.to_array();
    let [br, bg, bb, ba] = b.to_array();
    let mix = |x: u8, y: u8| -> u8 { (x as f32 + (y as f32 - x as f32) * t).round() as u8 };
    Color32::from_rgba_premultiplied(
        mix(ar, br),
        mix(ag, bg),
        mix(ab, bb),
        mix(aa, ba),
    )
}

pub fn with_alpha(c: Color32, alpha: u8) -> Color32 {
    let [r, g, b, _] = c.to_array();
    let f = alpha as f32 / 255.0;
    Color32::from_rgba_premultiplied(
        ((r as f32) * f).round() as u8,
        ((g as f32) * f).round() as u8,
        ((b as f32) * f).round() as u8,
        alpha,
    )
}

/* ---------------- background painters ---------------- */

/// Vertical two-stop gradient (used for the sidebar: ink -> ink-2).
pub fn fill_vertical_gradient(painter: &egui::Painter, rect: Rect, top: Color32, bottom: Color32) {
    let bands = 32;
    let h = rect.height() / bands as f32;
    if h <= 0.0 {
        return;
    }
    for i in 0..bands {
        let t = i as f32 / (bands - 1) as f32;
        let y0 = rect.top() + i as f32 * h;
        let r = Rect::from_min_max(
            egui::pos2(rect.left(), y0),
            egui::pos2(rect.right(), (y0 + h + 1.0).min(rect.bottom())),
        );
        painter.rect_filled(r, 0.0, lerp(top, bottom, t));
    }
}

/// Concentric-ring radial gradient (egui has no native gradients).
///
/// `color_at(t)` returns the colour at normalized distance `t` from the focus,
/// so `t = 0` is the focus and `t = 1` the outer edge of `radius`.
///
/// Every ring is a real annulus drawn exactly once, so translucent colours
/// composite against the background instead of stacking on top of each other.
pub fn fill_radial(
    painter: &egui::Painter,
    focus: egui::Pos2,
    radius: Vec2,
    rings: usize,
    color_at: impl Fn(f32) -> Color32,
) {
    if radius.x <= 0.0 || radius.y <= 0.0 || rings == 0 {
        return;
    }
    const SEGMENTS: usize = 72;
    let tau = std::f32::consts::TAU;

    for i in (0..rings).rev() {
        let t_out = (i as f32 + 1.0) / rings as f32;
        let t_in = i as f32 / rings as f32;
        let color = color_at(t_out);
        if color.a() == 0 {
            continue;
        }

        let mut mesh = egui::epaint::Mesh::default();
        for k in 0..SEGMENTS {
            let a0 = k as f32 * tau / SEGMENTS as f32;
            let a1 = (k + 1) as f32 * tau / SEGMENTS as f32;
            let point = |a: f32, t: f32| {
                egui::pos2(
                    focus.x + radius.x * t * a.cos(),
                    focus.y + radius.y * t * a.sin(),
                )
            };
            let o0 = push_vertex(&mut mesh, point(a0, t_out), color);
            let o1 = push_vertex(&mut mesh, point(a1, t_out), color);
            let i0 = push_vertex(
                &mut mesh,
                if t_in > 0.0 { point(a0, t_in) } else { focus },
                color,
            );
            let i1 = push_vertex(
                &mut mesh,
                if t_in > 0.0 { point(a1, t_in) } else { focus },
                color,
            );
            mesh.add_triangle(o0, o1, i1);
            mesh.add_triangle(o0, i1, i0);
        }
        painter.add(egui::Shape::Mesh(std::sync::Arc::new(mesh)));
    }
}

fn push_vertex(mesh: &mut egui::epaint::Mesh, pos: egui::Pos2, color: Color32) -> u32 {
    let index = mesh.vertices.len() as u32;
    mesh.vertices.push(egui::epaint::Vertex {
        pos,
        uv: egui::epaint::WHITE_UV,
        color,
    });
    index
}

/// Letter-spaced text (CSS `letter-spacing`), anchored with [`egui::Align2`].
///
/// Returns the laid-out rect so callers can keep stacking blocks.
pub fn paint_tracked(
    painter: &egui::Painter,
    anchor: egui::Align2,
    center: egui::Pos2,
    text: &str,
    font: FontId,
    color: Color32,
    tracking: f32,
) -> Rect {
    paint_tracked_format(
        painter,
        anchor,
        center,
        text,
        egui::text::TextFormat {
            font_id: font,
            color,
            extra_letter_spacing: tracking,
            ..Default::default()
        },
    )
}

/// [`paint_tracked`] with a full [`egui::text::TextFormat`] (italics, …).
pub fn paint_tracked_format(
    painter: &egui::Painter,
    anchor: egui::Align2,
    center: egui::Pos2,
    text: &str,
    format: egui::text::TextFormat,
) -> Rect {
    let mut job = egui::text::LayoutJob::default();
    let color = format.color;
    job.append(text, 0.0, format);
    let galley = painter.layout_job(job);
    let rect = anchor.anchor_size(center, galley.size());
    painter.galley(rect.min, galley, color);
    rect
}

/* ---------------- logo + avatars ---------------- */

/// The Automata mark: a browser window with three cells and an amber cursor.
pub fn paint_logo(painter: &egui::Painter, rect: Rect, color: Color32) {
    let scale = rect.width() / 48.0;
    if scale <= 0.0 {
        return;
    }
    let stroke = Stroke::new((2.5 * scale).max(1.2), color);

    painter.rect_stroke(
        rect.shrink(1.5 * scale),
        egui::CornerRadius::ZERO,
        stroke,
        egui::StrokeKind::Inside,
    );
    painter.line_segment(
        [
            egui::pos2(rect.left() + 1.5 * scale, rect.top() + 11.0 * scale),
            egui::pos2(rect.right() - 1.5 * scale, rect.top() + 11.0 * scale),
        ],
        stroke,
    );

    let cell = 7.0 * scale;
    let top = rect.top() + 16.0 * scale;
    painter.rect_filled(
        egui::Rect::from_min_size(
            egui::pos2(rect.left() + (24.0 * scale - cell / 2.0), top),
            egui::vec2(cell, cell),
        ),
        0.0,
        color,
    );
    painter.rect_filled(
        egui::Rect::from_min_size(
            egui::pos2(rect.left() + 14.0 * scale, top + cell + 3.0 * scale),
            egui::vec2(cell, cell),
        ),
        0.0,
        color,
    );
    painter.rect_filled(
        egui::Rect::from_min_size(
            egui::pos2(rect.left() + 27.0 * scale, top + cell + 3.0 * scale),
            egui::vec2(cell, cell),
        ),
        0.0,
        AMBER,
    );

    let cursor = vec![
        egui::pos2(rect.left() + 33.0 * scale, rect.top() + 24.0 * scale),
        egui::pos2(rect.left() + 33.0 * scale, rect.top() + 36.0 * scale),
        egui::pos2(rect.left() + 36.0 * scale, rect.top() + 33.0 * scale),
        egui::pos2(rect.left() + 38.5 * scale, rect.top() + 38.0 * scale),
        egui::pos2(rect.left() + 41.0 * scale, rect.top() + 36.8 * scale),
        egui::pos2(rect.left() + 38.5 * scale, rect.top() + 31.8 * scale),
        egui::pos2(rect.left() + 43.0 * scale, rect.top() + 31.3 * scale),
    ];
    painter.add(egui::Shape::convex_polygon(cursor, AMBER, Stroke::NONE));
}

pub fn paint_assistant_avatar(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(30.0, 25.0), Sense::hover());
    paint_logo(ui.painter(), rect.shrink2(Vec2::new(2.0, 1.0)), NAVY);
}

pub fn paint_user_avatar(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(30.0, 25.0), Sense::hover());
    let center = rect.center();
    ui.painter()
        .circle_filled(center, 8.0, with_alpha(AMBER, 41));
    ui.painter().circle_filled(center, 5.0, AMBER);
}

/* ---------------- widgets ---------------- */

/// Amber / outline pill used by the THINKING level chips.
pub fn chip_toggle(ui: &mut Ui, label: &str, active: bool) -> bool {
    let text_color = if active { DARK_TEXT } else { MUTED };
    let fill = if active { AMBER } else { Color32::TRANSPARENT };
    let stroke = if active {
        Stroke::NONE
    } else {
        Stroke::new(1.0, LINE)
    };
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .font(mono_font(9.0))
                .color(text_color),
        )
        .fill(fill)
        .stroke(stroke)
        .corner_radius(255)
        .min_size(Vec2::new(0.0, 24.0)),
    )
    .clicked()
}

/// Hero suggestion chip (`.chip` in app.css).
pub fn prompt_chip(ui: &mut Ui, label: &str) -> bool {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .font(body_font(12.0))
                .color(NAVY_SOFT),
        )
        .fill(CHIP_FILL)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(255)
        .min_size(Vec2::new(0.0, 34.0)),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
    .clicked()
}

/// Sidebar `.ghost-btn` (uppercase mono, left aligned).
pub fn ghost_button(ui: &mut Ui, label: &str) -> bool {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::click());
    if resp.hovered() || resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, 8.0, WHITE_05);
    }
    ui.painter().text(
        rect.left_center() + Vec2::new(11.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label.to_uppercase(),
        mono_font(9.5),
        if resp.hovered() { CREAM } else { INK_MUTED },
    );
    resp.clicked()
}

/// Header `.win-btn` (30x28, radius 7).
pub fn win_btn(ui: &mut Ui, glyph: &str, close_style: bool) -> bool {
    win_btn_draw(ui, close_style, |painter, rect, color| {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            mono_font(14.0),
            color,
        );
    })
}

/// Same chrome as [`win_btn`], but the glyph is drawn by the caller.
///
/// Needed for symbols the default egui fonts do not carry, like `U+2630` `☰`.
pub fn win_btn_draw(
    ui: &mut Ui,
    close_style: bool,
    draw: impl FnOnce(&egui::Painter, Rect, Color32),
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(30.0, 28.0), Sense::click());
    let hot = resp.hovered() || resp.is_pointer_button_down_on();
    if hot {
        ui.painter()
            .rect_filled(rect, 7.0, if close_style { DANGER_TINT } else { WHITE_08 });
    }
    let color = if hot {
        if close_style {
            RED
        } else {
            NAVY
        }
    } else {
        MUTED
    };
    draw(ui.painter(), rect, color);
    resp.clicked()
}
