const APP_CSS: &str = r#"
window {
  background: #0d0d0d;
  color: #f0f0f0;
}

.sidebar {
  background: #141414;
  border-right: 1px solid #2a2a2a;
}

.tab-list {
  background: transparent;
}

.tab-item {
  min-height: 36px;
  border-radius: 6px;
  margin: 2px 8px;
}

.tab-item:selected {
  background: #1a1a1a;
  box-shadow: inset 3px 0 #ff4d00;
}

.tab-item label {
  font-size: 13px;
}

button {
  border-radius: 6px;
}

button:hover {
  background: #1a1a1a;
}

.omnibar {
  min-height: 34px;
  border-radius: 8px;
  border: 1px solid #2a2a2a;
  background: #141414;
  color: #f0f0f0;
  padding: 0 12px;
}

.omnibar:focus {
  border-color: #ff4d00;
  box-shadow: 0 0 0 1px alpha(#ff4d00, 0.35);
}

.command-palette {
  background: #1a1a1a;
  border-radius: 8px;
  border: 1px solid #2a2a2a;
}

.command-palette searchentry {
  min-height: 40px;
  border-radius: 8px;
  border: 1px solid #2a2a2a;
  background: #141414;
  color: #f0f0f0;
}

.boxed-list {
  background: #141414;
  border-radius: 8px;
}

.boxed-list row {
  min-height: 42px;
}

.boxed-list row:selected {
  background: #26211f;
  color: #f0f0f0;
}
"#;

pub fn load() {
    let Some(display) = gtk4::gdk::Display::default() else {
        tracing::warn!("Cannot load application CSS before a GTK display is available");
        return;
    };

    let provider = gtk4::CssProvider::new();
    provider.load_from_string(APP_CSS);
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
