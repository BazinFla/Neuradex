use crate::api::types::{ModelNameUtils, ModelPs, ModelTag};
use crate::core::hardware::estimator::{format_bytes, format_gib};
use crate::core::hardware::HardwareSnapshot;
use crate::t;
use crate::ui::components::{ModelCard, RunningModelInfoDialog, VramGauge};
use adw::prelude::*;
use gtk4::{
    Align, Box, Button, Label, LevelBar, ListBox, ListBoxRow, Orientation,
    ScrolledWindow, Separator,
};

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

type UnloadCallback = Rc<dyn Fn(&str)>;

pub struct InstancesView {
    container: Box,
    installed_list: ListBox,
    installed_count_label: Label,
    empty_installed_label: Label,
    running_box: Box,
    btn_unload_all: Button,
    vram_gauge: VramGauge,
    ram_bar: LevelBar,
    ram_label: Label,
    cpu_label: Label,
    cards: Rc<RefCell<Vec<Rc<ModelCard>>>>,
    installed_structure_fingerprint: Rc<RefCell<String>>,
    installed_loaded_fingerprint: Rc<RefCell<String>>,
    loading_models: Rc<RefCell<HashSet<String>>>,
    last_running: Rc<RefCell<Vec<ModelPs>>>,
    last_on_unload: Rc<RefCell<Option<UnloadCallback>>>,
}

impl Default for InstancesView {
    fn default() -> Self {
        Self::new()
    }
}

