use crate::core::config::ApiProfile;
use crate::t;
use adw::prelude::*;
use gtk4::gio::SimpleAction;
use gtk4::{
    Box, Button, DropDown, Label, MenuButton, Orientation, PopoverMenu, ProgressBar, Spinner,
    StringList,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let b = bytes as f64;
    if b >= GIB {
        format!("{:.2} Go", b / GIB)
    } else if b >= MIB {
        format!("{:.1} Mo", b / MIB)
    } else if b >= KIB {
        format!("{:.0} Ko", b / KIB)
    } else {
        format!("{} o", bytes)
    }
}

fn format_speed(bytes_per_sec: f64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    if bytes_per_sec >= GIB {
        format!("{:.2} Go/s", bytes_per_sec / GIB)
    } else if bytes_per_sec >= MIB {
        format!("{:.1} Mo/s", bytes_per_sec / MIB)
    } else if bytes_per_sec >= KIB {
        format!("{:.0} Ko/s", bytes_per_sec / KIB)
    } else {
        format!("{:.0} o/s", bytes_per_sec)
    }
}

struct DownloadTaskItem {
    metrics_label: Label,
    status_label: Label,
    progress_bar: ProgressBar,
    btn_action: Button,
    container: Box,
    is_finished: bool,
    last_completed: u64,
    last_time: std::time::Instant,
    current_speed: f64,
}

pub struct Header {
    container: adw::HeaderBar,
    window_title: adw::WindowTitle,
    status_label: Label,
    status_box: Box,
    profile_dropdown: DropDown,
    profile_ids: Rc<RefCell<Vec<String>>>,
    updating_dropdown: Rc<RefCell<bool>>,
    downloads_badge: Label,
    download_tasks_box: Box,
    download_empty_box: Box,
    active_downloads: Rc<RefCell<HashMap<String, DownloadTaskItem>>>,
    spinner: Spinner,
}

impl Default for Header {
    fn default() -> Self {
        Self::new()
    }
}

