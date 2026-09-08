use crate::core::chat_session::ChatSession;
use crate::t;
use adw::prelude::*;
use gtk4::pango::EllipsizeMode;
use gtk4::{
    Align, Box, Button, Label, ListBox, ListBoxRow, Orientation, PolicyType, ScrolledWindow,
    SelectionMode,
};
use std::boxed::Box as StdBox;
use std::cell::RefCell;
use std::rc::Rc;

type SessionCallback = StdBox<dyn Fn(String)>;

/// Component managing the Chat history sidebar
pub struct ChatSidebar {
    container: Box,
    conversations_list: ListBox,
    btn_new_chat: Button,
    selected_callback: Rc<RefCell<Option<SessionCallback>>>,
    deleted_callback: Rc<RefCell<Option<SessionCallback>>>,
    current_session_ids: Rc<RefCell<Vec<String>>>,
}

impl Default for ChatSidebar {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatSidebar {
    pub fn new() -> Self {
        let container = Box::new(Orientation::Vertical, 0);
        container.set_css_classes(&["chat-sidebar"]);
        container.set_size_request(250, -1);
        container.set_width_request(250);
        container.set_hexpand(false);
        container.set_vexpand(true);

        // Sidebar Header
        let sidebar_header = Box::new(Orientation::Horizontal, 6);
        sidebar_header.set_css_classes(&["chat-sidebar-header"]);
        sidebar_header.set_margin_start(8);
        sidebar_header.set_margin_end(8);
        sidebar_header.set_margin_top(12);
        sidebar_header.set_margin_bottom(8);

        let sidebar_title = Label::builder()
            .label(t!("chat.sidebar_title"))
            .css_classes(["heading"])
            .halign(Align::Start)
            .ellipsize(EllipsizeMode::End)
            .width_chars(1)
            .hexpand(true)
            .build();

        let btn_new_chat = Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text(t!("chat.new_chat_tooltip"))
            .css_classes(["suggested-action", "circular"])
            .valign(Align::Center)
            .build();

        sidebar_header.append(&sidebar_title);
        sidebar_header.append(&btn_new_chat);
        container.append(&sidebar_header);

        // Conversations list
        let conversations_list = ListBox::builder()
            .selection_mode(SelectionMode::Single)
            .css_classes(["navigation-sidebar"])
            .hexpand(false)
            .build();

        let conv_scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .propagate_natural_width(false)
            .hexpand(false)
            .vexpand(true)
            .child(&conversations_list)
            .build();
        container.append(&conv_scrolled);

        let selected_callback: Rc<RefCell<Option<SessionCallback>>> = Rc::new(RefCell::new(None));
        let deleted_callback: Rc<RefCell<Option<SessionCallback>>> = Rc::new(RefCell::new(None));
        let current_session_ids: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

        let sel_cb_clone = selected_callback.clone();
        let ids_clone = current_session_ids.clone();
        conversations_list.connect_row_activated(move |_, row| {
            let idx = row.index() as usize;
            let ids = ids_clone.borrow();
            if let Some(target_id) = ids.get(idx) {
                if let Some(ref cb) = *sel_cb_clone.borrow() {
                    cb(target_id.clone());
                }
            }
        });

        Self {
            container,
            conversations_list,
            btn_new_chat,
            selected_callback,
            deleted_callback,
            current_session_ids,
        }
    }

    pub fn widget(&self) -> &Box {
        &self.container
    }

    pub fn toggle_visibility(&self) {
        self.container.set_visible(!self.container.is_visible());
    }

    pub fn connect_new_chat<F: Fn() + 'static>(&self, f: F) {
        self.btn_new_chat.connect_clicked(move |_| {
            f();
        });
    }

    pub fn connect_session_selected<F: Fn(String) + 'static>(&self, f: F) {
        *self.selected_callback.borrow_mut() = Some(StdBox::new(f));
    }

    pub fn connect_session_deleted<F: Fn(String) + 'static>(&self, f: F) {
        *self.deleted_callback.borrow_mut() = Some(StdBox::new(f));
    }

    pub fn render_sessions(&self, sessions: &[ChatSession], active_id: &str) {
        let mut child = self.conversations_list.first_child();
        while let Some(w) = child {
            let next = w.next_sibling();
            self.conversations_list.remove(&w);
            child = next;
        }

        let mut ids = Vec::with_capacity(sessions.len());

        for s in sessions {
            ids.push(s.id.clone());

            let row = ListBoxRow::new();
            row.set_css_classes(&["chat-history-row"]);
            if s.id == active_id {
                row.add_css_class("active");
            }

            let row_box = Box::new(Orientation::Horizontal, 8);
            row_box.set_margin_start(4);
            row_box.set_margin_end(4);
            row_box.set_margin_top(4);
            row_box.set_margin_bottom(4);

            let btn_delete = Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text(t!("chat.delete_chat_tooltip"))
                .css_classes(["flat", "circular", "chat-delete-btn"])
                .valign(Align::Center)
                .build();

            let sess_id_del = s.id.clone();
            let del_cb_ref = self.deleted_callback.clone();
            btn_delete.connect_clicked(move |_| {
                if let Some(ref cb) = *del_cb_ref.borrow() {
                    cb(sess_id_del.clone());
                }
            });

            let info_box = Box::new(Orientation::Vertical, 2);
            info_box.set_hexpand(true);
            info_box.set_valign(Align::Center);

            let display_title = if s.messages.is_empty() && ChatSession::is_default_title(&s.title) {
                t!("chat.default_title")
            } else {
                s.title.clone()
            };

            let title_lbl = Label::builder()
                .label(&display_title)
                .ellipsize(EllipsizeMode::End)
                .xalign(0.0)
                .width_chars(1)
                .max_width_chars(14)
                .hexpand(true)
                .css_classes(["heading", "caption"])
                .build();

            let model_display = s.model.as_deref().unwrap_or("General");
            let count_display = t!("chat.messages_count", count = s.messages.len().to_string());
            let sub_text = format!("{} · {}", model_display, count_display);

            let meta_lbl = Label::builder()
                .label(&sub_text)
                .ellipsize(EllipsizeMode::End)
                .xalign(0.0)
                .width_chars(1)
                .max_width_chars(14)
                .hexpand(true)
                .css_classes(["caption", "dim-label"])
                .build();

            info_box.append(&title_lbl);
            info_box.append(&meta_lbl);

            row_box.append(&btn_delete);
            row_box.append(&info_box);
            row.set_child(Some(&row_box));

            self.conversations_list.append(&row);
        }

        *self.current_session_ids.borrow_mut() = ids;
    }
}
