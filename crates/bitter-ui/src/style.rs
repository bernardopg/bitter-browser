const APP_CSS: &str = r#"
/* Motion: short, eased transitions; GTK disables them when animations are off. */

/* Window chrome: one continuous surface for the toolbar and sidebar. */
.browser-chrome headerbar {
  background-color: @window_bg_color;
  box-shadow: none;
  border-bottom: 1px solid alpha(currentColor, 0.07);
  min-height: 50px;
}

.browser-chrome headerbar button {
  border-radius: 9px;
  transition: background-color 150ms ease, opacity 150ms ease;
}

.browser-chrome headerbar button:disabled {
  opacity: 0.35;
}

/* Loading progress: a thin accent line that fades in and out. */
.navigation-progress {
  opacity: 0;
  transition: opacity 320ms ease;
}

.navigation-progress.loading {
  opacity: 1;
  transition: opacity 120ms ease;
}

.navigation-progress trough,
.navigation-progress progress {
  min-height: 2px;
  border: none;
  border-radius: 0 2px 2px 0;
}

.navigation-progress trough {
  background: transparent;
}

.navigation-progress progress {
  background-image: linear-gradient(to right, alpha(@accent_bg_color, 0.55), @accent_bg_color);
  transition: min-width 200ms ease;
}

/* Address bar. */
.omnibar {
  min-height: 36px;
  border-radius: 11px;
  border: none;
  background-color: alpha(currentColor, 0.065);
  color: @window_fg_color;
  padding: 0 12px;
  box-shadow: inset 0 0 0 1px transparent;
  transition: background-color 160ms ease, box-shadow 160ms ease;
}

.omnibar:hover {
  background-color: alpha(currentColor, 0.09);
}

.omnibar:focus-within {
  background-color: @view_bg_color;
  box-shadow: inset 0 0 0 2px alpha(@accent_color, 0.75), 0 0 0 4px alpha(@accent_color, 0.14);
}

.omnibar image {
  opacity: 0.6;
  margin-right: 4px;
  transition: opacity 160ms ease, color 160ms ease;
}

.omnibar.secure image {
  color: @success_color;
  opacity: 0.9;
}

.omnibar.insecure image {
  color: @warning_color;
  opacity: 1;
}

/* Sidebar. */
.browser-chrome .sidebar {
  background-color: @sidebar_bg_color;
  color: @sidebar_fg_color;
  border-right: 1px solid alpha(currentColor, 0.07);
}

.workspace-switcher {
  padding: 3px;
  border-radius: 12px;
  background-color: alpha(currentColor, 0.065);
}

.workspace-switcher button {
  min-height: 28px;
  border-radius: 9px;
  border: none;
  background: transparent;
  box-shadow: none;
  font-weight: 600;
  opacity: 0.7;
  transition: background-color 180ms ease, box-shadow 180ms ease, opacity 180ms ease;
}

.workspace-switcher button:hover {
  opacity: 1;
}

.workspace-switcher button:checked {
  opacity: 1;
  background-color: mix(@window_bg_color, white, 0.12);
  box-shadow: 0 1px 3px alpha(black, 0.12), 0 0 0 1px alpha(currentColor, 0.04);
}

.sidebar .new-tab {
  min-height: 34px;
  border-radius: 10px;
  font-weight: 500;
  opacity: 0.85;
  transition: background-color 150ms ease, opacity 150ms ease;
}

.sidebar .new-tab:hover {
  opacity: 1;
}

.sidebar separator {
  background-color: transparent;
  min-height: 0;
}

.tab-list {
  background-color: transparent;
}

.tab-item {
  min-height: 38px;
  border-radius: 10px;
  margin: 1px 8px;
  transition: background-color 160ms ease, box-shadow 160ms ease;
}

.tab-item:hover {
  background-color: alpha(currentColor, 0.055);
}

.tab-item:selected,
.tab-item:selected:hover {
  background-color: alpha(@accent_bg_color, 0.16);
  box-shadow: inset 0 0 0 1px alpha(@accent_bg_color, 0.22);
}

.tab-item .tab-title {
  font-size: 0.95em;
  opacity: 0.86;
  transition: opacity 160ms ease;
}

.tab-item:selected .tab-title {
  opacity: 1;
  font-weight: 600;
}

.tab-item .tab-close {
  min-width: 24px;
  min-height: 24px;
  padding: 0;
  opacity: 0;
  transition: opacity 140ms ease, background-color 140ms ease;
}

.tab-item:hover .tab-close,
.tab-item:selected .tab-close,
.tab-item .tab-close:focus-visible {
  opacity: 0.7;
}

.tab-item .tab-close:hover {
  opacity: 1;
  background-color: alpha(currentColor, 0.1);
}

/* Find bar. */
.find-bar > revealer > box {
  background-color: @window_bg_color;
  border-bottom: 1px solid alpha(currentColor, 0.07);
  box-shadow: none;
}

.find-bar entry {
  border-radius: 10px;
}

/* Address suggestions. */
.omnibar-suggestions > contents {
  border-radius: 14px;
  padding: 6px;
}

.omnibar-suggestions list {
  background-color: transparent;
}

.omnibar-suggestions row {
  min-height: 48px;
  border-radius: 9px;
  margin: 1px 0;
  transition: background-color 120ms ease;
}

.omnibar-suggestion-detail {
  font-size: 0.9em;
  opacity: 0.7;
}

/* Downloads. */
.downloads-active {
  color: @accent_color;
}

.downloads-popover > contents {
  border-radius: 16px;
}

.download-list {
  background: transparent;
}

.download-row {
  transition: background-color 140ms ease;
}

.download-row .download-name {
  font-weight: 600;
}

.download-row .download-icon {
  opacity: 0.8;
}

.download-failed .download-icon,
.download-failed .download-name {
  opacity: 0.55;
}

.download-progress trough,
.download-progress progress {
  min-height: 4px;
  border-radius: 99px;
}

.downloads-empty {
  padding: 12px 0;
}

/* Command palette. */
.command-palette {
  background-color: @window_bg_color;
  color: @window_fg_color;
  border-radius: 16px;
  border: 1px solid alpha(currentColor, 0.08);
  box-shadow: 0 18px 48px alpha(black, 0.28);
}

.command-palette searchentry {
  min-height: 42px;
  border-radius: 12px;
}

.command-palette .boxed-list row {
  min-height: 42px;
  transition: background-color 120ms ease;
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
