use crate::api::client::OllamaClient;
use crate::api::types::{ChatMessage, ChatMetrics, ModelNameUtils, ModelPs, ModelTag};
use crate::core::capabilities::detect_icons_for_model_tag;
use crate::core::config::AppConfig;
use crate::core::hardware::estimator::format_gib;
use crate::core::hardware::HardwareSnapshot;
use crate::core::hub::{HardwareFitness, HubManager};
use crate::t;
use crate::ui::components::{ChatSettingsPopover, ChatSidebar};
use adw::prelude::*;
use gtk4::gdk::Key;
use gtk4::{
    Align, Box, Button, DropDown, EventControllerKey, Image, Label, ListItem, MenuButton,
    Orientation, PolicyType, ScrolledWindow, Separator, SignalListItemFactory, StringList,
    StringObject, TextBuffer, TextView, WrapMode,
};
use std::boxed::Box as StdBox;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub use crate::core::chat_session::ChatSession;

type ModelHardwareInfo = (HardwareFitness, bool, u64, bool);
type SendMessageCallback = StdBox<dyn Fn(String, Vec<ChatMessage>, Option<String>, Option<f64>, Option<u32>)>;
type CancelMessageCallback = StdBox<dyn Fn()>;

struct AssistantMessageWidgets {
    text_label: Label,
    metrics_box: Box,
    speed_badge: Label,
    ttft_badge: Label,
    tokens_badge: Label,
    vram_badge: Label,
    status_label: Label,
    full_text: Rc<RefCell<String>>,
}

#[derive(Clone)]
struct SessionLoadContext {
    sessions: Rc<RefCell<Vec<ChatSession>>>,
    active_id: Rc<RefCell<String>>,
    msgs_box: Box,
    empty_box: Box,
    scrolled: ScrolledWindow,
    popover: Rc<ChatSettingsPopover>,
    sel_model: Rc<RefCell<Option<String>>>,
    model_dropdown: DropDown,
    model_ids: Rc<RefCell<Vec<String>>>,
    config: Rc<RefCell<AppConfig>>,
    client: Rc<RefCell<OllamaClient>>,
    model_max_contexts: Rc<RefCell<HashMap<String, u32>>>,
}

pub struct ChatView {
    container: Box,
    sidebar: Rc<ChatSidebar>,
    btn_toggle_sidebar: Button,

    client: Rc<RefCell<OllamaClient>>,
    config: Rc<RefCell<AppConfig>>,
    model_max_contexts: Rc<RefCell<HashMap<String, u32>>>,

    sessions: Rc<RefCell<Vec<ChatSession>>>,
    active_session_id: Rc<RefCell<String>>,

    model_dropdown: DropDown,
    model_ids: Rc<RefCell<Vec<String>>>,
    model_infos: Rc<RefCell<HashMap<String, ModelHardwareInfo>>>,
    model_status_badge: Label,
    updating_dropdown: Rc<RefCell<bool>>,
    selected_model: Rc<RefCell<Option<String>>>,

    messages_scrolled: ScrolledWindow,
    messages_box: Box,
    empty_state_box: Box,
    active_assistant: Rc<RefCell<Option<AssistantMessageWidgets>>>,

    input_text_view: TextView,
    input_buffer: TextBuffer,
    btn_send: Button,
    btn_stop: Button,
    btn_clear: Button,
    settings_popover: Rc<ChatSettingsPopover>,

    is_generating: Rc<RefCell<bool>>,
    generation_cancelled: Arc<AtomicBool>,

    send_callback: Rc<RefCell<Option<SendMessageCallback>>>,
    cancel_callback: Rc<RefCell<Option<CancelMessageCallback>>>,
}

