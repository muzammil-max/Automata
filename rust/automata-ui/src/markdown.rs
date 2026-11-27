//! Markdown renderer for assistant messages.
//!
//! Supports the subset that matters in chat: paragraphs, headings, lists,
//! quotes, fenced code, tables, links, emphasis, inline code, and TeX math
//! (via `pulldown-cmark`'s math extension + [`crate::latex`]).

use egui::{Color32, RichText, Ui};
use pulldown_cmark::{
    CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};

use crate::latex;
use crate::theme::{
    body_font, mono_font, AMBER, AMBER_LINE, CHIP_FILL, CREAM, INK_2, LINE, MUTED,
    NAVY, NAVY_SOFT, PAPER_2,
};

const LINK: Color32 = Color32::from_rgb(0x7c, 0xb0, 0xff);
const CODE_BG: Color32 = Color32::from_rgba_premultiplied(12, 22, 38, 120);

#[derive(Clone, PartialEq, Default)]
struct TextStyle {
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
    muted: bool,
    link: Option<String>,
}

#[derive(Clone)]
enum PieceKind {
    Text(String),
    Math(String),
}

#[derive(Clone)]
struct Piece {
    kind: PieceKind,
    style: TextStyle,
}

struct ListState {
    ordered: bool,
    next: u64,
}

#[derive(Default)]
struct TableState {
    rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    cell: String,
    in_cell: bool,
    in_head: bool,
    header_rows: usize,
    columns: usize,
}

enum Flush {
    None,
    Paragraph,
    Heading(u32),
    Item,
}

struct Renderer<'a> {
    color: Color32,
    inline: Vec<Piece>,
    strong: u32,
    emphasis: u32,
    strike: u32,
    link: Option<String>,
    image_alt: Option<String>,
    quote_depth: usize,
    list_stack: Vec<ListState>,
    item_marker: Option<String>,
    code_block: Option<String>,
    code_lang: Option<String>,
    table: Option<TableState>,
    table_seq: usize,
    _marker: std::marker::PhantomData<&'a ()>,
}

pub fn render(ui: &mut Ui, text: &str, color: Color32) {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_MATH);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut renderer = Renderer {
        color,
        inline: Vec::new(),
        strong: 0,
        emphasis: 0,
        strike: 0,
        link: None,
        image_alt: None,
        quote_depth: 0,
        list_stack: Vec::new(),
        item_marker: None,
        code_block: None,
        code_lang: None,
        table: None,
        table_seq: 0,
        _marker: std::marker::PhantomData,
    };

    for event in Parser::new_ext(text, options) {
        renderer.event(ui, event);
    }
    renderer.flush(ui, Flush::Paragraph);
}

impl<'a> Renderer<'a> {
    fn event(&mut self, ui: &mut Ui, event: Event<'a>) {
        match event {
            Event::Start(tag) => self.start(ui, tag),
            Event::End(tag) => self.end(ui, tag),
            Event::Text(text) => {
                if let Some(code) = &mut self.code_block {
                    code.push_str(&text);
                } else if let Some(alt) = &mut self.image_alt {
                    alt.push_str(&text);
                } else if let Some(table) = &mut self.table {
                    if table.in_cell {
                        table.cell.push_str(&text);
                    }
                } else {
                    self.push_text(&text);
                }
            }
            Event::Code(code) => {
                if let Some(table) = &mut self.table {
                    if table.in_cell {
                        table.cell.push_str(&code);
                    }
                } else {
                    let mut style = self.style();
                    style.code = true;
                    self.push_piece(Piece {
                        kind: PieceKind::Text(code.to_string()),
                        style,
                    });
                }
            }
            Event::InlineMath(tex) => {
                if let Some(table) = &mut self.table {
                    if table.in_cell {
                        table.cell.push_str(&latex::to_unicode(&tex));
                    }
                } else {
                    self.push_piece(Piece {
                        kind: PieceKind::Math(tex.to_string()),
                        style: self.style(),
                    });
                }
            }
            Event::DisplayMath(tex) => {
                if let Some(table) = &mut self.table {
                    if table.in_cell {
                        table.cell.push_str(&latex::to_unicode(&tex));
                    }
                } else {
                    self.flush(ui, Flush::None);
                    latex::display(ui, &tex, self.color);
                }
            }
            Event::SoftBreak => {
                if let Some(code) = &mut self.code_block {
                    code.push('\n');
                } else if let Some(table) = &mut self.table {
                    if table.in_cell {
                        table.cell.push(' ');
                    }
                } else if self.image_alt.is_none() {
                    self.push_text(" ");
                }
            }
            Event::HardBreak => {
                if let Some(code) = &mut self.code_block {
                    code.push('\n');
                } else if let Some(table) = &mut self.table {
                    if table.in_cell {
                        table.cell.push(' ');
                    }
                } else if self.image_alt.is_none() {
                    self.push_text(" ");
                }
            }
            Event::Rule => {
                self.flush(ui, Flush::None);
                ui.add_space(5.0);
                ui.separator();
                ui.add_space(6.0);
            }
            Event::TaskListMarker(checked) => {
                self.item_marker = None;
                self.push_text(if checked { "☑ " } else { "☐ " });
            }
            Event::Html(_) | Event::InlineHtml(_) | Event::FootnoteReference(_) => {}
        }
    }

