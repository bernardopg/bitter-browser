use crate::command_palette::CommandPalette;
use crate::sidebar::tab_list::TabList;
use crate::sidebar::Sidebar;
use crate::toolbar::omnibar::Omnibar;
use crate::toolbar::Toolbar;
use crate::webview::tab::Tab;
use bitter_core::bookmarks::Bookmarks;
use bitter_core::history::History;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

const HOME_URI: &str = "https://duckduckgo.com";

pub struct BitterWindow {
    window: adw::ApplicationWindow,
    _command_palette: Rc<CommandPalette>,
}

struct BrowserData {
    history: Option<History>,
    bookmarks: Option<Bookmarks>,
}

impl BitterWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Bitter Browser")
            .default_width(1200)
            .default_height(800)
            .build();

        let command_palette = Rc::new(CommandPalette::new(&window));
        let sidebar = Sidebar::new();
        let toolbar = Toolbar::new();

        let main_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .build();

        let content_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();

        let stack = gtk4::Stack::builder().hexpand(true).vexpand(true).build();

        content_box.append(toolbar.widget());
        content_box.append(&stack);

        main_box.append(sidebar.widget());
        main_box.append(&content_box);
        window.set_content(Some(&main_box));

        let tabs: Rc<RefCell<Vec<Tab>>> = Rc::new(RefCell::new(Vec::new()));
        let tab_list = sidebar.tab_list().clone();
        let omnibar = toolbar.omnibar().clone();
        let data = Rc::new(BrowserData {
            history: History::new()
                .map_err(|err| tracing::warn!("Failed to initialize history: {err}"))
                .ok(),
            bookmarks: Bookmarks::new()
                .map_err(|err| tracing::warn!("Failed to initialize bookmarks: {err}"))
                .ok(),
        });

        open_tab(
            HOME_URI,
            "DuckDuckGo",
            &tab_list.active_workspace(),
            &tab_list,
            &stack,
            &tabs,
            &omnibar,
            &data,
        );

        {
            let tabs = tabs.clone();
            let stack = stack.clone();
            tab_list.connect_tab_selected(move |id| {
                stack.set_visible_child_name(id);
                with_tab(&tabs, id, |tab| {
                    if let Some(uri) = tab.uri() {
                        omnibar.set_text(&uri);
                    }
                });
            });
        }

        {
            let tabs = tabs.clone();
            let stack = stack.clone();
            let tab_list = tab_list.clone();
            let omnibar = toolbar.omnibar().clone();
            let data = data.clone();
            let tab_list_for_handler = tab_list.clone();
            tab_list.connect_tab_closed(move |id| {
                close_tab(id, &tab_list_for_handler, &stack, &tabs, &omnibar, &data);
            });
        }

        {
            let tabs = tabs.clone();
            let stack = stack.clone();
            let tab_list = tab_list.clone();
            let omnibar = toolbar.omnibar().clone();
            let data = data.clone();
            sidebar.new_tab_button().connect_clicked(move |_| {
                open_tab(
                    HOME_URI,
                    "New Tab",
                    &tab_list.active_workspace(),
                    &tab_list,
                    &stack,
                    &tabs,
                    &omnibar,
                    &data,
                );
            });
        }

        {
            let tabs = tabs.clone();
            let stack = stack.clone();
            let tab_list = tab_list.clone();
            let omnibar = toolbar.omnibar().clone();
            let data = data.clone();
            sidebar
                .workspace()
                .connect_workspace_selected(move |workspace_id| {
                    tab_list.set_active_workspace(workspace_id);

                    if let Some(first_tab_id) = tab_list.first_tab_in_workspace(workspace_id) {
                        tab_list.select_tab(&first_tab_id);
                        stack.set_visible_child_name(&first_tab_id);
                        with_tab(&tabs, &first_tab_id, |tab| {
                            if let Some(uri) = tab.uri() {
                                omnibar.set_text(&uri);
                            }
                        });
                    } else {
                        open_tab(
                            HOME_URI,
                            "New Tab",
                            workspace_id,
                            &tab_list,
                            &stack,
                            &tabs,
                            &omnibar,
                            &data,
                        );
                    }
                });
        }

        connect_toolbar(&toolbar, &tabs, &stack);
        connect_omnibar(toolbar.omnibar(), &tabs, &stack, &data);
        connect_shortcuts(
            &window,
            command_palette.clone(),
            toolbar.omnibar().clone(),
            tab_list.clone(),
            stack.clone(),
            tabs.clone(),
            data.clone(),
        );
        connect_command_palette(
            &command_palette,
            toolbar.omnibar().clone(),
            tab_list.clone(),
            stack.clone(),
            tabs.clone(),
            data.clone(),
        );

        Self {
            window,
            _command_palette: command_palette,
        }
    }

    pub fn present(&self) {
        self.window.present();
    }
}

