mod app;
mod window;
mod webview;
mod sidebar;
mod toolbar;

use app::BitterApp;
use gtk4::prelude::*;
use libadwaita as adw;

fn main() {
    tracing_subscriber::fmt::init();

    let app = BitterApp::new();
    app.run();
}