    fn start(&mut self, ui: &mut Ui, tag: Tag<'a>) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { .. } => {}
            Tag::BlockQuote(_) => {
                self.flush(ui, Flush::None);
                self.quote_depth += 1;
            }
            Tag::CodeBlock(kind) => {
                self.flush(ui, Flush::None);
                self.code_block = Some(String::new());
                self.code_lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let lang = lang.trim().to_string();
                        if lang.is_empty() {
                            None
                        } else {
                            Some(lang)
                        }
                    }
                    CodeBlockKind::Indented => None,
                };
            }
            Tag::List(start) => {
                self.flush(ui, Flush::None);
                self.list_stack.push(ListState {
                    ordered: start.is_some(),
                    next: start.unwrap_or(1),
                });
            }
            Tag::Item => {
                self.flush(ui, Flush::None);
                let marker = if let Some(state) = self.list_stack.last_mut() {
                    if state.ordered {
                        let n = state.next;
                        state.next += 1;
                        format!("{}.", n)
                    } else {
                        "•".to_string()
                    }
                } else {
                    "•".to_string()
                };
                self.item_marker = Some(marker);
            }
            Tag::Table(alignments) => {
                self.flush(ui, Flush::None);
                self.table = Some(TableState {
                    columns: alignments.len(),
                    ..Default::default()
                });
            }
            Tag::TableHead => {
                if let Some(table) = &mut self.table {
                    table.in_head = true;
                }
            }
            Tag::TableRow => {
                if let Some(table) = &mut self.table {
                    table.current_row.clear();
                }
            }
            Tag::TableCell => {
                if let Some(table) = &mut self.table {
                    table.cell.clear();
                    table.in_cell = true;
                }
            }
            Tag::Emphasis => self.emphasis += 1,
            Tag::Strong => self.strong += 1,
            Tag::Strikethrough => self.strike += 1,
            Tag::Link { dest_url, .. } => {
                self.link = Some(dest_url.to_string());
            }
            Tag::Image { .. } => {
                self.image_alt = Some(String::new());
            }
            _ => {}
        }
    }

    fn end(&mut self, ui: &mut Ui, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush(ui, Flush::Paragraph),
            TagEnd::Heading(level) => {
                let level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                self.flush(ui, Flush::Heading(level));
            }
            TagEnd::BlockQuote(_) => {
                self.flush(ui, Flush::None);
                self.quote_depth = self.quote_depth.saturating_sub(1);
                ui.add_space(4.0);
            }
            TagEnd::CodeBlock => {
                if let Some(code) = self.code_block.take() {
                    self.render_code_block(ui, &code);
                }
                self.code_lang = None;
            }
            TagEnd::List(_) => {
                self.flush(ui, Flush::None);
                self.list_stack.pop();
                if self.list_stack.is_empty() {
                    ui.add_space(4.0);
                }
            }
            TagEnd::Item => {
                self.flush(ui, Flush::Item);
                self.item_marker = None;
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    self.table_seq += 1;
                    render_table(ui, &table, self.table_seq);
                }
            }
            TagEnd::TableHead => {
                if let Some(table) = &mut self.table {
                    table.in_head = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(table) = &mut self.table {
                    let row = std::mem::take(&mut table.current_row);
                    if !row.is_empty() {
                        table.rows.push(row);
                    }
                    if table.in_head {
                        table.header_rows = table.rows.len();
                    }
                }
            }
            TagEnd::TableCell => {
                if let Some(table) = &mut self.table {
                    let cell = std::mem::take(&mut table.cell);
                    table.current_row.push(cell);
                    table.in_cell = false;
                }
            }
            TagEnd::Emphasis => self.emphasis = self.emphasis.saturating_sub(1),
            TagEnd::Strong => self.strong = self.strong.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            TagEnd::Link => self.link = None,
            TagEnd::Image => {
                if let Some(alt) = self.image_alt.take() {
                    let alt = alt.trim();
                    if !alt.is_empty() {
                        self.push_piece(Piece {
                            kind: PieceKind::Text(format!("[{}]", alt)),
                            style: TextStyle {
                                italic: true,
                                muted: true,
                                ..Default::default()
                            },
                        });
                    }
                }
            }
            _ => {}
        }
    }

    fn style(&self) -> TextStyle {
        TextStyle {
            bold: self.strong > 0,
            italic: self.emphasis > 0,
            strike: self.strike > 0,
            code: false,
            muted: false,
            link: self.link.clone(),
        }
    }

    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let style = self.style();
        self.push_piece(Piece {
            kind: PieceKind::Text(text.to_string()),
            style,
        });
    }

    fn push_piece(&mut self, piece: Piece) {
        if let Some(last) = self.inline.last_mut() {
            if last.style == piece.style {
                if let (PieceKind::Text(existing), PieceKind::Text(extra)) =
                    (&mut last.kind, &piece.kind)
                {
                    existing.push_str(extra);
                    return;
                }
            }
        }
        self.inline.push(piece);
    }

    fn flush(&mut self, ui: &mut Ui, kind: Flush) {
        if self.inline.is_empty() {
            return;
        }

        let pieces = std::mem::take(&mut self.inline);
        let marker = self.item_marker.take();
        let indent = self.list_stack.len().saturating_sub(1) as f32 * 16.0;
        let quote = self.quote_depth;
        let color = if quote > 0 { NAVY_SOFT } else { self.color };
        let heading = matches!(&kind, Flush::Heading(_));
        let (base_size, spacing) = match kind {
            Flush::Heading(level) => (heading_size(level), 10.0),
            Flush::Item => (14.0, 3.0),
            _ => (
                14.0,
                if self.list_stack.is_empty() {
                    7.0
                } else {
                    3.0
                },
            ),
        };

        let response = ui.horizontal_top(|ui| {
            if quote > 0 {
                ui.add_space(14.0);
            }
            if indent > 0.0 {
                ui.add_space(indent);
            }
            if let Some(marker) = marker {
                ui.label(
                    RichText::new(marker)
                        .font(body_font(13.0))
                        .color(AMBER),
                );
                ui.add_space(6.0);
            }
            ui.vertical(|ui| {
                render_pieces(ui, &pieces, base_size, color, heading);
            });
        });

        if quote > 0 {
            let rect = response.response.rect;
            ui.painter().vline(
                rect.left() + 3.0,
                rect.y_range(),
                egui::Stroke::new(2.0, AMBER_LINE),
            );
        }

        if spacing > 0.0 {
            ui.add_space(spacing);
        }
    }

    fn render_code_block(&mut self, ui: &mut Ui, code: &str) {
        ui.add_space(5.0);
        egui::Frame::new()
            .fill(INK_2)
            .stroke(egui::Stroke::new(1.0, LINE))
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                if let Some(lang) = &self.code_lang {
                    ui.label(
                        RichText::new(lang.to_uppercase())
                            .font(mono_font(9.5))
                            .color(MUTED),
                    );
                    ui.add_space(4.0);
                }
                ui.add(
                    egui::Label::new(
                        RichText::new(code.trim_end())
                            .font(mono_font(12.5))
                            .color(CREAM),
                    )
                    .wrap(),
                );
            });
        ui.add_space(7.0);
    }
}