fn connect_toolbar(toolbar: &Toolbar, tabs: &Rc<RefCell<Vec<Tab>>>, stack: &gtk4::Stack) {
    {
        let tabs = tabs.clone();
        let stack = stack.clone();
        toolbar.connect_back_clicked(move || {
            with_active_tab(&tabs, &stack, |tab| tab.go_back());
        });
    }

    {
        let tabs = tabs.clone();
        let stack = stack.clone();
        toolbar.connect_forward_clicked(move || {
            with_active_tab(&tabs, &stack, |tab| tab.go_forward());
        });
    }

    {
        let tabs = tabs.clone();
        let stack = stack.clone();
        toolbar.connect_reload_clicked(move || {
            with_active_tab(&tabs, &stack, |tab| tab.reload());
        });
    }
}

fn connect_omnibar(
    omnibar: &Omnibar,
    tabs: &Rc<RefCell<Vec<Tab>>>,
    stack: &gtk4::Stack,
    data: &Rc<BrowserData>,
) {
    let tabs = tabs.clone();
    let stack = stack.clone();
    let data = data.clone();
    omnibar.connect_activate(move |input| {
        if let Some(uri) = navigation_target(&input) {
            load_uri_or_internal_page(&uri, &tabs, &stack, &data);
        }
    });
}

fn connect_shortcuts(
    window: &adw::ApplicationWindow,
    command_palette: Rc<CommandPalette>,
    omnibar: Omnibar,
    tab_list: TabList,
    stack: gtk4::Stack,
    tabs: Rc<RefCell<Vec<Tab>>>,
    data: Rc<BrowserData>,
) {
    let key_controller = gtk4::EventControllerKey::new();
    key_controller.connect_key_pressed(move |_, keyval, _, state| {
        let ctrl = state.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
        let alt = state.contains(gtk4::gdk::ModifierType::ALT_MASK);

        if ctrl && keyval == gtk4::gdk::Key::k {
            command_palette.show();
            return gtk4::glib::Propagation::Stop;
        }

        if ctrl && keyval == gtk4::gdk::Key::t {
            open_tab(
                HOME_URI,
                "New Tab",
                &tab_list.active_workspace(),
                &tab_list,
                &stack,
                &tabs,
                &omnibar,
                &data,
            );
            return gtk4::glib::Propagation::Stop;
        }

        if ctrl && keyval == gtk4::gdk::Key::w {
            close_active_tab(&tab_list, &stack, &tabs, &omnibar, &data);
            return gtk4::glib::Propagation::Stop;
        }

        if ctrl && keyval == gtk4::gdk::Key::l {
            focus_omnibar(&omnibar);
            return gtk4::glib::Propagation::Stop;
        }

        if ctrl && keyval == gtk4::gdk::Key::r {
            with_active_tab(&tabs, &stack, |tab| tab.reload());
            return gtk4::glib::Propagation::Stop;
        }

        if ctrl && keyval == gtk4::gdk::Key::d {
            toggle_active_bookmark(&data, &tabs, &stack);
            return gtk4::glib::Propagation::Stop;
        }

        if ctrl && keyval == gtk4::gdk::Key::h {
            open_history_page(&data, &tabs, &stack);
            omnibar.set_text("bitter://history");
            return gtk4::glib::Propagation::Stop;
        }

        if ctrl && keyval == gtk4::gdk::Key::b {
            open_bookmarks_page(&data, &tabs, &stack);
            omnibar.set_text("bitter://bookmarks");
            return gtk4::glib::Propagation::Stop;
        }

        if alt && keyval == gtk4::gdk::Key::Left {
            with_active_tab(&tabs, &stack, |tab| tab.go_back());
            return gtk4::glib::Propagation::Stop;
        }

        if alt && keyval == gtk4::gdk::Key::Right {
            with_active_tab(&tabs, &stack, |tab| tab.go_forward());
            return gtk4::glib::Propagation::Stop;
        }

        gtk4::glib::Propagation::Proceed
    });
    window.add_controller(key_controller);
}