impl InstancesView {
    pub fn new() -> Self {
        let container = Box::new(Orientation::Horizontal, 20);
        container.set_margin_start(20);
        container.set_margin_end(20);
        container.set_margin_top(20);
        container.set_margin_bottom(20);
        container.set_hexpand(true);
        container.set_vexpand(true);

        // ==========================================
        // LEFT COLUMN: Installed Models
        // ==========================================
        let left_col = Box::new(Orientation::Vertical, 10);
        left_col.set_hexpand(true);
        left_col.set_vexpand(true);

        let installed_header_box = Box::new(Orientation::Horizontal, 8);
        installed_header_box.set_valign(Align::Center);

        let installed_title = Label::builder()
            .label(t!("instances.installed_title"))
            .css_classes(["heading"])
            .halign(Align::Start)
            .hexpand(true)
            .build();

        let installed_count_label = Label::builder()
            .label(t!("instances.installed_count", count = "0"))
            .css_classes(["dim-label", "caption"])
            .halign(Align::End)
            .build();

        installed_header_box.append(&installed_title);
        installed_header_box.append(&installed_count_label);
        left_col.append(&installed_header_box);

        let installed_list = ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .focus_on_click(false)
            .can_focus(false)
            .build();

        let scrolled_left = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .child(&installed_list)
            .build();

        left_col.append(&scrolled_left);

        let empty_installed_label = Label::builder()
            .label(t!("instances.empty_installed"))
            .css_classes(["dim-label"])
            .visible(false)
            .build();
        left_col.append(&empty_installed_label);

        // Vertical separator
        let separator = Separator::new(Orientation::Vertical);

        // ==========================================
        // RIGHT COLUMN: Monitoring & Active VRAM
        // ==========================================
        let right_col = Box::new(Orientation::Vertical, 14);
        right_col.set_hexpand(true);
        right_col.set_vexpand(true);

        // Hardware Monitoring Header
        let hw_title = Label::builder()
            .label(t!("instances.hardware_title"))
            .css_classes(["heading"])
            .halign(Align::Start)
            .build();
        right_col.append(&hw_title);

        // GPU VRAM Gauge
        let vram_gauge = VramGauge::new();
        right_col.append(vram_gauge.widget());

        // System RAM & CPU Card (styled identically to GPU cards)
        let sys_card = Box::new(Orientation::Vertical, 6);
        sys_card.set_css_classes(&["card"]);
        sys_card.set_margin_start(2);
        sys_card.set_margin_end(2);
        sys_card.set_margin_top(2);
        sys_card.set_margin_bottom(2);

        let sys_inner = Box::new(Orientation::Vertical, 6);
        sys_inner.set_margin_start(12);
        sys_inner.set_margin_end(12);
        sys_inner.set_margin_top(10);
        sys_inner.set_margin_bottom(10);
        sys_card.append(&sys_inner);

        let sys_header_box = Box::new(Orientation::Horizontal, 12);
        sys_header_box.set_hexpand(true);

        let sys_title = Label::builder()
            .label(t!("instances.sys_ram_title"))
            .css_classes(["heading"])
            .halign(Align::Start)
            .hexpand(true)
            .build();

        let ram_label = Label::builder()
            .label("0.0 / 0.0 GB (0%)")
            .css_classes(["numeric", "heading"])
            .halign(Align::End)
            .build();

        sys_header_box.append(&sys_title);
        sys_header_box.append(&ram_label);

        let ram_bar = LevelBar::builder()
            .min_value(0.0)
            .max_value(1.0)
            .value(0.0)
            .height_request(10)
            .build();

        let cpu_label = Label::builder()
            .label(t!("instances.calculating"))
            .css_classes(["dim-label", "caption"])
            .halign(Align::Start)
            .build();

        sys_inner.append(&sys_header_box);
        sys_inner.append(&ram_bar);
        sys_inner.append(&cpu_label);

        right_col.append(&sys_card);


        // Active VRAM Header (Title + Bulk Flush button)
        let active_vram_header = Box::new(Orientation::Horizontal, 8);
        active_vram_header.set_valign(Align::Center);
        active_vram_header.set_margin_top(8);
        active_vram_header.set_margin_bottom(2);

        let active_vram_title = Label::builder()
            .label(t!("instances.loaded_title"))
            .css_classes(["heading"])
            .halign(Align::Start)
            .hexpand(true)
            .build();

        let btn_unload_all = Button::builder()
            .label(t!("instances.unload_all"))
            .css_classes(["destructive-action", "pill"])
            .tooltip_text(t!("instances.unload_all_tooltip"))
            .valign(Align::Center)
            .sensitive(false)
            .build();

        active_vram_header.append(&active_vram_title);
        active_vram_header.append(&btn_unload_all);
        right_col.append(&active_vram_header);

        let running_box = Box::new(Orientation::Vertical, 8);
        running_box.set_hexpand(true);
        running_box.set_vexpand(true);

        let scrolled_right = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .child(&running_box)
            .build();

        right_col.append(&scrolled_right);

        // Assembly
        container.append(&left_col);
        container.append(&separator);
        container.append(&right_col);

        Self {
            container,
            installed_list,
            installed_count_label,
            empty_installed_label,
            running_box,
            btn_unload_all,
            vram_gauge,
            ram_bar,
            ram_label,
            cpu_label,
            cards: Rc::new(RefCell::new(Vec::new())),
            installed_structure_fingerprint: Rc::new(RefCell::new(String::new())),
            installed_loaded_fingerprint: Rc::new(RefCell::new(String::new())),
            loading_models: Rc::new(RefCell::new(HashSet::new())),
            last_running: Rc::new(RefCell::new(Vec::new())),
            last_on_unload: Rc::new(RefCell::new(None)),
        }
    }

    pub fn widget(&self) -> &Box {
        &self.container
    }

    pub fn set_model_loading(&self, model_name: &str, loading: bool) {
        for card in self.cards.borrow().iter() {
            if card.model_name() == model_name {
                card.set_loading(loading);
            }
        }
        if loading {
            self.loading_models.borrow_mut().insert(model_name.to_string());
        } else {
            self.loading_models.borrow_mut().remove(model_name);
        }
        self.render_running();
    }