impl ChatView {
    pub fn new(config: Rc<RefCell<AppConfig>>, client: Rc<RefCell<OllamaClient>>) -> Self {
        let loaded_sessions = ChatSession::load_all();
        let initial_active_id = loaded_sessions.first().map(|s| s.id.clone()).unwrap_or_default();
        let sessions = Rc::new(RefCell::new(loaded_sessions));
        let active_session_id = Rc::new(RefCell::new(initial_active_id));

        let container = Box::new(Orientation::Horizontal, 0);
        container.set_hexpand(true);
        container.set_vexpand(true);

        let model_ids = Rc::new(RefCell::new(Vec::new()));
        let model_infos = Rc::new(RefCell::new(HashMap::new()));
        let model_max_contexts = Rc::new(RefCell::new(HashMap::new()));
        let updating_dropdown = Rc::new(RefCell::new(false));
        let selected_model = Rc::new(RefCell::new(None));
        let is_generating = Rc::new(RefCell::new(false));
        let generation_cancelled = Arc::new(AtomicBool::new(false));
        let active_assistant = Rc::new(RefCell::new(None));

        // 0. Left Side Panel (History)
        let sidebar = Rc::new(ChatSidebar::new());
        container.append(sidebar.widget());

        let sep = Separator::new(Orientation::Vertical);
        container.append(&sep);

        // Right Main Area
        let main_chat_box = Box::new(Orientation::Vertical, 0);
        main_chat_box.set_hexpand(true);
        main_chat_box.set_vexpand(true);

        // 1. Top Toolbar
        let top_toolbar = Box::new(Orientation::Horizontal, 10);
        top_toolbar.set_margin_start(16);
        top_toolbar.set_margin_end(16);
        top_toolbar.set_margin_top(10);
        top_toolbar.set_margin_bottom(10);
        top_toolbar.set_valign(Align::Center);

        let btn_toggle_sidebar = Button::builder()
            .icon_name("sidebar-show-symbolic")
            .tooltip_text(t!("chat.toggle_sidebar_tooltip"))
            .css_classes(["flat"])
            .valign(Align::Center)
            .build();
        top_toolbar.append(&btn_toggle_sidebar);

        let model_label = Label::builder()
            .label(t!("chat.model_label"))
            .css_classes(["heading"])
            .valign(Align::Center)
            .build();
        top_toolbar.append(&model_label);

        let button_factory = SignalListItemFactory::new();
        button_factory.connect_setup(|_, list_item| {
            let label = Label::builder()
                .use_markup(true)
                .xalign(0.0)
                .halign(Align::Start)
                .build();
            if let Some(item) = list_item.downcast_ref::<ListItem>() {
                item.set_child(Some(&label));
            }
        });
        button_factory.connect_bind(|_, list_item| {
            if let Some(list_item) = list_item.downcast_ref::<ListItem>() {
                if let Some(str_obj) = list_item.item().and_then(|item| item.downcast::<StringObject>().ok()) {
                    if let Some(label) = list_item.child().and_then(|c| c.downcast::<Label>().ok()) {
                        label.set_markup(&str_obj.string());
                    }
                }
            }
        });

        let list_factory = SignalListItemFactory::new();
        list_factory.connect_setup(|_, list_item| {
            let label = Label::builder()
                .use_markup(true)
                .xalign(0.0)
                .halign(Align::Start)
                .margin_top(4)
                .margin_bottom(4)
                .margin_start(6)
                .margin_end(6)
                .build();
            if let Some(item) = list_item.downcast_ref::<ListItem>() {
                item.set_child(Some(&label));
            }
        });
        list_factory.connect_bind(|_, list_item| {
            if let Some(list_item) = list_item.downcast_ref::<ListItem>() {
                if let Some(str_obj) = list_item.item().and_then(|item| item.downcast::<StringObject>().ok()) {
                    if let Some(label) = list_item.child().and_then(|c| c.downcast::<Label>().ok()) {
                        label.set_markup(&str_obj.string());
                    }
                }
            }
        });

        let initial_strings = StringList::new(&[&t!("chat.loading_models")]);
        let model_dropdown = DropDown::builder()
            .model(&initial_strings)
            .factory(&button_factory)
            .list_factory(&list_factory)
            .valign(Align::Center)
            .tooltip_text(t!("chat.model_dropdown_tooltip"))
            .build();
        top_toolbar.append(&model_dropdown);

        let model_status_badge = Label::builder()
            .label(t!("chat.badge_on_disk"))
            .use_markup(true)
            .css_classes(["caption", "dim-label"])
            .valign(Align::Center)
            .margin_start(4)
            .build();
        top_toolbar.append(&model_status_badge);

        let spacer = Box::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        top_toolbar.append(&spacer);

        let settings_popover = Rc::new(ChatSettingsPopover::new());

        let btn_settings = MenuButton::builder()
            .icon_name("emblem-system-symbolic")
            .tooltip_text(t!("chat.settings_btn_tooltip"))
            .css_classes(["flat"])
            .valign(Align::Center)
            .popover(settings_popover.popover())
            .build();
        top_toolbar.append(&btn_settings);

        let btn_clear = Button::builder()
            .icon_name("edit-clear-symbolic")
            .tooltip_text(t!("chat.clear_btn_tooltip"))
            .css_classes(["flat"])
            .valign(Align::Center)
            .build();
        top_toolbar.append(&btn_clear);

        main_chat_box.append(&top_toolbar);

        // 2. Central Messages Area
        let messages_box = Box::new(Orientation::Vertical, 12);
        messages_box.set_margin_start(20);
        messages_box.set_margin_end(20);
        messages_box.set_margin_top(16);
        messages_box.set_margin_bottom(16);

        let empty_state_box = Box::new(Orientation::Vertical, 12);
        empty_state_box.set_valign(Align::Center);
        empty_state_box.set_halign(Align::Center);
        empty_state_box.set_vexpand(true);
        empty_state_box.set_margin_top(40);
        empty_state_box.set_margin_bottom(40);

        let welcome_icon = Image::from_icon_name("user-available-symbolic");
        welcome_icon.set_pixel_size(48);
        welcome_icon.set_css_classes(&["accent", "dim-label"]);
        empty_state_box.append(&welcome_icon);

        let welcome_title = Label::builder()
            .label(t!("chat.welcome_title"))
            .css_classes(["title-1"])
            .build();
        empty_state_box.append(&welcome_title);

        let welcome_subtitle = Label::builder()
            .label(t!("chat.welcome_subtitle"))
            .css_classes(["dim-label", "caption"])
            .wrap(true)
            .justify(gtk4::Justification::Center)
            .max_width_chars(50)
            .build();
        empty_state_box.append(&welcome_subtitle);

        let suggestions_box = Box::new(Orientation::Vertical, 6);
        suggestions_box.set_margin_top(12);

        let sugg1 = Button::builder()
            .label(t!("chat.suggestions.llm"))
            .css_classes(["chat-suggestion-chip"])
            .build();
        let sugg2 = Button::builder()
            .label(t!("chat.suggestions.rust"))
            .css_classes(["chat-suggestion-chip"])
            .build();
        let sugg3 = Button::builder()
            .label(t!("chat.suggestions.vram"))
            .css_classes(["chat-suggestion-chip"])
            .build();

        suggestions_box.append(&sugg1);
        suggestions_box.append(&sugg2);
        suggestions_box.append(&sugg3);
        empty_state_box.append(&suggestions_box);

        messages_box.append(&empty_state_box);

        let messages_scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .child(&messages_box)
            .build();

        main_chat_box.append(&messages_scrolled);

        // 3. Bottom Input Bar
        let bottom_container = Box::new(Orientation::Vertical, 6);
        bottom_container.set_margin_start(20);
        bottom_container.set_margin_end(20);
        bottom_container.set_margin_bottom(16);

        let input_bar = Box::new(Orientation::Horizontal, 8);
        input_bar.set_css_classes(&["chat-input-bar"]);
        input_bar.set_valign(Align::End);

        let input_buffer = TextBuffer::new(None);
        let input_text_view = TextView::builder()
            .buffer(&input_buffer)
            .wrap_mode(WrapMode::Word)
            .hexpand(true)
            .vexpand(false)
            .accepts_tab(false)
            .top_margin(6)
            .bottom_margin(6)
            .left_margin(6)
            .right_margin(6)
            .valign(Align::Center)
            .build();

        let input_scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .propagate_natural_height(true)
            .max_content_height(180)
            .hexpand(true)
            .vexpand(false)
            .valign(Align::Center)
            .child(&input_text_view)
            .build();

        let btn_stop = Button::builder()
            .icon_name("media-playback-stop-symbolic")
            .tooltip_text(t!("chat.stop_tooltip"))
            .css_classes(["destructive-action", "circular", "chat-action-btn"])
            .valign(Align::End)
            .halign(Align::Center)
            .visible(false)
            .build();

        let btn_send = Button::builder()
            .icon_name("document-send-symbolic")
            .tooltip_text(t!("chat.send_tooltip"))
            .css_classes(["suggested-action", "circular", "chat-action-btn"])
            .valign(Align::End)
            .halign(Align::Center)
            .build();

        let actions_box = Box::new(Orientation::Horizontal, 4);
        actions_box.set_valign(Align::End);
        actions_box.set_margin_bottom(2);
        actions_box.append(&btn_stop);
        actions_box.append(&btn_send);

        input_bar.append(&input_scrolled);
        input_bar.append(&actions_box);
        bottom_container.append(&input_bar);

        let hint_label = Label::builder()
            .label(t!("chat.input_hint"))
            .css_classes(["caption", "dim-label"])
            .halign(Align::Center)
            .build();
        bottom_container.append(&hint_label);

        main_chat_box.append(&bottom_container);
        container.append(&main_chat_box);

        let chat_view = Self {
            container,
            sidebar,
            btn_toggle_sidebar,
            client,
            config,
            model_max_contexts,
            sessions,
            active_session_id,
            model_dropdown,
            model_ids,
            model_infos,
            model_status_badge,
            updating_dropdown,
            selected_model,
            messages_scrolled,
            messages_box,
            empty_state_box,
            active_assistant,
            input_text_view,
            input_buffer,
            btn_send,
            btn_stop,
            btn_clear,
            settings_popover,
            is_generating,
            generation_cancelled,
            send_callback: Rc::new(RefCell::new(None)),
            cancel_callback: Rc::new(RefCell::new(None)),
        };

        chat_view.setup_internal_events(sugg1, sugg2, sugg3);
        chat_view.render_sidebar_list();
        chat_view.load_active_session_into_ui();

        chat_view
    }

