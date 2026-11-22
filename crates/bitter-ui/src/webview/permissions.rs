//! Permission requests from pages: what is asked, and the bar that asks.

use bitter_core::records::Permission;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use webkit6::prelude::*;

/// What a page asked for, in the terms the browser remembers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Answered without asking, and never remembered.
    Automatic(bool),
    /// Needs an answer; remembered for the listed permissions.
    Ask(Vec<Permission>),
    /// Needs an answer every time (screen sharing).
    AskOnce(&'static str),
}

pub fn classify(request: &webkit6::PermissionRequest) -> Request {
    use webkit6::*;
    if request.is::<NotificationPermissionRequest>() {
        Request::Ask(vec![Permission::Notifications])
    } else if request.is::<GeolocationPermissionRequest>() {
        Request::Ask(vec![Permission::Location])
    } else if let Some(media) = request.downcast_ref::<UserMediaPermissionRequest>() {
        if functions::user_media_permission_is_for_display_device(media) {
            return Request::AskOnce("share your screen");
        }
        let mut wanted = Vec::new();
        if media.is_for_video_device() {
            wanted.push(Permission::Camera);
        }
        if media.is_for_audio_device() {
            wanted.push(Permission::Microphone);
        }
        if wanted.is_empty() {
            Request::Automatic(false)
        } else {
            Request::Ask(wanted)
        }
    } else if request.is::<ClipboardPermissionRequest>() {
        Request::Ask(vec![Permission::Clipboard])
    } else if request.is::<PointerLockPermissionRequest>() {
        // Games capture the pointer; Esc always releases it.
        Request::Automatic(true)
    } else {
        // Device lists are answered from the camera and microphone choices
        // by the caller; DRM, XR and storage access are unavailable.
        Request::Automatic(false)
    }
}

/// The question shown to the user.
pub fn message(site: &str, request: &Request) -> String {
    let wants = match request {
        Request::AskOnce(action) => (*action).to_owned(),
        Request::Ask(permissions) => {
            let has = |permission| permissions.contains(&permission);
            if has(Permission::Camera) && has(Permission::Microphone) {
                "use your camera and microphone".to_owned()
            } else {
                match permissions.first() {
                    Some(Permission::Notifications) => "show notifications".to_owned(),
                    Some(Permission::Location) => "know your location".to_owned(),
                    Some(Permission::Camera) => "use your camera".to_owned(),
                    Some(Permission::Microphone) => "use your microphone".to_owned(),
                    Some(Permission::Clipboard) => "see text and images you copied".to_owned(),
                    None => "access your device".to_owned(),
                }
            }
        }
        Request::Automatic(_) => "access your device".to_owned(),
    };
    format!("{site} wants to {wants}")
}

pub fn icon(request: &Request) -> &'static str {
    match request {
        Request::AskOnce(_) => "video-display-symbolic",
        Request::Ask(permissions) => match permissions.first() {
            Some(Permission::Notifications) => "preferences-system-notifications-symbolic",
            Some(Permission::Location) => "find-location-symbolic",
            Some(Permission::Camera) => "camera-web-symbolic",
            Some(Permission::Microphone) => "audio-input-microphone-symbolic",
            Some(Permission::Clipboard) => "edit-paste-symbolic",
            None => "dialog-question-symbolic",
        },
        Request::Automatic(_) => "dialog-question-symbolic",
    }
}

pub struct Prompt {
    pub request: webkit6::PermissionRequest,
    pub origin: String,
    pub kind: Request,
}

type Answered = Box<dyn Fn(&Prompt, bool)>;

/// A bar above the page that asks one question at a time.
pub struct PermissionBar {
    pub revealer: gtk4::Revealer,
    icon: gtk4::Image,
    label: gtk4::Label,
    queue: RefCell<VecDeque<Prompt>>,
    answered: RefCell<Option<Answered>>,
}

