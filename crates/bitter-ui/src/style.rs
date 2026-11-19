const APP_CSS: &str = r#"
.navigation-progress trough,
.navigation-progress progress {
  min-height: 2px;
  border: none;
  border-radius: 0;
}

.workspace-switcher button {
  min-height: 30px;
}

.browser-chrome .sidebar {
  background-color: @sidebar_bg_color;
  border-right: 1px solid @sidebar_border_color;
}

.tab-list {
  background-color: transparent;
}

.tab-item {
  min-height: 36px;
  border-radius: 8px;
  margin: 2px 8px;
}

.tab-item:selected {
  background-color: alpha(@accent_color, 0.14);
  box-shadow: inset 3px 0 @accent_color;
}

.omnibar {
  min-height: 34px;
  border-radius: 8px;
  border: 1px solid @borders;
  background-color: @view_bg_color;
  color: @view_fg_color;
  padding: 0 12px;
}

.omnibar:focus-within {
  border-color: @accent_color;
  box-shadow: 0 0 0 1px alpha(@accent_color, 0.35);
}

.omnibar-suggestions {
  padding: 6px;
}

.omnibar-suggestions list {
  background-color: transparent;
}

.omnibar-suggestions row {
  min-height: 48px;
  border-radius: 6px;
  margin: 1px 0;
}

.omnibar-suggestion-detail {
  font-size: 0.9em;
  opacity: 0.78;
}

.command-palette {
  background-color: @window_bg_color;
  color: @window_fg_color;
  border-radius: 12px;
  border: 1px solid @borders;
}

.command-palette searchentry {
  min-height: 40px;
}

.command-palette .boxed-list row {
  min-height: 42px;
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