fn connect_command_palette(
    command_palette: &CommandPalette,
    omnibar: Omnibar,
    tab_list: TabList,
    stack: gtk4::Stack,
    tabs: Rc<RefCell<Vec<Tab>>>,
    data: Rc<BrowserData>,
) {
    command_palette.connect_command_activated(move |cmd_id| match cmd_id {
        "new_tab" => open_tab(
            HOME_URI,
            "New Tab",
            &tab_list.active_workspace(),
            &tab_list,
            &stack,
            &tabs,
            &omnibar,
            &data,
        ),
        "close_tab" => close_active_tab(&tab_list, &stack, &tabs, &omnibar, &data),
        "focus_omnibar" => focus_omnibar(&omnibar),
        "reload" => with_active_tab(&tabs, &stack, |tab| tab.reload()),
        "go_back" => with_active_tab(&tabs, &stack, |tab| tab.go_back()),
        "go_forward" => with_active_tab(&tabs, &stack, |tab| tab.go_forward()),
        "copy_url" => copy_active_url(&tabs, &stack),
        "bookmark_page" => toggle_active_bookmark(&data, &tabs, &stack),
        "history" => {
            open_history_page(&data, &tabs, &stack);
            omnibar.set_text("bitter://history");
        }
        "bookmarks" => {
            open_bookmarks_page(&data, &tabs, &stack);
            omnibar.set_text("bitter://bookmarks");
        }
        "settings" => {
            open_settings_page(&tabs, &stack);
            omnibar.set_text("bitter://settings");
        }
        _ => tracing::debug!("Command not implemented: {cmd_id}"),
    });
}

fn open_tab(
    uri: &str,
    fallback_title: &str,
    workspace_id: &str,
    tab_list: &TabList,
    stack: &gtk4::Stack,
    tabs: &Rc<RefCell<Vec<Tab>>>,
    omnibar: &Omnibar,
    data: &Rc<BrowserData>,
) {
    let tab = Tab::new(uri);
    let id = tab.id().to_string();

    tab_list.add_tab(&id, fallback_title, workspace_id);
    stack.add_named(tab.widget(), Some(&id));

    {
        let tab_list = tab_list.clone();
        let id = id.clone();
        tab.connect_title_changed(move |title| {
            tab_list.set_tab_title(&id, title);
        });
    }

    {
        let stack = stack.clone();
        let id = id.clone();
        let omnibar = omnibar.clone();
        tab.connect_uri_changed(move |uri| {
            if stack.visible_child_name().as_deref() == Some(id.as_str()) {
                omnibar.set_text(uri);
            }
        });
    }

    {
        let data = data.clone();
        tab.connect_metadata_changed(move |uri, title| {
            record_history_visit(&data, uri.as_deref(), title.as_deref());
        });
    }

    tabs.borrow_mut().push(tab);
    stack.set_visible_child_name(&id);
    tab_list.select_tab(&id);
    omnibar.set_text(uri);
}

fn close_active_tab(
    tab_list: &TabList,
    stack: &gtk4::Stack,
    tabs: &Rc<RefCell<Vec<Tab>>>,
    omnibar: &Omnibar,
    data: &Rc<BrowserData>,
) {
    if let Some(id) = stack.visible_child_name() {
        close_tab(id.as_str(), tab_list, stack, tabs, omnibar, data);
    }
}

fn close_tab(
    id: &str,
    tab_list: &TabList,
    stack: &gtk4::Stack,
    tabs: &Rc<RefCell<Vec<Tab>>>,
    omnibar: &Omnibar,
    data: &Rc<BrowserData>,
) {
    let tab = {
        let mut tabs_mut = tabs.borrow_mut();
        let Some(index) = tabs_mut.iter().position(|tab| tab.id() == id) else {
            return;
        };

        tabs_mut.remove(index)
    };
    stack.remove(tab.widget());
    tab_list.remove_tab(id);

    let active_workspace = tab_list.active_workspace();
    if let Some(next_tab_id) = tab_list.first_tab_in_workspace(&active_workspace) {
        tab_list.select_tab(&next_tab_id);
        stack.set_visible_child_name(&next_tab_id);
        with_tab(tabs, &next_tab_id, |tab| {
            if let Some(uri) = tab.uri() {
                omnibar.set_text(&uri);
            }
        });
    } else {
        open_tab(
            HOME_URI,
            "New Tab",
            &active_workspace,
            tab_list,
            stack,
            tabs,
            omnibar,
            data,
        );
    }
}