    pub fn widget(&self) -> &Box {
        &self.container
    }

    pub fn render_sidebar_list(&self) {
        let sessions = self.sessions.borrow().clone();
        let active_id = self.active_session_id.borrow().clone();
        self.sidebar.render_sessions(&sessions, &active_id);
    }

    fn session_load_context(&self) -> SessionLoadContext {
        SessionLoadContext {
            sessions: self.sessions.clone(),
            active_id: self.active_session_id.clone(),
            msgs_box: self.messages_box.clone(),
            empty_box: self.empty_state_box.clone(),
            scrolled: self.messages_scrolled.clone(),
            popover: self.settings_popover.clone(),
            sel_model: self.selected_model.clone(),
            model_dropdown: self.model_dropdown.clone(),
            model_ids: self.model_ids.clone(),
            config: self.config.clone(),
            client: self.client.clone(),
            model_max_contexts: self.model_max_contexts.clone(),
        }
    }

    fn load_session_data(ctx: &SessionLoadContext) {
        let active_id = ctx.active_id.borrow().clone();
        let sessions = ctx.sessions.borrow().clone();

        if let Some(session) = sessions.iter().find(|s| s.id == active_id) {
            let mut child = ctx.msgs_box.first_child();
            while let Some(w) = child {
                let next = w.next_sibling();
                if w != ctx.empty_box {
                    ctx.msgs_box.remove(&w);
                }
                child = next;
            }

            if session.messages.is_empty() {
                ctx.empty_box.set_visible(true);
            } else {
                ctx.empty_box.set_visible(false);
                for msg in &session.messages {
                    if msg.role == "user" {
                        Self::append_user_message_widget(&ctx.msgs_box, &msg.content);
                    } else {
                        let model_name = session.model.as_deref().unwrap_or("Assistant");
                        let asst = Self::append_assistant_message_widget(&ctx.msgs_box, model_name);
                        asst.text_label.set_label(&msg.content);
                        asst.status_label.set_visible(false);
                    }
                }
                Self::force_scroll_to_bottom(&ctx.scrolled);
            }

            if let Some(ref m) = session.model {
                *ctx.sel_model.borrow_mut() = Some(m.clone());
                let ids = ctx.model_ids.borrow();
                if let Some(pos) = ids.iter().position(|id| id == m || id.starts_with(m) || m.starts_with(id)) {
                    ctx.model_dropdown.set_selected(pos as u32);
                }
                ctx.popover.apply_model_config(m, &ctx.config, &ctx.client, &ctx.model_max_contexts);
            }

            if let Some(ref sp) = session.system_prompt {
                ctx.popover.set_system_prompt(sp);
            }

            if let Some(temp) = session.temperature {
                ctx.popover.set_temperature(temp);
            }

            if let Some(ctx_val) = session.num_ctx {
                ctx.popover.set_num_ctx(ctx_val);
            }
        }
    }