impl Header {
    pub fn new() -> Self {
        let container = adw::HeaderBar::new();

        // 1. Center WindowTitle
        let window_title = adw::WindowTitle::builder()
            .title(t!("app.title"))
            .subtitle(t!("nav.home"))
            .build();
        container.set_title_widget(Some(&window_title));

        // ========================================================
        // LEFT SECTION: Brand, Profile Selector & Daemon Status
        // ========================================================
        let left_box = Box::new(Orientation::Horizontal, 8);
        left_box.set_valign(gtk4::Align::Center);
        left_box.set_margin_start(4);

        // 1. Brand Chip (Logo + "NeuraDex")
        let brand_chip_box = Box::new(Orientation::Horizontal, 8);
        brand_chip_box.set_css_classes(&["brand-chip"]);
        brand_chip_box.set_valign(gtk4::Align::Center);
        brand_chip_box.set_tooltip_text(Some(&t!("header.brand_tooltip")));

        let brand_icon = gtk4::Image::from_icon_name("io.github.bazinfla.NeuraDex");
        brand_icon.set_pixel_size(26);
        brand_icon.set_valign(gtk4::Align::Center);
        brand_chip_box.append(&brand_icon);

        let brand_label = Label::builder()
            .label("NeuraDex")
            .valign(gtk4::Align::Center)
            .build();
        brand_chip_box.append(&brand_label);

        left_box.append(&brand_chip_box);

        // 2. Daemon Status (Pill Badge)
        let status_box = Box::new(Orientation::Horizontal, 6);
        status_box.set_css_classes(&["daemon-badge", "connecting"]);
        status_box.set_valign(gtk4::Align::Center);

        let status_label = Label::builder()
            .label(t!("header.daemon_connecting"))
            .css_classes(["caption"])
            .build();

        status_box.append(&status_label);
        left_box.append(&status_box);

        container.pack_start(&left_box);

        // ========================================================
        // RIGHT SECTION: Profile Selector, Downloads, Spinner & Hamburger Menu
        // ========================================================
        let right_box = Box::new(Orientation::Horizontal, 6);
        right_box.set_valign(gtk4::Align::Center);
        right_box.set_margin_end(4);

        // 1. Fast API / SSH Profile Selector (Pill Dropdown)
        let profile_pill_box = Box::new(Orientation::Horizontal, 4);
        profile_pill_box.set_css_classes(&["profile-dropdown-pill"]);
        profile_pill_box.set_valign(gtk4::Align::Center);

        let key_icon = gtk4::Image::from_icon_name("dialog-password-symbolic");
        key_icon.set_pixel_size(16);
        key_icon.set_margin_start(4);
        profile_pill_box.append(&key_icon);

        let profile_strings = StringList::new(&[&t!("header.no_profile")]);
        let profile_dropdown = DropDown::builder()
            .model(&profile_strings)
            .valign(gtk4::Align::Center)
            .tooltip_text(t!("header.profile_tooltip"))
            .build();
        profile_pill_box.append(&profile_dropdown);

        right_box.append(&profile_pill_box);

        let spinner = Spinner::builder().spinning(false).visible(false).build();

        // Downloads Popover
        let popover_dl = gtk4::Popover::builder()
            .autohide(true)
            .build();

        let popover_content = Box::new(Orientation::Vertical, 10);
        popover_content.set_margin_top(12);
        popover_content.set_margin_bottom(12);
        popover_content.set_margin_start(14);
        popover_content.set_margin_end(14);
        popover_content.set_width_request(420);

        let pop_header = Box::new(Orientation::Horizontal, 8);
        pop_header.set_valign(gtk4::Align::Center);

        let pop_icon = gtk4::Image::from_icon_name("folder-download-symbolic");
        pop_icon.set_pixel_size(18);

        let pop_title = Label::builder()
            .label(t!("header.downloads_title"))
            .css_classes(["heading"])
            .halign(gtk4::Align::Start)
            .hexpand(true)
            .build();

        pop_header.append(&pop_icon);
        pop_header.append(&pop_title);
        popover_content.append(&pop_header);

        let pop_sep = gtk4::Separator::new(Orientation::Horizontal);
        popover_content.append(&pop_sep);

        let download_empty_box = Box::new(Orientation::Vertical, 6);
        download_empty_box.set_halign(gtk4::Align::Center);
        download_empty_box.set_margin_top(18);
        download_empty_box.set_margin_bottom(18);

        let empty_icon = gtk4::Image::from_icon_name("folder-download-symbolic");
        empty_icon.set_pixel_size(36);
        empty_icon.set_opacity(0.35);

        let download_empty_label = Label::builder()
            .label(t!("header.downloads_empty"))
            .css_classes(["caption", "dim-label"])
            .halign(gtk4::Align::Center)
            .build();

        download_empty_box.append(&empty_icon);
        download_empty_box.append(&download_empty_label);
        popover_content.append(&download_empty_box);

        let scroll_tasks = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .max_content_height(340)
            .propagate_natural_height(true)
            .build();

        let download_tasks_box = Box::new(Orientation::Vertical, 6);
        scroll_tasks.set_child(Some(&download_tasks_box));
        popover_content.append(&scroll_tasks);

        popover_dl.set_child(Some(&popover_content));

        let btn_downloads_box = Box::new(Orientation::Horizontal, 4);
        let dl_icon = gtk4::Image::from_icon_name("folder-download-symbolic");
        let downloads_badge = Label::builder()
            .label("0")
            .css_classes(["caption", "numeric"])
            .visible(false)
            .build();
        btn_downloads_box.append(&dl_icon);
        btn_downloads_box.append(&downloads_badge);

        let btn_downloads = MenuButton::builder()
            .child(&btn_downloads_box)
            .tooltip_text(t!("header.downloads_tooltip"))
            .css_classes(["flat"])
            .popover(&popover_dl)
            .build();

        // Main Hamburger Menu with Navigation and Info Sections
        let menu = gtk4::gio::Menu::new();

        let nav_section = gtk4::gio::Menu::new();
        nav_section.append(Some(&t!("nav.home")), Some("win.nav_home"));
        nav_section.append(Some(&t!("nav.chat")), Some("win.nav_chat"));
        nav_section.append(Some(&t!("nav.hub")), Some("win.nav_hub"));
        nav_section.append(Some(&t!("nav.logs")), Some("win.nav_logs"));
        nav_section.append(Some(&t!("nav.settings")), Some("win.nav_settings"));
        menu.append_section(Some(&t!("nav.section")), &nav_section);

        let info_section = gtk4::gio::Menu::new();
        info_section.append(Some(&t!("app.about")), Some("win.about"));
        info_section.append(Some(&t!("app.doc_github")), Some("win.github"));
        menu.append_section(Some(&t!("nav.info_section")), &info_section);

        let popover_menu = PopoverMenu::from_model(Some(&menu));
        let menu_btn = MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text(t!("header.menu_tooltip"))
            .css_classes(["flat"])
            .popover(&popover_menu)
            .build();

        right_box.append(&spinner);
        right_box.append(&btn_downloads);
        right_box.append(&menu_btn);

        container.pack_end(&right_box);

        Self {
            container,
            window_title,
            status_label,
            status_box,
            profile_dropdown,
            profile_ids: Rc::new(RefCell::new(Vec::new())),
            updating_dropdown: Rc::new(RefCell::new(false)),
            downloads_badge,
            download_tasks_box,
            download_empty_box,
            active_downloads: Rc::new(RefCell::new(HashMap::new())),
            spinner,
        }
    }