impl PermissionBar {
    pub fn new() -> Rc<Self> {
        let icon = gtk4::Image::builder().pixel_size(18).build();
        let label = gtk4::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .wrap(true)
            .css_classes(["heading"])
            .build();
        let block = gtk4::Button::builder()
            .label("Block")
            .css_classes(["pill"])
            .build();
        let allow = gtk4::Button::builder()
            .label("Allow")
            .css_classes(["pill", "suggested-action"])
            .build();
        let content = gtk4::Box::builder()
            .spacing(12)
            .css_classes(["permission-bar"])
            .build();
        content.append(&icon);
        content.append(&label);
        content.append(&block);
        content.append(&allow);
        content.update_property(&[gtk4::accessible::Property::Label("Permission request")]);
        let revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::SlideDown)
            .transition_duration(160)
            .child(&content)
            .build();
        let bar = Rc::new(Self {
            revealer,
            icon,
            label,
            queue: RefCell::default(),
            answered: RefCell::default(),
        });
        for (button, allowed) in [(&allow, true), (&block, false)] {
            let weak = Rc::downgrade(&bar);
            button.connect_clicked(move |_| {
                if let Some(bar) = weak.upgrade() {
                    bar.answer(allowed);
                }
            });
        }
        bar
    }

    pub fn connect_answered(&self, f: impl Fn(&Prompt, bool) + 'static) {
        self.answered.replace(Some(Box::new(f)));
    }

    pub fn ask(&self, prompt: Prompt) {
        // Pages often repeat a request; one question per kind and site.
        let duplicate = self
            .queue
            .borrow()
            .iter()
            .any(|known| known.origin == prompt.origin && known.kind == prompt.kind);
        if duplicate {
            prompt.request.deny();
            return;
        }
        self.queue.borrow_mut().push_back(prompt);
        self.show_first();
    }

    /// Answers the open question, as the buttons do.
    pub fn answer(&self, allowed: bool) {
        let Some(prompt) = self.queue.borrow_mut().pop_front() else {
            return;
        };
        if allowed {
            prompt.request.allow();
        } else {
            prompt.request.deny();
        }
        if let Some(answered) = self.answered.borrow().as_ref() {
            answered(&prompt, allowed);
        }
        self.show_first();
    }

    /// Denies every open question, when the page goes away.
    pub fn dismiss(&self) {
        let prompts: Vec<_> = self.queue.borrow_mut().drain(..).collect();
        for prompt in prompts {
            prompt.request.deny();
        }
        self.show_first();
    }

    fn show_first(&self) {
        let queue = self.queue.borrow();
        match queue.front() {
            Some(prompt) => {
                let site = url::Url::parse(&prompt.origin)
                    .ok()
                    .and_then(|url| url.host_str().map(str::to_owned))
                    .unwrap_or_else(|| prompt.origin.clone());
                self.label.set_label(&message(&site, &prompt.kind));
                self.icon.set_icon_name(Some(icon(&prompt.kind)));
                self.revealer.set_reveal_child(true);
            }
            None => self.revealer.set_reveal_child(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn questions_name_the_site_and_what_it_wants() {
        let ask = |permissions: &[Permission]| Request::Ask(permissions.to_vec());
        assert_eq!(
            message(
                "meet.example",
                &ask(&[Permission::Camera, Permission::Microphone])
            ),
            "meet.example wants to use your camera and microphone"
        );
        assert_eq!(
            message("maps.example", &ask(&[Permission::Location])),
            "maps.example wants to know your location"
        );
        assert_eq!(
            message("chat.example", &ask(&[Permission::Notifications])),
            "chat.example wants to show notifications"
        );
        assert_eq!(
            message("call.example", &Request::AskOnce("share your screen")),
            "call.example wants to share your screen"
        );
        assert_eq!(
            icon(&ask(&[Permission::Microphone])),
            "audio-input-microphone-symbolic"
        );
    }

    #[test]
    fn questions_cover_automatic_and_empty_requests() {
        let ask = |permissions: &[Permission]| Request::Ask(permissions.to_vec());
        assert_eq!(
            message("site", &Request::Automatic(true)),
            "site wants to access your device"
        );
        assert_eq!(
            message("site", &Request::Automatic(false)),
            "site wants to access your device"
        );
        assert_eq!(
            message("site", &ask(&[])),
            "site wants to access your device"
        );
        assert_eq!(
            message(
                "site",
                &ask(&[Permission::Clipboard, Permission::Notifications])
            ),
            "site wants to see text and images you copied"
        );
        assert_eq!(icon(&ask(&[Permission::Camera])), "camera-web-symbolic");
        assert_eq!(icon(&Request::Automatic(true)), "dialog-question-symbolic");
    }

    #[test]
    fn icons_and_messages_cover_each_permission_kind() {
        let ask = |permissions: &[Permission]| Request::Ask(permissions.to_vec());
        assert_eq!(
            message("site", &ask(&[Permission::Camera])),
            "site wants to use your camera"
        );
        assert_eq!(
            message("site", &ask(&[Permission::Microphone])),
            "site wants to use your microphone"
        );
        assert_eq!(
            icon(&ask(&[Permission::Notifications])),
            "preferences-system-notifications-symbolic"
        );
        assert_eq!(
            icon(&ask(&[Permission::Location])),
            "find-location-symbolic"
        );
        assert_eq!(icon(&ask(&[Permission::Clipboard])), "edit-paste-symbolic");
        assert_eq!(icon(&ask(&[])), "dialog-question-symbolic");
        assert_eq!(
            icon(&Request::AskOnce("share your screen")),
            "video-display-symbolic"
        );
        assert_eq!(icon(&Request::Automatic(false)), "dialog-question-symbolic");
    }
}