    pub fn load_active_session_into_ui(&self) {
        Self::load_session_data(&self.session_load_context());
    }

    fn setup_internal_events(&self, sugg1: Button, sugg2: Button, sugg3: Button) {
        let sidebar = self.sidebar.clone();
        self.btn_toggle_sidebar.connect_clicked(move |_| {
            sidebar.toggle_visibility();
        });

        let ctx_new = self.session_load_context();
        let in_buf = self.input_buffer.clone();
        let sidebar_new = self.sidebar.clone();

        self.sidebar.connect_new_chat(move || {
            let cur_model = ctx_new.sel_model.borrow().clone();
            let new_sess = ChatSession::new_empty(cur_model.clone());
            let new_id = new_sess.id.clone();

            ctx_new.sessions.borrow_mut().insert(0, new_sess);
            *ctx_new.active_id.borrow_mut() = new_id.clone();

            ChatSession::save_all(&ctx_new.sessions.borrow());
            in_buf.set_text("");

            sidebar_new.render_sessions(&ctx_new.sessions.borrow(), &new_id);

            Self::load_session_data(&ctx_new);

            if let Some(ref m) = cur_model {
                ctx_new.popover.apply_model_config(m, &ctx_new.config, &ctx_new.client, &ctx_new.model_max_contexts);
            }
        });

        let ctx_sel = self.session_load_context();
        let sidebar_sel = self.sidebar.clone();

        self.sidebar.connect_session_selected(move |sess_id: String| {
            *ctx_sel.active_id.borrow_mut() = sess_id.clone();
            sidebar_sel.render_sessions(&ctx_sel.sessions.borrow(), &sess_id);

            Self::load_session_data(&ctx_sel);
        });

        let ctx_del = self.session_load_context();
        let sidebar_del = self.sidebar.clone();

        self.sidebar.connect_session_deleted(move |sess_id: String| {
            let mut sess = ctx_del.sessions.borrow_mut();
            sess.retain(|item| item.id != sess_id);

            if sess.is_empty() {
                let new_blank = ChatSession::new_empty(None);
                *ctx_del.active_id.borrow_mut() = new_blank.id.clone();
                sess.push(new_blank);
            } else if *ctx_del.active_id.borrow() == sess_id {
                *ctx_del.active_id.borrow_mut() = sess[0].id.clone();
            }

            ChatSession::save_all(&sess);
            let active_now = ctx_del.active_id.borrow().clone();
            sidebar_del.render_sessions(&sess, &active_now);
            drop(sess);

            Self::load_session_data(&ctx_del);
        });

        let in_buf1 = self.input_buffer.clone();
        sugg1.connect_clicked(move |_| {
            in_buf1.set_text(&t!("chat.suggestions.llm_prompt"));
        });

        let in_buf2 = self.input_buffer.clone();
        sugg2.connect_clicked(move |_| {
            in_buf2.set_text(&t!("chat.suggestions.rust_prompt"));
        });

        let in_buf3 = self.input_buffer.clone();
        sugg3.connect_clicked(move |_| {
            in_buf3.set_text(&t!("chat.suggestions.vram_prompt"));
        });

        let key_controller = EventControllerKey::new();
        let btn_send_clone = self.btn_send.clone();
        let is_gen_key = self.is_generating.clone();

        key_controller.connect_key_pressed(move |_, keyval, _, state| {
            if (keyval == Key::Return || keyval == Key::KP_Enter)
                && !state.contains(gtk4::gdk::ModifierType::SHIFT_MASK) {
                    if !*is_gen_key.borrow() {
                        btn_send_clone.emit_clicked();
                    }
                    return glib::Propagation::Stop;
                }
            glib::Propagation::Proceed
        });
        self.input_text_view.add_controller(key_controller);

        let sel_model = self.selected_model.clone();
        let model_ids_c = self.model_ids.clone();
        let is_updating_dd = self.updating_dropdown.clone();
        let badge_c = self.model_status_badge.clone();
        let infos_c = self.model_infos.clone();
        let cfg_c = self.config.clone();
        let client_c = self.client.clone();
        let max_ctx_c = self.model_max_contexts.clone();
        let popover_c = self.settings_popover.clone();

        self.model_dropdown.connect_selected_notify(move |dd| {
            if *is_updating_dd.borrow() {
                return;
            }
            let idx = dd.selected() as usize;
            let ids = model_ids_c.borrow();
            if let Some(id) = ids.get(idx) {
                *sel_model.borrow_mut() = Some(id.clone());
                Self::update_badge_for_model(&badge_c, id, &infos_c.borrow());
                popover_c.apply_model_config(id, &cfg_c, &client_c, &max_ctx_c);
            }
        });
    }

