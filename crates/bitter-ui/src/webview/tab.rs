use gtk4::prelude::*;
use webkit6::prelude::*;
use webkit6::WebView;

pub struct Tab {
    webview: WebView,
    id: String,
}

impl Tab {
    pub fn new(url: &str) -> Self {
        let webview = WebView::new();
        webview.load_uri(url);
        webview.set_hexpand(true);
        webview.set_vexpand(true);

        Self {
            webview,
            id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.webview.upcast_ref()
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn title(&self) -> Option<String> {
        self.webview.title().map(|t| t.to_string())
    }
}