    pub fn update_installed_models<FA, FC, FD, FS>(
        &self,
        models: &[ModelTag],
        running_names: &HashSet<String>,
        on_action: FA,
        on_chat: FC,
        on_delete: FD,
        on_settings: FS,
    )
    where
        FA: Fn(&str, bool) + Clone + 'static,
        FC: Fn(&str) + Clone + 'static,
        FD: Fn(&str) + Clone + 'static,
        FS: Fn(&str, u64) + Clone + 'static,
    {
        // 1. Structural fingerprint: models and their sizes (detects added, removed, or resized models)
        let mut structure_fp = String::new();
        for m in models {
            structure_fp.push_str(&m.name);
            structure_fp.push(':');
            structure_fp.push_str(&m.size.to_string());
            structure_fp.push(';');
        }

        // 2. Loaded fingerprint: loaded/running status in VRAM
        let mut loaded_fp = String::new();
        for m in models {
            let is_loaded = ModelNameUtils::is_tag_installed(running_names.iter().map(|s| s.as_str()), &m.name);
            loaded_fp.push(if is_loaded { '1' } else { '0' });
        }

        let structure_unchanged = *self.installed_structure_fingerprint.borrow() == structure_fp
            && self.cards.borrow().len() == models.len();
        let loaded_unchanged = *self.installed_loaded_fingerprint.borrow() == loaded_fp;

        // Case A: Nothing changed -> early exit, zero work
        if structure_unchanged && loaded_unchanged {
            return;
        }

        // Case B: Models on disk are identical, only loaded states changed -> in-place update!
        if structure_unchanged {
            *self.installed_loaded_fingerprint.borrow_mut() = loaded_fp;
            let cards = self.cards.borrow();
            for (i, m) in models.iter().enumerate() {
                let is_loaded = ModelNameUtils::is_tag_installed(running_names.iter().map(|s| s.as_str()), &m.name);
                if let Some(card) = cards.get(i).filter(|c| c.model_name() == m.name) {
                    if card.is_loaded() != is_loaded {
                        card.set_loaded(is_loaded);
                    }
                } else if let Some(card) = cards.iter().find(|c| c.model_name() == m.name) {
                    if card.is_loaded() != is_loaded {
                        card.set_loaded(is_loaded);
                    }
                }
            }
            return;
        }

        // Case C: Models changed (added, deleted, resized, or reordered) -> full rebuild
        *self.installed_structure_fingerprint.borrow_mut() = structure_fp;
        *self.installed_loaded_fingerprint.borrow_mut() = loaded_fp;

        // Clear list
        self.installed_list.unselect_all();
        while let Some(child) = self.installed_list.first_child() {
            self.installed_list.remove(&child);
        }
        self.cards.borrow_mut().clear();

        self.installed_count_label
            .set_label(&t!("instances.installed_count", count = models.len().to_string()));

        if models.is_empty() {
            self.empty_installed_label.set_visible(true);
            self.installed_list.set_visible(false);
        } else {
            self.empty_installed_label.set_visible(false);
            self.installed_list.set_visible(true);

            for model in models {
                let is_loaded = ModelNameUtils::is_tag_installed(running_names.iter().map(|s| s.as_str()), &model.name);

                let card = Rc::new(ModelCard::new(model, is_loaded));
                let on_action_clone = on_action.clone();
                let on_chat_clone = on_chat.clone();
                let on_delete_clone = on_delete.clone();
                let on_settings_clone = on_settings.clone();

                card.connect_action(move |name, loaded| {
                    on_action_clone(name, loaded);
                });

                card.connect_chat(move |name| {
                    on_chat_clone(name);
                });

                card.connect_settings(move |name, size| {
                    on_settings_clone(name, size);
                });

                card.connect_delete(move |name| {
                    on_delete_clone(name);
                });

                let row = ListBoxRow::builder()
                    .selectable(false)
                    .activatable(false)
                    .can_focus(false)
                    .child(card.widget())
                    .build();

                self.installed_list.append(&row);
                self.cards.borrow_mut().push(card);
            }
        }
    }