fn with_active_tab<F: FnOnce(&Tab)>(tabs: &Rc<RefCell<Vec<Tab>>>, stack: &gtk4::Stack, f: F) {
    if let Some(id) = stack.visible_child_name() {
        with_tab(tabs, id.as_str(), f);
    }
}

fn with_tab<F: FnOnce(&Tab)>(tabs: &Rc<RefCell<Vec<Tab>>>, id: &str, f: F) {
    let tabs = tabs.borrow();
    with_tab_in_borrow(&tabs, id, f);
}

fn with_tab_in_borrow<F: FnOnce(&Tab)>(tabs: &[Tab], id: &str, f: F) {
    if let Some(tab) = tabs.iter().find(|tab| tab.id() == id) {
        f(tab);
    }
}

fn focus_omnibar(omnibar: &Omnibar) {
    omnibar.grab_focus();
    omnibar.select_all();
}

fn copy_active_url(tabs: &Rc<RefCell<Vec<Tab>>>, stack: &gtk4::Stack) {
    with_active_tab(tabs, stack, |tab| {
        if let (Some(display), Some(uri)) = (gtk4::gdk::Display::default(), tab.uri()) {
            display.clipboard().set_text(&uri);
        }
    });
}

fn load_uri_or_internal_page(
    uri: &str,
    tabs: &Rc<RefCell<Vec<Tab>>>,
    stack: &gtk4::Stack,
    data: &Rc<BrowserData>,
) {
    match uri {
        "bitter://history" => open_history_page(data, tabs, stack),
        "bitter://bookmarks" => open_bookmarks_page(data, tabs, stack),
        "bitter://settings" => open_settings_page(tabs, stack),
        _ => with_active_tab(tabs, stack, |tab| tab.load_uri(uri)),
    }
}

fn open_history_page(data: &BrowserData, tabs: &Rc<RefCell<Vec<Tab>>>, stack: &gtk4::Stack) {
    let rows = data
        .history
        .as_ref()
        .and_then(|history| {
            history
                .recent(50)
                .map_err(|err| tracing::warn!("Failed to load history: {err}"))
                .ok()
        })
        .unwrap_or_default();

    let html = internal_list_page(
        "History",
        "Recent pages",
        &rows,
        "History will appear here after you visit pages.",
    );
    with_active_tab(tabs, stack, |tab| {
        tab.load_html(&html, "bitter://history");
    });
}

fn open_bookmarks_page(data: &BrowserData, tabs: &Rc<RefCell<Vec<Tab>>>, stack: &gtk4::Stack) {
    let rows = data
        .bookmarks
        .as_ref()
        .and_then(|bookmarks| {
            bookmarks
                .get_all()
                .map_err(|err| tracing::warn!("Failed to load bookmarks: {err}"))
                .ok()
        })
        .unwrap_or_default();

    let html = internal_list_page(
        "Bookmarks",
        "Saved pages",
        &rows,
        "Bookmarks will appear here after you save pages with Ctrl+D.",
    );
    with_active_tab(tabs, stack, |tab| {
        tab.load_html(&html, "bitter://bookmarks");
    });
}

fn open_settings_page(tabs: &Rc<RefCell<Vec<Tab>>>, stack: &gtk4::Stack) {
    let html = internal_page_shell(
        "Settings",
        r#"<section class="empty">
            <h2>Settings</h2>
            <p>Preferences UI is not implemented yet.</p>
        </section>"#,
    );

    with_active_tab(tabs, stack, |tab| {
        tab.load_html(&html, "bitter://settings");
    });
}

fn toggle_active_bookmark(data: &BrowserData, tabs: &Rc<RefCell<Vec<Tab>>>, stack: &gtk4::Stack) {
    let Some(bookmarks) = data.bookmarks.as_ref() else {
        tracing::warn!("Bookmarks store is unavailable");
        return;
    };

    with_active_tab(tabs, stack, |tab| {
        let Some(uri) = tab.uri() else {
            return;
        };

        if !is_recordable_uri(&uri) {
            return;
        }

        let title = tab.title();
        match bookmarks.is_bookmarked(&uri) {
            Ok(true) => {
                if let Err(err) = bookmarks.remove_bookmark(&uri) {
                    tracing::warn!("Failed to remove bookmark: {err}");
                }
            }
            Ok(false) => {
                if let Err(err) = bookmarks.add_bookmark(&uri, title.as_deref()) {
                    tracing::warn!("Failed to add bookmark: {err}");
                }
            }
            Err(err) => tracing::warn!("Failed to inspect bookmark state: {err}"),
        }
    });
}