fn heading_size(level: u32) -> f32 {
    match level {
        1 => 23.0,
        2 => 20.0,
        3 => 17.5,
        4 => 16.0,
        5 => 14.5,
        _ => 13.5,
    }
}

fn render_pieces(
    ui: &mut Ui,
    pieces: &[Piece],
    base_size: f32,
    color: Color32,
    heading: bool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 3.0);
        for piece in pieces {
            match &piece.kind {
                PieceKind::Text(text) => {
                    if text.is_empty() {
                        continue;
                    }
                    let mut rich = RichText::new(text)
                        .font(body_font(base_size))
                        .color(if piece.style.link.is_some() {
                            LINK
                        } else if piece.style.muted {
                            MUTED
                        } else {
                            color
                        });
                    if piece.style.bold || heading {
                        rich = rich.strong();
                    }
                    if piece.style.italic {
                        rich = rich.italics();
                    }
                    if piece.style.strike {
                        rich = rich.strikethrough();
                    }
                    if piece.style.code {
                        rich = rich
                            .monospace()
                            .background_color(CODE_BG)
                            .color(CREAM);
                    }
                    if piece.style.link.is_some() {
                        rich = rich.underline();
                    }

                    let sense = if piece.style.link.is_some() {
                        egui::Sense::click()
                    } else {
                        egui::Sense::hover()
                    };
                    let response = ui.add(egui::Label::new(rich).wrap().sense(sense));
                    if let Some(url) = &piece.style.link {
                        if response.clicked() {
                            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                        }
                        response.on_hover_cursor(egui::CursorIcon::PointingHand);
                    }
                }
                PieceKind::Math(tex) => latex::inline(ui, tex, color),
            }
        }
    });
}

fn render_table(ui: &mut Ui, table: &TableState, seq: usize) {
    if table.rows.is_empty() {
        return;
    }

    ui.add_space(5.0);
    egui::Frame::new()
        .fill(PAPER_2)
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            egui::Grid::new(("md-table", seq))
                .striped(true)
                .num_columns(table.columns.max(1))
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    for (row_index, row) in table.rows.iter().enumerate() {
                        let header = row_index < table.header_rows;
                        for cell in row {
                            let mut rich = RichText::new(cell)
                                .font(body_font(12.5))
                                .color(if header { AMBER } else { NAVY });
                            if header {
                                rich = rich.strong();
                            }
                            ui.label(rich);
                        }
                        ui.end_row();
                    }
                });
        });
    ui.add_space(7.0);
}

#[allow(dead_code)]
fn chip(ui: &mut Ui, text: &str) {
    egui::Frame::new()
        .fill(CHIP_FILL)
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).font(mono_font(10.0)).color(MUTED));
        });
}
