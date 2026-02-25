pub mod tab;

use gtk4::prelude::*;
use webkit6::prelude::*;
use webkit6::WebView;

pub struct BitterWebView {
    webview: WebView,
}

impl BitterWebView {
    pub fn new() -> Self {
        let webview = WebView::new();
        webview.load_uri("https://duckduckgo.com");
        webview.set_hexpand(true);
        webview.set_vexpand(true);

        Self { webview }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.webview.upcast_ref()
    }
}