    fn update_badge_for_model(
        badge: &Label,
        model_id: &str,
        infos: &HashMap<String, (HardwareFitness, bool, u64, bool)>,
    ) {
        if let Some(&(fitness, is_vram, size, is_cloud)) = infos.get(model_id) {
            if is_cloud {
                badge.set_markup(&format!("<span foreground='#3584e4'>☁</span> <b>{}</b>", t!("chat.cloud_remote_model")));
                badge.set_css_classes(&["caption", "accent"]);
            } else if is_vram {
                match fitness {
                    HardwareFitness::FullGpu => {
                        badge.set_markup(&format!(
                            "<span foreground='#2ec27e'>●</span> <b>{}</b> ({})",
                            t!("chat.badge_vram"),
                            format_gib(size)
                        ));
                        badge.set_css_classes(&["caption", "success"]);
                    }
                    HardwareFitness::GpuOffload => {
                        badge.set_markup(&format!(
                            "<span foreground='#ff7800'>●</span> <b>{}</b> ({})",
                            t!("chat.badge_hybrid"),
                            format_gib(size)
                        ));
                        badge.set_css_classes(&["caption", "warning"]);
                    }
                    HardwareFitness::Insufficient => {
                        badge.set_markup(&format!(
                            "<span foreground='#e01b24'>●</span> <b>{}</b> ({})",
                            t!("chat.badge_insufficient"),
                            format_gib(size)
                        ));
                        badge.set_css_classes(&["caption", "error"]);
                    }
                }
            } else {
                match fitness {
                    HardwareFitness::FullGpu => {
                        badge.set_markup(&format!(
                            "<span foreground='#2ec27e'>○</span> <b>{}</b> ({})",
                            t!("chat.badge_on_disk_full_gpu"),
                            format_gib(size)
                        ));
                        badge.set_css_classes(&["caption", "dim-label"]);
                    }
                    HardwareFitness::GpuOffload => {
                        badge.set_markup(&format!(
                            "<span foreground='#ff7800'>○</span> <b>{}</b> ({})",
                            t!("chat.badge_on_disk_hybrid"),
                            format_gib(size)
                        ));
                        badge.set_css_classes(&["caption", "dim-label"]);
                    }
                    HardwareFitness::Insufficient => {
                        badge.set_markup(&format!(
                            "<span foreground='#e01b24'>○</span> <b>{}</b> ({})",
                            t!("chat.badge_on_disk_insufficient"),
                            format_gib(size)
                        ));
                        badge.set_css_classes(&["caption", "error"]);
                    }
                }
            }
        } else {
            badge.set_markup("<span alpha='50%'>—</span>");
            badge.set_css_classes(&["caption", "dim-label"]);
        }
    }

    pub fn update_models(
        &self,
        installed: &[ModelTag],
        running: &[ModelPs],
        snap: &HardwareSnapshot,
    ) {
        *self.updating_dropdown.borrow_mut() = true;

        let mut preloaded_items: Vec<(String, String, HardwareFitness, bool, u64)> = Vec::new();
        let mut on_disk_items: Vec<(String, String, HardwareFitness, bool, u64)> = Vec::new();
        let mut new_infos = HashMap::new();

        for m in installed {
            let is_cloud = m.is_cloud();
            let (is_in_vram, vram_size) = ModelNameUtils::check_model_running(&m.name, m.size, running);
            let fitness = HubManager::evaluate_fitness(m.size, snap);
            let caps = detect_icons_for_model_tag(m);

            let color_hex = if is_cloud {
                "#3584e4"
            } else {
                match fitness {
                    HardwareFitness::FullGpu => "#2ec27e",
                    HardwareFitness::GpuOffload => "#ff7800",
                    HardwareFitness::Insufficient => "#e01b24",
                }
            };

            let status_char = if is_cloud {
                "☁"
            } else if is_in_vram {
                "●"
            } else {
                "○"
            };

            let size_text = if is_cloud {
                "Cloud".to_string()
            } else {
                format_gib(m.size)
            };

            let markup_label = format!(
                "<span foreground='{}'>{}</span> <b>{}</b>  <span alpha='65%'>({})</span>{}",
                color_hex, status_char, m.name, size_text, caps
            );

            new_infos.insert(m.name.clone(), (fitness, is_in_vram, if is_in_vram { vram_size } else { m.size }, is_cloud));

            if is_in_vram {
                preloaded_items.push((m.name.clone(), markup_label, fitness, is_in_vram, vram_size));
            } else {
                on_disk_items.push((m.name.clone(), markup_label, fitness, is_in_vram, m.size));
            }
        }

        for r in running {
            let r_name = &r.name;
            let already_included = installed.iter().any(|m| {
                let (matched, _) = ModelNameUtils::check_model_running(&m.name, m.size, std::slice::from_ref(r));
                matched
            });

            if !already_included {
                let is_cloud = r_name.ends_with(":cloud") || r_name.ends_with("-cloud") || r_name.contains("cloud");
                let caps = detect_icons_for_model_tag(&ModelTag {
                    name: r_name.clone(),
                    model: r.model.clone(),
                    remote_model: None,
                    remote_host: None,
                    modified_at: None,
                    size: r.size,
                    digest: r.digest.clone(),
                    details: r.details.clone(),
                });
                let vram_size = if r.size_vram > 0 { r.size_vram } else { r.size };
                let fitness = HubManager::evaluate_fitness(vram_size, snap);
                let color_hex = if is_cloud {
                    "#3584e4"
                } else {
                    match fitness {
                        HardwareFitness::FullGpu => "#2ec27e",
                        HardwareFitness::GpuOffload => "#ff7800",
                        HardwareFitness::Insufficient => "#e01b24",
                    }
                };
                let status_char = if is_cloud { "☁" } else { "●" };
                let size_text = if is_cloud {
                    "Cloud · In Memory".to_string()
                } else {
                    format!("In Memory · {}", format_gib(vram_size))
                };
                let markup_label = format!(
                    "<span foreground='{}'>{}</span> <b>{}</b>  <span alpha='65%'>({})</span>{}",
                    color_hex, status_char, r_name, size_text, caps
                );
                new_infos.insert(r_name.clone(), (fitness, true, vram_size, is_cloud));
                preloaded_items.push((r_name.clone(), markup_label, fitness, true, vram_size));
            }
        }

        *self.model_infos.borrow_mut() = new_infos;

        let mut final_ids = Vec::new();
        let mut final_labels = Vec::new();

        for (id, label, _, _, _) in preloaded_items {
            final_ids.push(id);
            final_labels.push(label);
        }
        for (id, label, _, _, _) in on_disk_items {
            final_ids.push(id);
            final_labels.push(label);
        }

        let label_refs: Vec<&str> = final_labels.iter().map(|s| s.as_str()).collect();
        let string_list = StringList::new(&label_refs);
        self.model_dropdown.set_model(Some(&string_list));

        let prev_selected = self.selected_model.borrow().clone();
        let mut new_selection_idx = 0;

        if let Some(ref prev) = prev_selected {
            if let Some(pos) = final_ids.iter().position(|id| id == prev || id.starts_with(prev) || prev.starts_with(id)) {
                new_selection_idx = pos;
            }
        }

        if !final_ids.is_empty() {
            let active_id = final_ids[new_selection_idx].clone();
            *self.selected_model.borrow_mut() = Some(active_id.clone());
            self.model_dropdown.set_selected(new_selection_idx as u32);
            Self::update_badge_for_model(&self.model_status_badge, &active_id, &self.model_infos.borrow());
            self.settings_popover.apply_model_config(&active_id, &self.config, &self.client, &self.model_max_contexts);
        } else {
            *self.selected_model.borrow_mut() = None;
            Self::update_badge_for_model(&self.model_status_badge, "", &self.model_infos.borrow());
        }

        *self.model_ids.borrow_mut() = final_ids;
        *self.updating_dropdown.borrow_mut() = false;
    }

