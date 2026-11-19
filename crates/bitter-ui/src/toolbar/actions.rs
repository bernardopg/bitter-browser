use gtk4::prelude::*;

pub struct Actions {
    container: gtk4::Box,
    back: gtk4::Button,
    forward: gtk4::Button,
    reload: gtk4::Button,
}

impl Actions {
    pub fn new() -> Self {
        let container = gtk4::Box::builder().spacing(4).build();
        let back = button("go-previous-symbolic", "Back (Alt+Left)", "win.back");
        let forward = button("go-next-symbolic", "Forward (Alt+Right)", "win.forward");
        let reload = button("view-refresh-symbolic", "Reload (Ctrl+R)", "win.reload");
        for button in [&back, &forward, &reload] {
            container.append(button);
        }
        Self {
            container,
            back,
            forward,
            reload,
        }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.container.upcast_ref()
    }

    pub fn refresh(&self, back: bool, forward: bool, loading: bool) {
        self.back.set_sensitive(back);
        self.forward.set_sensitive(forward);
        self.reload.set_icon_name(if loading {
            "process-stop-symbolic"
        } else {
            "view-refresh-symbolic"
        });
        let label = if loading {
            "Stop loading (Escape)"
        } else {
            "Reload (Ctrl+R)"
        };
        self.reload.set_tooltip_text(Some(label));
        self.reload
            .update_property(&[gtk4::accessible::Property::Label(label)]);
    }
}

fn button(icon: &str, title: &str, action: &str) -> gtk4::Button {
    let button = gtk4::Button::builder()
        .icon_name(icon)
        .tooltip_text(title)
        .action_name(action)
        .build();
    button.update_property(&[gtk4::accessible::Property::Label(title)]);
    button
}