    pub fn widget(&self) -> &adw::HeaderBar {
        &self.container
    }

    pub fn set_view_subtitle(&self, subtitle: &str) {
        self.window_title.set_subtitle(subtitle);
    }

    pub fn set_status_connected(&self, version: &str) {
        self.status_label
            .set_label(&format!("🟢 Ollama v{}", version));
        self.status_box
            .set_css_classes(&["daemon-badge", "connected"]);
        self.status_box
            .set_tooltip_text(Some(&t!("header.daemon_normal_tooltip")));
    }

    pub fn set_status_disconnected(&self, error: &str) {
        self.status_label.set_label(&t!("header.daemon_disconnected"));
        self.status_box
            .set_css_classes(&["daemon-badge", "disconnected"]);
        self.status_box.set_tooltip_text(Some(error));
    }

    pub fn set_refreshing(&self, refreshing: bool) {
        self.spinner.set_visible(refreshing);
        self.spinner.set_spinning(refreshing);
    }

    pub fn update_download_progress<FC: Fn(String) + 'static>(
        &self,
        tag: &str,
        status: &str,
        completed: Option<u64>,
        total: Option<u64>,
        on_cancel: FC,
    ) {
        let mut tasks = self.active_downloads.borrow_mut();
        if let Some(task) = tasks.get(tag) {
            if task.is_finished {
                self.download_tasks_box.remove(&task.container);
                tasks.remove(tag);
            }
        }

        let now = std::time::Instant::now();

        if let Some(task) = tasks.get_mut(tag) {
            task.status_label.set_label(status);
            task.progress_bar.set_visible(true);

            if let (Some(comp), Some(tot)) = (completed, total) {
                let elapsed = now.duration_since(task.last_time).as_secs_f64();
                if elapsed >= 0.25 {
                    let delta = comp.saturating_sub(task.last_completed);
                    let inst_speed = delta as f64 / elapsed;
                    task.current_speed = if task.current_speed > 0.0 {
                        task.current_speed * 0.6 + inst_speed * 0.4
                    } else {
                        inst_speed
                    };
                    task.last_completed = comp;
                    task.last_time = now;
                }

                let pct = if tot > 0 {
                    ((comp as f64 / tot as f64) * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                let remaining = tot.saturating_sub(comp);
                let speed_str = if task.current_speed > 1024.0 {
                    format_speed(task.current_speed)
                } else {
                    "-- Mo/s".to_string()
                };

                let remaining_word = t!("header.remaining");
                let metrics_text = format!(
                    "{:.0}% - {} / {} ({} {}) - {}",
                    pct,
                    format_bytes(comp),
                    format_bytes(tot),
                    format_bytes(remaining),
                    remaining_word,
                    speed_str
                );
                task.metrics_label.set_label(&metrics_text);

                if tot > 0 {
                    task.progress_bar.set_fraction((comp as f64 / tot as f64).clamp(0.0, 1.0));
                }
            } else {
                task.progress_bar.pulse();
                task.metrics_label.set_label(status);
            }
        } else {
            let row = Box::new(Orientation::Vertical, 6);
            row.set_css_classes(&["download-popover-card"]);

            let top_row = Box::new(Orientation::Horizontal, 8);
            top_row.set_valign(gtk4::Align::Center);

            let model_icon = gtk4::Image::from_icon_name("cube-symbolic");
            model_icon.set_pixel_size(16);
            model_icon.set_opacity(0.85);

            let name_label = Label::builder()
                .label(tag)
                .css_classes(["heading"])
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .halign(gtk4::Align::Start)
                .hexpand(true)
                .build();

            let btn_action = Button::builder()
                .icon_name("media-playback-stop-symbolic")
                .tooltip_text(t!("header.btn_cancel_download_tooltip"))
                .css_classes(["flat", "circular", "destructive-action"])
                .valign(gtk4::Align::Center)
                .build();

            let tag_c = tag.to_string();
            btn_action.connect_clicked(move |_| {
                on_cancel(tag_c.clone());
            });

            top_row.append(&model_icon);
            top_row.append(&name_label);
            top_row.append(&btn_action);
            row.append(&top_row);

            let progress_bar = ProgressBar::builder()
                .css_classes(["download-progress-bar"])
                .build();
            if let (Some(comp), Some(tot)) = (completed, total) {
                if tot > 0 {
                    progress_bar.set_fraction((comp as f64 / tot as f64).clamp(0.0, 1.0));
                }
            }
            row.append(&progress_bar);

            let metrics_text = if let (Some(comp), Some(tot)) = (completed, total) {
                let pct = if tot > 0 { ((comp as f64 / tot as f64) * 100.0).clamp(0.0, 100.0) } else { 0.0 };
                let remaining = tot.saturating_sub(comp);
                let remaining_word = t!("header.remaining");
                format!(
                    "{:.0}% - {} / {} ({} {}) - -- Mo/s",
                    pct,
                    format_bytes(comp),
                    format_bytes(tot),
                    format_bytes(remaining),
                    remaining_word
                )
            } else {
                format!("0% - {}", status)
            };

            let metrics_label = Label::builder()
                .label(&metrics_text)
                .css_classes(["download-metrics-label"])
                .halign(gtk4::Align::Start)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .build();
            row.append(&metrics_label);

            let status_label = Label::builder()
                .label(status)
                .css_classes(["caption", "dim-label"])
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .halign(gtk4::Align::Start)
                .build();
            row.append(&status_label);

            self.download_tasks_box.append(&row);

            let initial_comp = completed.unwrap_or(0);
            tasks.insert(tag.to_string(), DownloadTaskItem {
                metrics_label,
                status_label,
                progress_bar,
                btn_action,
                container: row,
                is_finished: false,
                last_completed: initial_comp,
                last_time: now,
                current_speed: 0.0,
            });
        }

        let active_count = tasks.values().filter(|t| !t.is_finished).count();
        let total_count = tasks.len();
        self.download_empty_box.set_visible(total_count == 0);
        self.downloads_badge.set_label(&active_count.to_string());
        self.downloads_badge.set_visible(active_count > 0);
    }

    pub fn mark_download_finished(&self, tag: &str, final_status: &str, _is_success: bool) {
        let mut tasks = self.active_downloads.borrow_mut();
        if let Some(task) = tasks.get_mut(tag) {
            task.is_finished = true;
            task.progress_bar.set_visible(false);
            task.status_label.set_label(final_status);

            if _is_success {
                task.container.set_css_classes(&["download-popover-card", "finished"]);
                task.metrics_label.set_label(&t!("header.download_done"));
                task.metrics_label.set_css_classes(&["download-metrics-label", "finished"]);
            } else {
                task.container.set_css_classes(&["download-popover-card", "failed"]);
                task.metrics_label.set_label(&t!("header.download_failed"));
                task.metrics_label.set_css_classes(&["download-metrics-label", "failed"]);
            }

            // Replace Stop button with a Close icon to remove the entry from the list
            task.btn_action.set_icon_name("window-close-symbolic");
            task.btn_action.set_tooltip_text(Some(&t!("header.btn_remove_download")));
            task.btn_action.set_css_classes(&["flat", "circular"]);

            let tasks_rc = self.active_downloads.clone();
            let tasks_box = self.download_tasks_box.clone();
            let empty_box = self.download_empty_box.clone();
            let badge_lbl = self.downloads_badge.clone();
            let tag_to_remove = tag.to_string();

            task.btn_action.connect_clicked(move |_| {
                let mut map = tasks_rc.borrow_mut();
                if let Some(item) = map.remove(&tag_to_remove) {
                    tasks_box.remove(&item.container);
                }
                let active = map.values().filter(|t| !t.is_finished).count();
                let total = map.len();
                empty_box.set_visible(total == 0);
                badge_lbl.set_label(&active.to_string());
                badge_lbl.set_visible(active > 0);
            });
        }

        let active_count = tasks.values().filter(|t| !t.is_finished).count();
        let total_count = tasks.len();
        self.download_empty_box.set_visible(total_count == 0);
        self.downloads_badge.set_label(&active_count.to_string());
        self.downloads_badge.set_visible(active_count > 0);
    }

    pub fn update_profiles(&self, profiles: &[ApiProfile], active_id: Option<&str>) {
        *self.updating_dropdown.borrow_mut() = true;

        let mut collected_ids = Vec::new();
        let mut new_strings = Vec::new();

        // Explicit allowlist: only display Ollama daemon identities (SSH keys & Ollama cloud/remote keys)
        let ollama_profiles: Vec<&ApiProfile> = profiles
            .iter()
            .filter(|p| p.provider_type.is_ollama_identity())
            .collect();

        if ollama_profiles.is_empty() {
            new_strings.push(t!("header.no_profile_item"));
        } else {
            for p in ollama_profiles {
                let badge = match p.is_valid {
                    Some(true) => "🟢",
                    Some(false) => "🔴",
                    None => "🟡",
                };
                new_strings.push(format!("{} {}", badge, p.name));
                collected_ids.push(p.id.clone());
            }
        }

        let target_pos = if let Some(act_id) = active_id {
            collected_ids.iter().position(|id| id == act_id).map(|p| p as u32).unwrap_or(0)
        } else {
            0
        };

        let ids_changed = *self.profile_ids.borrow() != collected_ids;

        if ids_changed || self.profile_dropdown.model().is_none() {
            *self.profile_ids.borrow_mut() = collected_ids;
            let slice_refs: Vec<&str> = new_strings.iter().map(|s| s.as_str()).collect();
            let new_model = StringList::new(&slice_refs);
            self.profile_dropdown.set_model(Some(&new_model));
        }

        if self.profile_dropdown.selected() != target_pos {
            self.profile_dropdown.set_selected(target_pos);
        }

        *self.updating_dropdown.borrow_mut() = false;
    }

    pub fn connect_profile_changed<F: Fn(&str) + 'static>(&self, f: F) {
        let ids_rc = self.profile_ids.clone();
        let is_updating = self.updating_dropdown.clone();
        self.profile_dropdown.connect_selected_notify(move |dd| {
            if *is_updating.borrow() {
                return;
            }
            let idx = dd.selected() as usize;
            let target_id = {
                let ids = ids_rc.borrow();
                ids.get(idx).cloned()
            };
            if let Some(id) = target_id {
                f(&id);
            }
        });
    }

    /// Registers global window actions (Navigation, About dialog, GitHub)
    pub fn setup_actions(window: &adw::ApplicationWindow, view_stack: &adw::ViewStack, header: &Rc<Header>) {
        // Navigation Home (Models)
        let stack_home = view_stack.clone();
        let hdr_home = header.clone();
        let action_home = SimpleAction::new("nav_home", None);
        action_home.connect_activate(move |_, _| {
            stack_home.set_visible_child_name("instances");
            hdr_home.set_view_subtitle(&t!("nav.home"));
        });
        window.add_action(&action_home);

        // Navigation Chat & Playground
        let stack_chat = view_stack.clone();
        let hdr_chat = header.clone();
        let action_chat = SimpleAction::new("nav_chat", None);
        action_chat.connect_activate(move |_, _| {
            stack_chat.set_visible_child_name("chat");
            hdr_chat.set_view_subtitle(&t!("nav.chat"));
        });
        window.add_action(&action_chat);

        // Navigation Hub
        let stack_hub = view_stack.clone();
        let hdr_hub = header.clone();
        let action_hub = SimpleAction::new("nav_hub", None);
        action_hub.connect_activate(move |_, _| {
            stack_hub.set_visible_child_name("hub");
            hdr_hub.set_view_subtitle(&t!("nav.hub"));
        });
        window.add_action(&action_hub);

        // Navigation Logs
        let stack_logs = view_stack.clone();
        let hdr_logs = header.clone();
        let action_logs = SimpleAction::new("nav_logs", None);
        action_logs.connect_activate(move |_, _| {
            stack_logs.set_visible_child_name("logs");
            hdr_logs.set_view_subtitle(&t!("nav.logs"));
        });
        window.add_action(&action_logs);

        // Navigation Settings
        let stack_set = view_stack.clone();
        let hdr_set = header.clone();
        let action_settings = SimpleAction::new("nav_settings", None);
        action_settings.connect_activate(move |_, _| {
            stack_set.set_visible_child_name("settings");
            hdr_set.set_view_subtitle(&t!("nav.settings"));
        });
        window.add_action(&action_settings);

        // Action About
        let window_weak = window.downgrade();
        let action_about = SimpleAction::new("about", None);
        action_about.connect_activate(move |_, _| {
            if let Some(win) = window_weak.upgrade() {
                show_about_dialog(&win);
            }
        });
        window.add_action(&action_about);

        // Action GitHub
        let action_github = SimpleAction::new("github", None);
        action_github.connect_activate(move |_, _| {
            let _ = gtk4::gio::AppInfo::launch_default_for_uri(
                "https://github.com/BazinFla/NeuraDex",
                None::<&gtk4::gio::AppLaunchContext>,
            );
        });
        window.add_action(&action_github);
    }
}