    pub fn update_running<FU>(&self, running: &[ModelPs], on_unload: FU)
    where
        FU: Fn(&str) + Clone + 'static,
    {
        *self.last_running.borrow_mut() = running.to_vec();
        *self.last_on_unload.borrow_mut() = Some(Rc::new(on_unload));

        // Remove from loading_models those that are now in running list
        {
            let mut loading = self.loading_models.borrow_mut();
            for ps in running {
                loading.remove(&ps.name);
                if let Some(ref m) = ps.model {
                    loading.remove(m);
                }
            }
        }

        self.render_running();
    }

    fn render_running(&self) {
        while let Some(child) = self.running_box.first_child() {
            self.running_box.remove(&child);
        }

        let running = self.last_running.borrow().clone();
        let loading = self.loading_models.borrow().clone();
        let on_unload_opt = self.last_on_unload.borrow().clone();

        if running.is_empty() && loading.is_empty() {
            let empty_card = Box::new(Orientation::Vertical, 8);
            empty_card.set_css_classes(&["running-empty-state-card"]);
            empty_card.set_valign(Align::Center);
            empty_card.set_halign(Align::Fill);

            let icon_box = Box::new(Orientation::Vertical, 0);
            icon_box.set_halign(Align::Center);
            icon_box.set_css_classes(&["empty-state-icon-badge"]);
            let icon = gtk4::Image::from_icon_name("weather-clear-symbolic");
            icon.set_pixel_size(28);
            icon_box.append(&icon);

            let title = Label::builder()
                .label(t!("instances.unoccupied_title"))
                .css_classes(["heading"])
                .halign(Align::Center)
                .justify(gtk4::Justification::Center)
                .build();

            let subtitle = Label::builder()
                .label(t!("instances.unoccupied_subtitle"))
                .css_classes(["caption", "dim-label"])
                .halign(Align::Center)
                .justify(gtk4::Justification::Center)
                .wrap(true)
                .build();

            let tip_pill = Label::builder()
                .label(t!("instances.unoccupied_tip"))
                .css_classes(["empty-state-tip-pill"])
                .halign(Align::Center)
                .justify(gtk4::Justification::Center)
                .margin_top(4)
                .build();

            empty_card.append(&icon_box);
            empty_card.append(&title);
            empty_card.append(&subtitle);
            empty_card.append(&tip_pill);
            self.running_box.append(&empty_card);
        } else {
            // 1. Models actively running in Ollama
            for ps in &running {
                let cpu_offload = ps.size.saturating_sub(ps.size_vram);
                let is_full_gpu = cpu_offload == 0;
                let vram_ratio = if ps.size > 0 {
                    (ps.size_vram as f64 / ps.size as f64).clamp(0.0, 1.0)
                } else {
                    1.0
                };

                let card = Box::new(Orientation::Vertical, 8);
                if is_full_gpu {
                    card.set_css_classes(&["running-model-card"]);
                } else {
                    card.set_css_classes(&["running-model-card", "hybrid"]);
                }
                card.set_margin_start(8);
                card.set_margin_end(8);
                card.set_margin_top(4);
                card.set_margin_bottom(4);

                // --- 1. TOP ROW: Status icon + Name + Memory chip + Flush button ---
                let top_row = Box::new(Orientation::Horizontal, 8);
                top_row.set_valign(Align::Center);

                let status_icon = gtk4::Image::from_icon_name("media-playback-start-symbolic");
                status_icon.set_pixel_size(16);

                let name_label = Label::builder()
                    .label(&ps.name)
                    .css_classes(["heading"])
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .halign(Align::Start)
                    .hexpand(true)
                    .build();

                // VRAM vs RAM allocation chip
                let alloc_text = if is_full_gpu {
                    t!("instances.alloc_full_gpu", vram = format_bytes(ps.size_vram))
                } else {
                    t!("instances.alloc_hybrid", vram = format_bytes(ps.size_vram), ram = format_bytes(cpu_offload))
                };

                let alloc_chip = Label::builder()
                    .label(&alloc_text)
                    .css_classes(if is_full_gpu {
                        vec!["running-alloc-chip"]
                    } else {
                        vec!["running-alloc-chip", "hybrid"]
                    })
                    .valign(Align::Center)
                    .build();

                // Info button ("i")
                let btn_info = Button::builder()
                    .icon_name("dialog-information-symbolic")
                    .css_classes(["flat", "circular", "running-info-btn"])
                    .tooltip_text(t!("instances.running_info_tooltip"))
                    .valign(Align::Center)
                    .build();

                let ps_for_info = ps.clone();
                let parent_widget = self.container.clone();
                btn_info.connect_clicked(move |_| {
                    RunningModelInfoDialog::show(Some(&parent_widget), ps_for_info.clone());
                });

                // Unload button (VRAM flush)
                let btn_flush = Button::builder()
                    .label(t!("instances.unload_btn"))
                    .css_classes(["destructive-action"])
                    .tooltip_text(t!("instances.unload_btn_tooltip"))
                    .valign(Align::Center)
                    .build();

                if let Some(ref on_unload) = on_unload_opt {
                    let on_unload_clone = on_unload.clone();
                    let model_name = ps.name.clone();
                    btn_flush.connect_clicked(move |_| {
                        on_unload_clone(&model_name);
                    });
                }

                top_row.append(&status_icon);
                top_row.append(&name_label);
                top_row.append(&alloc_chip);
                top_row.append(&btn_info);
                top_row.append(&btn_flush);
                card.append(&top_row);

                // --- 2. MEMORY ALLOCATION BAR ---
                let vram_bar = gtk4::ProgressBar::builder()
                    .fraction(vram_ratio)
                    .css_classes(if is_full_gpu {
                        vec!["running-vram-bar"]
                    } else {
                        vec!["running-vram-bar", "hybrid"]
                    })
                    .build();
                card.append(&vram_bar);

                // --- 3. BOTTOM ROW: Metadata badges & Expiration countdown ---
                let bottom_row = Box::new(Orientation::Horizontal, 8);
                bottom_row.set_valign(Align::Center);

                let mut meta_badges = Vec::new();

                if let Some(ref det) = ps.details {
                    if let Some(ref param) = det.parameter_size {
                        meta_badges.push(format!("🏷️ {}", param));
                    }
                    if let Some(ref quant) = det.quantization_level {
                        meta_badges.push(format!("⚙️ {}", quant));
                    }
                    if let Some(ref fam) = det.family {
                        meta_badges.push(format!("📦 {}", fam));
                    }
                }

                meta_badges.push(t!("instances.footprint", size = format_bytes(ps.size)));

                let meta_label = Label::builder()
                    .label(meta_badges.join("   •   "))
                    .css_classes(["caption", "dim-label"])
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .halign(Align::Start)
                    .hexpand(true)
                    .build();
                bottom_row.append(&meta_label);

                // Expiration timer
                let expire_text = if let Some(ref exp) = ps.expires_at {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(exp) {
                        let now = chrono::Utc::now();
                        let diff = dt.signed_duration_since(now);
                        if diff.num_seconds() > 0 {
                            let mins = diff.num_minutes();
                            let secs = diff.num_seconds() % 60;
                            t!("instances.expires_in", mins = mins.to_string(), secs = format!("{:02}", secs))
                        } else {
                            t!("instances.expired")
                        }
                    } else {
                        t!("instances.resident")
                    }
                } else {
                    t!("instances.resident")
                };

                let expire_chip = Label::builder()
                    .label(&expire_text)
                    .css_classes(["running-expire-chip"])
                    .valign(Align::Center)
                    .build();
                bottom_row.append(&expire_chip);

                card.append(&bottom_row);
                self.running_box.append(&card);
            }

            // 2. Loading models (Transferring tensors to GPU)
            for model_name in &loading {
                if running.iter().any(|p| p.name == *model_name || p.model.as_deref() == Some(model_name)) {
                    continue;
                }

                let card = Box::new(Orientation::Vertical, 8);
                card.set_css_classes(&["running-model-card", "loading"]);
                card.set_margin_start(8);
                card.set_margin_end(8);
                card.set_margin_top(4);
                card.set_margin_bottom(4);

                // Top row
                let top_row = Box::new(Orientation::Horizontal, 8);
                top_row.set_valign(Align::Center);

                let spinner = gtk4::Spinner::builder()
                    .spinning(true)
                    .build();

                let name_label = Label::builder()
                    .label(model_name)
                    .css_classes(["heading"])
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .halign(Align::Start)
                    .hexpand(true)
                    .build();

                let alloc_chip = Label::builder()
                    .label(t!("instances.loading_title"))
                    .css_classes(["running-alloc-chip", "loading"])
                    .valign(Align::Center)
                    .build();

                let loading_badge = Label::builder()
                    .label(t!("instances.loading_init"))
                    .css_classes(["caption", "dim-label"])
                    .valign(Align::Center)
                    .build();

                top_row.append(&spinner);
                top_row.append(&name_label);
                top_row.append(&alloc_chip);
                top_row.append(&loading_badge);
                card.append(&top_row);

                // Pulsing progress bar
                let loading_bar = gtk4::ProgressBar::builder()
                    .css_classes(["running-vram-bar", "loading"])
                    .pulse_step(0.2)
                    .build();
                loading_bar.pulse();
                card.append(&loading_bar);

                // Bottom row
                let bottom_row = Box::new(Orientation::Horizontal, 8);
                bottom_row.set_valign(Align::Center);

                let desc_label = Label::builder()
                    .label(t!("instances.loading_desc"))
                    .css_classes(["caption", "dim-label"])
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .halign(Align::Start)
                    .hexpand(true)
                    .build();
                bottom_row.append(&desc_label);

                let status_chip = Label::builder()
                    .label(t!("instances.loading_status"))
                    .css_classes(["running-expire-chip"])
                    .valign(Align::Center)
                    .build();
                bottom_row.append(&status_chip);

                card.append(&bottom_row);
                self.running_box.append(&card);
            }
        }

        self.btn_unload_all.set_sensitive(!running.is_empty());
    }

