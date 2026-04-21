mod app;
mod command_palette;
mod sidebar;
mod style;
mod toolbar;
mod webview;
mod window;

use app::BitterApp;

fn main() {
    tracing_subscriber::fmt::init();

    let app = BitterApp::new();
    app.run();
}
