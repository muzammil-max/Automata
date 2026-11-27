mod api;
mod app;
mod dialog;
mod latex;
mod launch;
mod markdown;
mod model;
mod splash;
mod theme;

fn main() -> eframe::Result<()> {
    app::run()
}