    pub fn connect_unload_all<F: Fn() + 'static>(&self, f: F) {
        self.btn_unload_all.connect_clicked(move |_| {
            f();
        });
    }

    pub fn set_unloading_all(&self, unloading: bool) {
        self.btn_unload_all.set_sensitive(!unloading);
        if unloading {
            self.btn_unload_all.set_label(&t!("instances.unloading"));
        } else {
            self.btn_unload_all.set_label(&t!("instances.unload_all"));
        }
    }

    pub fn update_hardware(&self, snap: &HardwareSnapshot) {
        self.vram_gauge.update_gpus(&snap.gpus, snap.gpu.as_ref());

        let ram_used = snap.cpu.ram_used;
        let ram_total = snap.cpu.ram_total;
        let ram_ratio = if ram_total > 0 {
            (ram_used as f64 / ram_total as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };

        self.ram_bar.set_value(ram_ratio);
        self.ram_label.set_label(&format!(
            "{} / {} ({:.0}%)",
            format_gib(ram_used),
            format_gib(ram_total),
            ram_ratio * 100.0
        ));

        self.cpu_label.set_label(&t!(
            "instances.cpu_label",
            name = snap.cpu.cpu_name.clone(),
            cores = snap.cpu.cpu_count.to_string(),
            load = format!("{:.0}", snap.cpu.cpu_usage_percent)
        ));
    }
}