    pub fn select_model(&self, model_name: &str) {
        let ids = self.model_ids.borrow();
        if let Some(pos) = ids.iter().position(|id| {
            id == model_name
                || id.starts_with(model_name)
                || model_name.starts_with(id)
                || id.trim_end_matches(":latest") == model_name.trim_end_matches(":latest")
        }) {
            self.model_dropdown.set_selected(pos as u32);
            let target_id = ids[pos].clone();
            *self.selected_model.borrow_mut() = Some(target_id.clone());
            Self::update_badge_for_model(&self.model_status_badge, &target_id, &self.model_infos.borrow());
            self.settings_popover.apply_model_config(&target_id, &self.config, &self.client, &self.model_max_contexts);

            let cur_act_id = self.active_session_id.borrow().clone();
            let mut sess = self.sessions.borrow_mut();
            if let Some(target) = sess.iter_mut().find(|s| s.id == cur_act_id) {
                if target.messages.is_empty() {
                    target.model = Some(target_id);
                }
            }
        }
    }

    pub fn selected_model_name(&self) -> Option<String> {
        self.selected_model.borrow().clone()
    }

    pub fn connect_send<F>(&self, f: F)
    where
        F: Fn(String, Vec<ChatMessage>, Option<String>, Option<f64>, Option<u32>) + 'static,
    {
        *self.send_callback.borrow_mut() = Some(StdBox::new(f));

        let send_cb = self.send_callback.clone();
        let input_buf = self.input_buffer.clone();
        let sessions_rc = self.sessions.clone();
        let active_id_rc = self.active_session_id.clone();
        let sel_m = self.selected_model.clone();
        let is_gen = self.is_generating.clone();
        let popover_c = self.settings_popover.clone();

        let msgs_box = self.messages_box.clone();
        let empty_box = self.empty_state_box.clone();
        let scrolled = self.messages_scrolled.clone();
        let active_asst = self.active_assistant.clone();
        let btn_send_c = self.btn_send.clone();
        let btn_stop_c = self.btn_stop.clone();
        let canc_flag = self.generation_cancelled.clone();
        let sidebar_c = self.sidebar.clone();

        self.btn_send.connect_clicked(move |_| {
            if *is_gen.borrow() {
                return;
            }

            let text = input_buf
                .text(&input_buf.start_iter(), &input_buf.end_iter(), false)
                .to_string()
                .trim()
                .to_string();

            if text.is_empty() {
                return;
            }

            let model = match sel_m.borrow().clone() {
                Some(m) if !m.is_empty() => m,
                _ => return,
            };

            empty_box.set_visible(false);
            input_buf.set_text("");

            let user_msg = ChatMessage::user(text.clone());
            let cur_act_id = active_id_rc.borrow().clone();

            {
                let mut sess = sessions_rc.borrow_mut();
                if let Some(target) = sess.iter_mut().find(|s| s.id == cur_act_id) {
                    if target.messages.is_empty() || ChatSession::is_default_title(&target.title) {
                        target.title = ChatSession::generate_title(&text);
                    }
                    target.model = Some(model.clone());
                    target.messages.push(user_msg);
                    target.updated_at = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                }
                ChatSession::save_all(&sess);
            }

            sidebar_c.render_sessions(&sessions_rc.borrow(), &cur_act_id);
            Self::append_user_message_widget(&msgs_box, &text);

            let asst_widgets = Self::append_assistant_message_widget(&msgs_box, &model);
            *active_asst.borrow_mut() = Some(asst_widgets);
            Self::force_scroll_to_bottom(&scrolled);

            *is_gen.borrow_mut() = true;
            canc_flag.store(false, Ordering::SeqCst);
            btn_send_c.set_visible(false);
            btn_stop_c.set_visible(true);

            let system_prompt = popover_c.system_prompt();
            let temperature = popover_c.temperature();
            let num_ctx = popover_c.num_ctx();

            let active_sess_messages = {
                let sess = sessions_rc.borrow();
                sess.iter()
                    .find(|s| s.id == cur_act_id)
                    .map(|s| s.messages.clone())
                    .unwrap_or_default()
            };

            if let Some(ref cb) = *send_cb.borrow() {
                cb(model, active_sess_messages, system_prompt, temperature, num_ctx);
            }
        });
    }

