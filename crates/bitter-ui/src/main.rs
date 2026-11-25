mod app;
mod command_palette;
mod content_blocking;
mod default_browser;
mod downloads;
mod find_bar;
mod https_only;
mod internal_pages;
mod preferences;
mod session_writer;
mod sidebar;
mod style;
mod suggestion_worker;
mod toolbar;
mod webview;
mod window;

use app::BitterApp;

fn main() -> gtk4::glib::ExitCode {
    tracing_subscriber::fmt::init();

    let app = BitterApp::new();
    app.run()
}
