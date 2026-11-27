//! Modal dialogs — a faithful port of `.dialog*` in `frontend/css/app.css`.

use crate::theme::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Intent {
    None,
    CloseBrowser,
    SafetyContinue,
    SafetyCancel,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DialogOutcome {
    Dismiss,
    Action(Intent),
}

#[derive(Clone)]
pub struct Dialog {
    title: String,
    message: String,
    primary: String,
    primary_intent: Intent,
    secondary: Option<String>,
}

impl Dialog {
    /// Plain acknowledgement ("OK").
    pub fn info(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            primary: "OK".into(),
            primary_intent: Intent::None,
            secondary: None,
        }
    }

    /// Two-button confirmation; `secondary` is the safe way out.
    pub fn confirm(
        title: impl Into<String>,
        message: impl Into<String>,
        primary: &str,
        primary_intent: Intent,
        secondary: &str,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            primary: primary.into(),
            primary_intent,
            secondary: Some(secondary.into()),
        }
    }
}

/// Draw the dialog for one frame.
///
/// Returns `None` while it is still open, otherwise what the user picked.
pub fn show(ctx: &egui::Context, dialog: &Dialog) -> Option<DialogOutcome> {
    let mut outcome: Option<DialogOutcome> = None;

    let response = egui::Modal::new(egui::Id::unique("automata-dialog"))
        .backdrop_color(BACKDROP)
        .frame(
            egui::Frame::new()
                .fill(PAPER_2)
                .stroke(egui::Stroke::new(1.0, LINE))
                .corner_radius(18)
                .inner_margin(egui::Margin::same(22)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(396.0);
            ui.set_max_width(396.0);

            ui.label(
                egui::RichText::new(&dialog.title)
                    .font(display_font(26.0))
                    .color(NAVY),
            );
            ui.add_space(10.0);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(&dialog.message)
                        .font(body_font(13.5))
                        .color(NAVY_SOFT),
                )
                .wrap(),
            );
            ui.add_space(20.0);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().button_padding = egui::vec2(16.0, 9.0);

                if ui
                    .add(
                        egui::Button::new(dialog_label(&dialog.primary, 10.0, DARK_TEXT))
                            .fill(AMBER)
                            .stroke(egui::Stroke::new(1.0, AMBER))
                            .corner_radius(10),
                    )
                    .clicked()
                {
                    outcome = Some(DialogOutcome::Action(dialog.primary_intent));
                }

                if let Some(secondary) = &dialog.secondary {
                    ui.add_space(8.0);
                    if ui
                        .add(
                            egui::Button::new(dialog_label(secondary, 10.0, NAVY))
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::new(1.0, LINE))
                                .corner_radius(10),
                        )
                        .clicked()
                    {
                        outcome = Some(DialogOutcome::Dismiss);
                    }
                }
            });
        });

    if outcome.is_some() {
        return outcome;
    }

    if response.should_close() {
        // Backdrop click / Escape. A paused safety prompt must never be
        // abandoned silently — it cancels instead.
        return Some(if dialog.primary_intent == Intent::SafetyContinue {
            DialogOutcome::Action(Intent::SafetyCancel)
        } else {
            DialogOutcome::Dismiss
        });
    }

    None
}