    pub fn connect_cancel<F: Fn() + 'static>(&self, f: F) {
        *self.cancel_callback.borrow_mut() = Some(StdBox::new(f));

        let canc_cb = self.cancel_callback.clone();
        let canc_flag = self.generation_cancelled.clone();
        let btn_send_c = self.btn_send.clone();
        let btn_stop_c = self.btn_stop.clone();
        let is_gen = self.is_generating.clone();
        let active_asst = self.active_assistant.clone();

        self.btn_stop.connect_clicked(move |_| {
            canc_flag.store(true, Ordering::SeqCst);
            *is_gen.borrow_mut() = false;
            btn_stop_c.set_visible(false);
            btn_send_c.set_visible(true);

            if let Some(ref mut asst) = *active_asst.borrow_mut() {
                asst.status_label.set_label(&t!("chat.status_interrupted"));
                asst.status_label.set_visible(true);
            }

            if let Some(ref cb) = *canc_cb.borrow() {
                cb();
            }
        });
    }

    pub fn connect_clear(&self) {
        let sessions_rc = self.sessions.clone();
        let active_id_rc = self.active_session_id.clone();
        let msgs_box = self.messages_box.clone();
        let empty_box = self.empty_state_box.clone();
        let active_asst = self.active_assistant.clone();
        let is_gen = self.is_generating.clone();
        let btn_send_c = self.btn_send.clone();
        let btn_stop_c = self.btn_stop.clone();
        let canc_flag = self.generation_cancelled.clone();
        let sidebar_c = self.sidebar.clone();

        self.btn_clear.connect_clicked(move |_| {
            canc_flag.store(true, Ordering::SeqCst);
            *is_gen.borrow_mut() = false;
            btn_stop_c.set_visible(false);
            btn_send_c.set_visible(true);

            let cur_act = active_id_rc.borrow().clone();
            {
                let mut sess = sessions_rc.borrow_mut();
                if let Some(target) = sess.iter_mut().find(|s| s.id == cur_act) {
                    target.messages.clear();
                }
                ChatSession::save_all(&sess);
            }

            *active_asst.borrow_mut() = None;

            let mut child = msgs_box.first_child();
            while let Some(widget) = child {
                let next = widget.next_sibling();
                if widget != empty_box {
                    msgs_box.remove(&widget);
                }
                child = next;
            }

            empty_box.set_visible(true);
            sidebar_c.render_sessions(&sessions_rc.borrow(), &cur_act);
        });
    }

    pub fn handle_stream_chunk(
        &self,
        token: &str,
        realtime_tok_sec: Option<f64>,
        ttft_ms: Option<f64>,
    ) {
        if let Some(ref mut asst) = *self.active_assistant.borrow_mut() {
            let mut text = asst.full_text.borrow_mut();
            text.push_str(token);
            asst.text_label.set_label(&text);

            asst.metrics_box.set_visible(true);
            asst.status_label.set_visible(false);

            if let Some(speed) = realtime_tok_sec {
                asst.speed_badge
                    .set_label(&format!("⚡ {:.1} tok/s", speed));
                asst.speed_badge.set_visible(true);
            }

            if let Some(ttft) = ttft_ms {
                asst.ttft_badge
                    .set_label(&format!("⏱️ TTFT: {:.0} ms", ttft));
                asst.ttft_badge.set_visible(true);
            }

            Self::auto_scroll_if_at_bottom(&self.messages_scrolled);
        }
    }

    pub fn handle_stream_done(&self, metrics: &ChatMetrics) {
        *self.is_generating.borrow_mut() = false;
        self.btn_stop.set_visible(false);
        self.btn_send.set_visible(true);

        if let Some(ref mut asst) = *self.active_assistant.borrow_mut() {
            let text = asst.full_text.borrow().clone();

            let cur_act_id = self.active_session_id.borrow().clone();
            {
                let mut sess = self.sessions.borrow_mut();
                if let Some(target) = sess.iter_mut().find(|s| s.id == cur_act_id) {
                    target.messages.push(ChatMessage::assistant(text));
                    target.updated_at = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                }
                ChatSession::save_all(&sess);
            }

            self.render_sidebar_list();

            asst.metrics_box.set_visible(true);
            asst.status_label.set_visible(false);

            if metrics.total_duration_ms > 0.0 {
                asst.metrics_box.set_tooltip_text(Some(&format!("Total duration: {:.2} s", metrics.total_duration_ms / 1000.0)));
            }

            if metrics.tokens_per_sec > 0.0 {
                asst.speed_badge
                    .set_label(&format!("⚡ {:.1} tok/s", metrics.tokens_per_sec));
                asst.speed_badge.set_visible(true);
            }

            if metrics.ttft_ms > 0.0 {
                asst.ttft_badge
                    .set_label(&format!("⏱️ TTFT: {:.0} ms", metrics.ttft_ms));
                asst.ttft_badge.set_visible(true);
            }

            if metrics.eval_count > 0 {
                asst.tokens_badge
                    .set_label(&format!("🔢 {} tok (ctx: {})", metrics.eval_count, metrics.prompt_eval_count));
                asst.tokens_badge.set_visible(true);
            }

            if metrics.vram_bytes > 0 {
                asst.vram_badge
                    .set_label(&format!("💾 VRAM: {}", format_gib(metrics.vram_bytes)));
                asst.vram_badge.set_visible(true);
            }

            Self::auto_scroll_if_at_bottom(&self.messages_scrolled);
        }
    }

    pub fn handle_stream_error(&self, error: &str) {
        *self.is_generating.borrow_mut() = false;
        self.btn_stop.set_visible(false);
        self.btn_send.set_visible(true);

        if let Some(ref mut asst) = *self.active_assistant.borrow_mut() {
            let display_msg = if error.to_lowercase().contains("unauthorized") || error.contains("requires a subscription") {
                t!("chat.cloud_unauthorized_error")
            } else {
                format!("❌ Error: {}", error)
            };
            asst.status_label.set_label(&display_msg);
            asst.status_label.set_visible(true);
            asst.status_label.set_css_classes(&["caption", "error"]);
        }
    }

    fn append_user_message_widget(parent: &Box, text: &str) {
        let row = Box::new(Orientation::Horizontal, 0);
        row.set_halign(Align::End);
        row.set_margin_start(60);

        let bubble_box = Box::new(Orientation::Vertical, 4);
        bubble_box.set_css_classes(&["chat-user-bubble"]);

        let header_row = Box::new(Orientation::Horizontal, 8);
        let user_label = Label::builder()
            .label(t!("chat.user_label"))
            .css_classes(["chat-role-label"])
            .halign(Align::Start)
            .hexpand(true)
            .build();

        let btn_copy = Button::builder()
            .icon_name("edit-copy-symbolic")
            .tooltip_text(t!("chat.copy_tooltip"))
            .css_classes(["flat", "circular"])
            .valign(Align::Center)
            .build();

        let text_c = text.to_string();
        btn_copy.connect_clicked(move |_| {
            if let Some(display) = gtk4::gdk::Display::default() {
                display.clipboard().set_text(&text_c);
            }
        });

        header_row.append(&user_label);
        header_row.append(&btn_copy);
        bubble_box.append(&header_row);

        let content_label = Label::builder()
            .label(text)
            .wrap(true)
            .wrap_mode(gtk4::pango::WrapMode::WordChar)
            .selectable(true)
            .halign(Align::Start)
            .xalign(0.0)
            .build();
        bubble_box.append(&content_label);

        row.append(&bubble_box);
        parent.append(&row);
    }

    fn append_assistant_message_widget(parent: &Box, model_name: &str) -> AssistantMessageWidgets {
        let row = Box::new(Orientation::Horizontal, 0);
        row.set_halign(Align::Start);
        row.set_margin_end(60);

        let bubble_box = Box::new(Orientation::Vertical, 6);
        bubble_box.set_css_classes(&["chat-assistant-bubble"]);

        let header_row = Box::new(Orientation::Horizontal, 8);
        let model_label = Label::builder()
            .label(format!("🤖 {}", model_name))
            .css_classes(["chat-role-label", "accent"])
            .halign(Align::Start)
            .hexpand(true)
            .build();

        let btn_copy = Button::builder()
            .icon_name("edit-copy-symbolic")
            .tooltip_text(t!("chat.copy_tooltip"))
            .css_classes(["flat", "circular"])
            .valign(Align::Center)
            .build();

        header_row.append(&model_label);
        header_row.append(&btn_copy);
        bubble_box.append(&header_row);

        let full_text = Rc::new(RefCell::new(String::new()));
        let full_text_copy = full_text.clone();

        btn_copy.connect_clicked(move |_| {
            let txt = full_text_copy.borrow().clone();
            if !txt.is_empty() {
                if let Some(display) = gtk4::gdk::Display::default() {
                    display.clipboard().set_text(&txt);
                }
            }
        });

        let text_label = Label::builder()
            .label("")
            .wrap(true)
            .wrap_mode(gtk4::pango::WrapMode::WordChar)
            .selectable(true)
            .halign(Align::Start)
            .xalign(0.0)
            .build();
        bubble_box.append(&text_label);

        let status_label = Label::builder()
            .label(t!("chat.generating_status"))
            .css_classes(["caption", "dim-label"])
            .halign(Align::Start)
            .build();
        bubble_box.append(&status_label);

        let metrics_box = Box::new(Orientation::Horizontal, 6);
        metrics_box.set_margin_top(4);
        metrics_box.set_visible(false);

        let speed_badge = Label::builder()
            .label("⚡ -- tok/s")
            .css_classes(["chat-metric-chip", "speed"])
            .visible(false)
            .build();

        let ttft_badge = Label::builder()
            .label("⏱️ TTFT: -- ms")
            .css_classes(["chat-metric-chip", "ttft"])
            .visible(false)
            .build();

        let tokens_badge = Label::builder()
            .label("🔢 0 tok")
            .css_classes(["chat-metric-chip"])
            .visible(false)
            .build();

        let vram_badge = Label::builder()
            .label("💾 VRAM: --")
            .css_classes(["chat-metric-chip", "vram"])
            .visible(false)
            .build();

        metrics_box.append(&speed_badge);
        metrics_box.append(&ttft_badge);
        metrics_box.append(&tokens_badge);
        metrics_box.append(&vram_badge);
        bubble_box.append(&metrics_box);

        row.append(&bubble_box);
        parent.append(&row);

        AssistantMessageWidgets {
            text_label,
            metrics_box,
            speed_badge,
            ttft_badge,
            tokens_badge,
            vram_badge,
            status_label,
            full_text,
        }
    }

    fn auto_scroll_if_at_bottom(scrolled: &ScrolledWindow) {
        let adj = scrolled.vadjustment();
        let val = adj.value();
        let page = adj.page_size();
        let upper = adj.upper();
        let is_at_bottom = (val + page) >= (upper - 40.0);

        if is_at_bottom {
            glib::idle_add_local_once(move || {
                adj.set_value(adj.upper() - adj.page_size());
            });
        }
    }

    fn force_scroll_to_bottom(scrolled: &ScrolledWindow) {
        let adj = scrolled.vadjustment();
        glib::idle_add_local_once(move || {
            adj.set_value(adj.upper() - adj.page_size());
        });
    }
}
