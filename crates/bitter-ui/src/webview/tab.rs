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

    pub fn uri(&self) -> Option<String> {
        self.webview.uri().map(|uri| uri.to_string())
    }

    pub fn load_uri(&self, uri: &str) {
        self.webview.load_uri(uri);
    }

    pub fn load_html(&self, html: &str, base_uri: &str) {
        self.webview.load_html(html, Some(base_uri));
    }

    pub fn reload(&self) {
        self.webview.reload();
    }

    pub fn go_back(&self) {
        self.webview.go_back();
    }

    pub fn go_forward(&self) {
        self.webview.go_forward();
    }

    pub fn connect_title_changed<F: Fn(&str) + 'static>(&self, f: F) {
        self.webview.connect_title_notify(move |webview| {
            if let Some(title) = webview.title() {
                f(title.as_str());
            }
        });
    }

    pub fn connect_uri_changed<F: Fn(&str) + 'static>(&self, f: F) {
        self.webview.connect_uri_notify(move |webview| {
            if let Some(uri) = webview.uri() {
                f(uri.as_str());
            }
        });
    }

    pub fn connect_metadata_changed<F: Fn(Option<String>, Option<String>) + 'static>(&self, f: F) {
        let f = std::rc::Rc::new(f);

        let f_title = f.clone();
        self.webview.connect_title_notify(move |webview| {
            f_title(
                webview.uri().map(|uri| uri.to_string()),
                webview.title().map(|title| title.to_string()),
            );
        });

        let f_uri = f.clone();
        self.webview.connect_uri_notify(move |webview| {
            f_uri(
                webview.uri().map(|uri| uri.to_string()),
                webview.title().map(|title| title.to_string()),
            );
        });
    }
}