fn show_about_dialog(parent: &adw::ApplicationWindow) {
    let dialog = adw::Dialog::builder()
        .title(t!("app.about"))
        .content_width(460)
        .content_height(480)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    let header_bar = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header_bar);

    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .propagate_natural_height(true)
        .build();

    let clamp = adw::Clamp::builder()
        .maximum_size(420)
        .tightening_threshold(360)
        .build();

    let content_box = Box::new(Orientation::Vertical, 16);
    content_box.set_margin_top(20);
    content_box.set_margin_bottom(24);
    content_box.set_margin_start(16);
    content_box.set_margin_end(16);

    // App Icon
    let app_icon = gtk4::Image::from_icon_name("io.github.bazinfla.NeuraDex");
    app_icon.set_pixel_size(96);
    app_icon.set_halign(gtk4::Align::Center);
    content_box.append(&app_icon);

    // Title and Version
    let title_box = Box::new(Orientation::Vertical, 4);
    title_box.set_halign(gtk4::Align::Center);

    let app_title = Label::builder()
        .label(t!("app.title"))
        .css_classes(["title-1"])
        .build();
    let version_badge = Label::builder()
        .label(concat!("v", env!("CARGO_PKG_VERSION")))
        .css_classes(["caption", "dim-label"])
        .build();

    title_box.append(&app_title);
    title_box.append(&version_badge);
    content_box.append(&title_box);

    // Comments / Tagline
    let comments_label = Label::builder()
        .label(t!("app.comments"))
        .css_classes(["body"])
        .wrap(true)
        .wrap_mode(gtk4::pango::WrapMode::WordChar)
        .justify(gtk4::Justification::Center)
        .halign(gtk4::Align::Center)
        .build();
    content_box.append(&comments_label);

    // Details group
    let details_group = adw::PreferencesGroup::new();

    // Developer row
    let dev_row = adw::ActionRow::builder()
        .title(t!("about.developer"))
        .subtitle("BazinFla")
        .build();
    let dev_icon = gtk4::Image::from_icon_name("avatar-default-symbolic");
    dev_row.add_prefix(&dev_icon);
    details_group.add(&dev_row);

    // Website row
    let web_row = adw::ActionRow::builder()
        .title(t!("about.website"))
        .subtitle("https://github.com/BazinFla/NeuraDex")
        .activatable(true)
        .build();
    let web_icon = gtk4::Image::from_icon_name("globe-symbolic");
    web_row.add_prefix(&web_icon);
    let open_btn = Button::builder()
        .icon_name("external-link-symbolic")
        .css_classes(["flat", "circular"])
        .valign(gtk4::Align::Center)
        .tooltip_text(t!("about.open_link_tooltip"))
        .build();
    open_btn.connect_clicked(|_| {
        let _ = gtk4::gio::AppInfo::launch_default_for_uri(
            "https://github.com/BazinFla/NeuraDex",
            None::<&gtk4::gio::AppLaunchContext>,
        );
    });
    web_row.connect_activated(|_| {
        let _ = gtk4::gio::AppInfo::launch_default_for_uri(
            "https://github.com/BazinFla/NeuraDex",
            None::<&gtk4::gio::AppLaunchContext>,
        );
    });
    web_row.add_suffix(&open_btn);
    details_group.add(&web_row);

    // License row
    let lic_row = adw::ActionRow::builder()
        .title(t!("about.license"))
        .subtitle(t!("about.license_details"))
        .activatable(true)
        .build();
    let lic_icon = gtk4::Image::from_icon_name("emblem-documents-symbolic");
    lic_row.add_prefix(&lic_icon);
    let lic_open_btn = Button::builder()
        .icon_name("external-link-symbolic")
        .css_classes(["flat", "circular"])
        .valign(gtk4::Align::Center)
        .tooltip_text(t!("about.open_link_tooltip"))
        .build();
    lic_open_btn.connect_clicked(|_| {
        let _ = gtk4::gio::AppInfo::launch_default_for_uri(
            "https://www.gnu.org/licenses/gpl-3.0.html",
            None::<&gtk4::gio::AppLaunchContext>,
        );
    });
    lic_row.connect_activated(|_| {
        let _ = gtk4::gio::AppInfo::launch_default_for_uri(
            "https://www.gnu.org/licenses/gpl-3.0.html",
            None::<&gtk4::gio::AppLaunchContext>,
        );
    });
    lic_row.add_suffix(&lic_open_btn);
    details_group.add(&lic_row);

    // Diagnostic row in About dialog
    let diag_row = adw::ActionRow::builder()
        .title(t!("settings.diagnostic_dialog_title"))
        .subtitle(t!("settings.diagnostic_row_sub"))
        .activatable(true)
        .build();
    let diag_icon = gtk4::Image::from_icon_name("utilities-system-monitor-symbolic");
    diag_row.add_prefix(&diag_icon);
    let diag_btn = Button::builder()
        .icon_name("edit-copy-symbolic")
        .css_classes(["flat", "circular"])
        .valign(gtk4::Align::Center)
        .tooltip_text(t!("settings.btn_copy_diagnostic_tooltip"))
        .build();
    let diag_btn_c = diag_btn.clone();
    diag_btn.connect_clicked(move |_| {
        let btn = diag_btn_c.clone();
        let cfg = crate::core::AppConfig::load();
        crate::core::spawn_async(
            async move {
                crate::core::generate_diagnostic_report(&cfg).await
            },
            move |report| {
                if let Some(display) = gtk4::gdk::Display::default() {
                    display.clipboard().set_text(&report);
                }
                btn.set_icon_name("object-select-symbolic");
                let btn_reset = btn.clone();
                glib::timeout_add_local_once(std::time::Duration::from_secs(2), move || {
                    btn_reset.set_icon_name("edit-copy-symbolic");
                });
            }
        );
    });
    let diag_btn_click = diag_btn.clone();
    diag_row.connect_activated(move |_| {
        diag_btn_click.emit_clicked();
    });
    diag_row.add_suffix(&diag_btn);
    details_group.add(&diag_row);

    content_box.append(&details_group);

    clamp.set_child(Some(&content_box));
    scrolled.set_child(Some(&clamp));
    toolbar_view.set_content(Some(&scrolled));
    dialog.set_child(Some(&toolbar_view));

    dialog.present(Some(parent));
}