fn record_history_visit(data: &BrowserData, uri: Option<&str>, title: Option<&str>) {
    let Some(uri) = uri else {
        return;
    };

    if !is_recordable_uri(uri) {
        return;
    }

    if let Some(history) = data.history.as_ref() {
        let title = title.filter(|title| !title.trim().is_empty());
        if let Err(err) = history.add_visit(uri, title) {
            tracing::warn!("Failed to record history visit: {err}");
        }
    }
}

fn internal_list_page(
    title: &str,
    heading: &str,
    rows: &[(String, String)],
    empty_message: &str,
) -> String {
    let body = if rows.is_empty() {
        format!(
            r#"<section class="empty"><h2>{}</h2><p>{}</p></section>"#,
            escape_html(heading),
            escape_html(empty_message)
        )
    } else {
        let items = rows
            .iter()
            .map(|(url, title)| {
                let label = if title.trim().is_empty() { url } else { title };
                format!(
                    r#"<a class="row" href="{url}">
                        <span class="title">{title}</span>
                        <span class="url">{url}</span>
                    </a>"#,
                    url = escape_html(url),
                    title = escape_html(label)
                )
            })
            .collect::<String>();

        format!(
            r#"<section><h2>{}</h2><div class="list">{}</div></section>"#,
            escape_html(heading),
            items
        )
    };

    internal_page_shell(title, &body)
}

fn internal_page_shell(title: &str, body: &str) -> String {
    format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <style>
    :root {{
      color-scheme: dark;
      font-family: Inter, ui-sans-serif, system-ui, sans-serif;
      background: #0d0d0d;
      color: #f0f0f0;
    }}
    body {{
      margin: 0;
      padding: 48px;
      background: #0d0d0d;
    }}
    main {{
      max-width: 880px;
      margin: 0 auto;
    }}
    h1 {{
      margin: 0 0 28px;
      font-size: 28px;
      font-weight: 650;
    }}
    h2 {{
      margin: 0 0 16px;
      font-size: 16px;
      font-weight: 650;
      color: #f0f0f0;
    }}
    p {{
      margin: 0;
      color: #888888;
    }}
    .list {{
      display: grid;
      gap: 1px;
      overflow: hidden;
      border: 1px solid #2a2a2a;
      border-radius: 8px;
      background: #2a2a2a;
    }}
    .row {{
      display: grid;
      gap: 4px;
      padding: 12px 14px;
      color: inherit;
      text-decoration: none;
      background: #141414;
    }}
    .row:hover {{
      background: #1a1a1a;
    }}
    .title {{
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-size: 14px;
    }}
    .url {{
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      color: #888888;
      font-size: 12px;
    }}
    .empty {{
      padding: 32px;
      border: 1px solid #2a2a2a;
      border-radius: 8px;
      background: #141414;
    }}
  </style>
</head>
<body>
  <main>
    <h1>{title}</h1>
    {body}
  </main>
</body>
</html>"#,
        title = escape_html(title),
        body = body
    )
}

fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn is_recordable_uri(uri: &str) -> bool {
    uri.starts_with("http://") || uri.starts_with("https://")
}

fn navigation_target(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    if looks_like_absolute_uri(input) {
        return Some(input.to_string());
    }

    if looks_like_hostname(input) {
        return Some(format!("https://{input}"));
    }

    let query: String = url::form_urlencoded::byte_serialize(input.as_bytes()).collect();
    Some(format!("https://duckduckgo.com/?q={query}"))
}

fn looks_like_absolute_uri(input: &str) -> bool {
    input.starts_with("http://")
        || input.starts_with("https://")
        || input.starts_with("about:")
        || input.starts_with("file:")
        || input.starts_with("bitter:")
}

fn looks_like_hostname(input: &str) -> bool {
    !input.contains(char::is_whitespace)
        && (input == "localhost"
            || input.starts_with("localhost:")
            || input.contains('.')
            || input.starts_with('['))
}
