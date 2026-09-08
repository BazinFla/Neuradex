use crate::api::types::ModelTag;
use crate::core::hardware::estimator::format_bytes;
use crate::t;
use adw::prelude::*;
use gtk4::{Box, Button, Orientation, ProgressBar};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct ModelCard {
    container: Box,
    icon: gtk4::Image,
    subtitle_label: gtk4::Label,
    static_tags: Vec<String>,
    btn_action: Button,
    btn_chat: Button,
    btn_settings: Button,
    btn_delete: Button,
    progress_bar: ProgressBar,
    model_name: String,
    model_size: u64,
    is_loaded: Rc<Cell<bool>>,
    is_cloud: bool,
    pulse_source: Rc<RefCell<Option<glib::SourceId>>>,
    confirm_timeout: Rc<RefCell<Option<glib::SourceId>>>,
    is_confirming_delete: Rc<Cell<bool>>,
}

fn reset_delete_btn_state(
    is_confirming: &Cell<bool>,
    confirm_timeout: &RefCell<Option<glib::SourceId>>,
    btn_delete: &Button,
) {
    if is_confirming.get() {
        is_confirming.set(false);
        if let Some(source_id) = confirm_timeout.borrow_mut().take() {
            source_id.remove();
        }
        btn_delete.set_child(None::<&gtk4::Widget>);
        btn_delete.set_icon_name("user-trash-symbolic");
        btn_delete.set_css_classes(&["destructive-action", "flat", "circular"]);
        btn_delete.set_tooltip_text(Some(&t!("model_card.btn_delete_tooltip")));
    }
}

impl ModelCard {
    pub fn new(model: &ModelTag, is_loaded: bool) -> Self {
        let container = Box::new(Orientation::Vertical, 0);
        container.set_css_classes(&["card"]);
        container.set_margin_bottom(6);

        let is_cloud = model.is_cloud();

        let main_row = Box::new(Orientation::Horizontal, 12);
        main_row.set_margin_start(12);
        main_row.set_margin_end(12);
        main_row.set_margin_top(10);
        main_row.set_margin_bottom(10);
        main_row.set_valign(gtk4::Align::Center);

        // Subtitle technical information
        let mut static_tags = Vec::new();

        if !is_cloud {
            static_tags.push(format_bytes(model.size));
        }

        if let Some(ref details) = model.details {
            if let Some(ref quant) = details.quantization_level {
                if !quant.is_empty() {
                    static_tags.push(quant.clone());
                }
            }
            if let Some(ref param) = details.parameter_size {
                if !param.is_empty() {
                    static_tags.push(param.clone());
                }
            }
            if let Some(ref family) = details.family {
                if !family.is_empty() {
                    static_tags.push(family.clone());
                }
            }
        }

        if is_cloud {
            let host = model.remote_host.as_deref().unwrap_or("ollama.com");
            let clean_host = host.replace("https://", "").replace(":443", "");
            static_tags.push(t!("model_card.hosted_on", host = clean_host));
        }

        let mut info_tags = Vec::new();
        if is_cloud {
            info_tags.push(t!("model_card.cloud_model"));
        } else if is_loaded {
            info_tags.push(t!("model_card.active_vram"));
        }
        info_tags.extend(static_tags.clone());

        // Model Icon with dynamic logo resolution (color model -> mono model -> color creator -> mono creator -> default)
        let icon_name = Self::resolve_installed_model_icon(model);
        let icon = gtk4::Image::from_icon_name(&icon_name);
        icon.set_pixel_size(26);
        icon.set_valign(gtk4::Align::Center);
        if is_loaded {
            icon.add_css_class("loaded-model-icon");
        }

        let text_box = Box::new(Orientation::Vertical, 2);
        text_box.set_hexpand(true);
        text_box.set_valign(gtk4::Align::Center);

        let caps = crate::core::capabilities::detect_icons_for_model_tag(model);
        let title_text = if caps.is_empty() {
            model.name.clone()
        } else {
            format!("{}{}", model.name, caps)
        };

        let title_label = gtk4::Label::builder()
            .label(&title_text)
            .css_classes(["heading"])
            .halign(gtk4::Align::Start)
            .build();

        let subtitle_label = gtk4::Label::builder()
            .label(info_tags.join("  •  "))
            .css_classes(["caption", "dim-label"])
            .halign(gtk4::Align::Start)
            .build();

        text_box.append(&title_label);
        text_box.append(&subtitle_label);

        // Actions
        let actions_box = Box::new(Orientation::Horizontal, 6);
        actions_box.set_valign(gtk4::Align::Center);

        let btn_action = Button::builder().build();
        if is_cloud {
            btn_action.set_label(&t!("model_card.btn_cloud"));
            btn_action.set_tooltip_text(Some(&t!("model_card.btn_cloud_tooltip")));
            btn_action.set_css_classes(&["flat", "accent"]);
            btn_action.set_sensitive(false);
        } else if is_loaded {
            btn_action.set_label(&t!("model_card.btn_unload"));
            btn_action.set_tooltip_text(Some(&t!("model_card.btn_unload_tooltip")));
            btn_action.set_css_classes(&["destructive-action"]);
        } else {
            btn_action.set_label(&t!("model_card.btn_load"));
            btn_action.set_tooltip_text(Some(&t!("model_card.btn_load_tooltip")));
            btn_action.set_css_classes(&["suggested-action"]);
        }

        let btn_chat = Button::builder()
            .icon_name("document-send-symbolic")
            .tooltip_text(t!("model_card.btn_chat_tooltip"))
            .css_classes(["flat", "circular"])
            .build();

        let btn_settings_tooltip = if is_cloud {
            t!("model_card.btn_settings_disabled")
        } else {
            t!("model_card.btn_settings_tooltip")
        };

        let btn_settings = Button::builder()
            .icon_name("emblem-system-symbolic")
            .tooltip_text(&btn_settings_tooltip)
            .css_classes(["flat", "circular"])
            .sensitive(!is_cloud)
            .build();

        let btn_delete = Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text(t!("model_card.btn_delete_tooltip"))
            .css_classes(["destructive-action", "flat", "circular"])
            .build();

        actions_box.append(&btn_action);
        actions_box.append(&btn_chat);
        actions_box.append(&btn_settings);
        actions_box.append(&btn_delete);

        main_row.append(&icon);
        main_row.append(&text_box);
        main_row.append(&actions_box);

        // Bottom progress bar
        let progress_bar = ProgressBar::builder()
            .visible(false)
            .height_request(4)
            .css_classes(["accent"])
            .build();

        container.append(&main_row);
        container.append(&progress_bar);

        Self {
            container,
            icon,
            subtitle_label,
            static_tags,
            btn_action,
            btn_chat,
            btn_settings,
            btn_delete,
            progress_bar,
            model_name: model.name.clone(),
            model_size: model.size,
            is_loaded: Rc::new(Cell::new(is_loaded)),
            is_cloud,
            pulse_source: Rc::new(RefCell::new(None)),
            confirm_timeout: Rc::new(RefCell::new(None)),
            is_confirming_delete: Rc::new(Cell::new(false)),
        }
    }

    pub fn widget(&self) -> &Box {
        &self.container
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    pub fn is_loaded(&self) -> bool {
        self.is_loaded.get()
    }

    pub fn set_loaded(&self, loaded: bool) {
        if self.is_cloud || self.is_loaded.get() == loaded {
            return;
        }
        self.is_loaded.set(loaded);

        // 1. Update icon styling
        if loaded {
            self.icon.add_css_class("loaded-model-icon");
        } else {
            self.icon.remove_css_class("loaded-model-icon");
        }

        // 2. Update subtitle tags (toggle Active in VRAM badge)
        let mut info_tags = Vec::new();
        if loaded {
            info_tags.push(t!("model_card.active_vram"));
        }
        info_tags.extend(self.static_tags.clone());
        self.subtitle_label.set_label(&info_tags.join("  •  "));

        // 3. Update action button label, tooltip, and styling
        if loaded {
            self.btn_action.set_label(&t!("model_card.btn_unload"));
            self.btn_action.set_tooltip_text(Some(&t!("model_card.btn_unload_tooltip")));
            self.btn_action.set_css_classes(&["destructive-action"]);
        } else {
            self.btn_action.set_label(&t!("model_card.btn_load"));
            self.btn_action.set_tooltip_text(Some(&t!("model_card.btn_load_tooltip")));
            self.btn_action.set_css_classes(&["suggested-action"]);
        }
    }

    pub fn reset_delete_confirm(&self) {
        reset_delete_btn_state(&self.is_confirming_delete, &self.confirm_timeout, &self.btn_delete);
    }

    pub fn set_loading(&self, loading: bool) {
        self.reset_delete_confirm();
        if !self.is_cloud {
            self.btn_action.set_sensitive(!loading);
            self.btn_settings.set_sensitive(!loading);
        } else {
            self.btn_action.set_sensitive(false);
            self.btn_settings.set_sensitive(false);
        }
        self.btn_chat.set_sensitive(!loading);
        self.btn_delete.set_sensitive(!loading);
        self.progress_bar.set_visible(loading);

        if loading {
            if self.pulse_source.borrow().is_none() {
                let pbar = self.progress_bar.clone();
                let source_id = glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                    pbar.pulse();
                    glib::ControlFlow::Continue
                });
                *self.pulse_source.borrow_mut() = Some(source_id);
            }
        } else if let Some(source_id) = self.pulse_source.borrow_mut().take() {
            source_id.remove();
        }
    }

    pub fn connect_action<F: Fn(&str, bool) + 'static>(&self, f: F) {
        if self.is_cloud {
            return;
        }
        let name = self.model_name.clone();
        let loaded_cell = self.is_loaded.clone();
        let is_confirming = self.is_confirming_delete.clone();
        let confirm_timeout = self.confirm_timeout.clone();
        let btn_delete = self.btn_delete.clone();

        self.btn_action.connect_clicked(move |_| {
            reset_delete_btn_state(&is_confirming, &confirm_timeout, &btn_delete);
            f(&name, loaded_cell.get());
        });
    }

    pub fn connect_chat<F: Fn(&str) + 'static>(&self, f: F) {
        let name = self.model_name.clone();
        let is_confirming = self.is_confirming_delete.clone();
        let confirm_timeout = self.confirm_timeout.clone();
        let btn_delete = self.btn_delete.clone();

        self.btn_chat.connect_clicked(move |_| {
            reset_delete_btn_state(&is_confirming, &confirm_timeout, &btn_delete);
            f(&name);
        });
    }

    pub fn connect_settings<F: Fn(&str, u64) + 'static>(&self, f: F) {
        if self.is_cloud {
            return;
        }
        let name = self.model_name.clone();
        let size = self.model_size;
        let is_confirming = self.is_confirming_delete.clone();
        let confirm_timeout = self.confirm_timeout.clone();
        let btn_delete = self.btn_delete.clone();

        self.btn_settings.connect_clicked(move |_| {
            reset_delete_btn_state(&is_confirming, &confirm_timeout, &btn_delete);
            f(&name, size);
        });
    }

    pub fn connect_delete<F: Fn(&str) + 'static>(&self, f: F) {
        let name = self.model_name.clone();
        let is_confirming = self.is_confirming_delete.clone();
        let confirm_timeout = self.confirm_timeout.clone();

        self.btn_delete.connect_clicked(move |btn| {
            if is_confirming.get() {
                if let Some(source_id) = confirm_timeout.borrow_mut().take() {
                    source_id.remove();
                }
                is_confirming.set(false);

                btn.set_sensitive(false);
                btn.set_child(None::<&gtk4::Widget>);
                btn.set_icon_name("user-trash-symbolic");
                btn.set_css_classes(&["destructive-action", "flat", "circular"]);
                btn.set_tooltip_text(Some(&t!("model_card.btn_delete_tooltip")));

                f(&name);
            } else {
                is_confirming.set(true);

                let confirm_box = Box::new(Orientation::Vertical, 0);
                confirm_box.set_valign(gtk4::Align::Center);
                confirm_box.set_halign(gtk4::Align::Center);

                let lbl_top = gtk4::Label::builder()
                    .label(t!("model_card.btn_delete_confirm_title"))
                    .css_classes(["model-delete-confirm-top"])
                    .build();

                let lbl_bottom = gtk4::Label::builder()
                    .label(t!("model_card.btn_delete_confirm_action"))
                    .css_classes(["model-delete-confirm-bottom"])
                    .build();

                confirm_box.append(&lbl_top);
                confirm_box.append(&lbl_bottom);

                btn.set_child(Some(&confirm_box));
                btn.set_css_classes(&["destructive-action", "flat", "model-delete-confirm-btn"]);
                btn.set_tooltip_text(Some(&t!("model_card.btn_delete_confirm_tooltip")));

                if let Some(old_source) = confirm_timeout.borrow_mut().take() {
                    old_source.remove();
                }

                let is_conf_c = is_confirming.clone();
                let conf_timeout_c = confirm_timeout.clone();
                let btn_c = btn.clone();

                let source_id = glib::timeout_add_local_once(
                    std::time::Duration::from_secs(4),
                    move || {
                        *conf_timeout_c.borrow_mut() = None;
                        if is_conf_c.get() {
                            is_conf_c.set(false);
                            btn_c.set_child(None::<&gtk4::Widget>);
                            btn_c.set_icon_name("user-trash-symbolic");
                            btn_c.set_css_classes(&["destructive-action", "flat", "circular"]);
                            btn_c.set_tooltip_text(Some(&t!("model_card.btn_delete_tooltip")));
                        }
                    },
                );
                *confirm_timeout.borrow_mut() = Some(source_id);
            }
        });
    }

    pub fn resolve_installed_model_icon(model: &ModelTag) -> String {
        let is_cloud = model.is_cloud();
        let raw_id = model.name.split(':').next().unwrap_or(&model.name);
        let namespace = if raw_id.contains('/') {
            raw_id.split('/').next().unwrap_or("").to_lowercase().replace(' ', "-")
        } else {
            String::new()
        };
        let model_id = raw_id.split('/').next_back().unwrap_or(raw_id).to_lowercase().replace(' ', "-");

        let mut candidates = Vec::new();

        // 1. Hugging Face downloads systematically use Hugging Face logo
        let is_hf = model.name.starts_with("hf.co/")
            || model.name.starts_with("huggingface.co/")
            || namespace == "hf.co"
            || namespace == "hf"
            || namespace == "huggingface";

        if is_hf {
            candidates.push("huggingface-color".to_string());
            candidates.push("huggingface".to_string());
            candidates.push("default-color".to_string());
            candidates.push("default".to_string());
        } else {
            // 2. Official models: match exact ID in curated catalog
            let cached_catalog = crate::core::hub::HubManager::load_catalog();
            if let Some(hub_info) = cached_catalog.iter().find(|m| m.id.to_lowercase() == model_id) {
                let creator_str = hub_info.creator.as_deref().or(hub_info.publisher.as_deref());
                let creator_slugs: Vec<String> = if let Some(c) = creator_str {
                    let clean = c.trim().to_lowercase().replace(' ', "-");
                    let compact = clean.replace('-', "");
                    let first_word = clean.split('-').next().unwrap_or("").to_string();

                    let mut list = vec![clean.clone()];
                    if compact != clean && !compact.is_empty() {
                        list.push(compact);
                    }
                    if !first_word.is_empty() && first_word != clean {
                        list.push(first_word);
                    }
                    list
                } else {
                    Vec::new()
                };

                let family_slug = hub_info.family.as_deref().map(|f| f.trim().to_lowercase().replace(' ', "-"));

                if let Some(ref f) = family_slug {
                    if !f.is_empty() {
                        for c in &creator_slugs {
                            let combined = format!("{}-{}", c, f);
                            candidates.push(format!("{}-color", combined));
                            candidates.push(combined);
                        }
                    }
                }

                for c in &creator_slugs {
                    candidates.push(format!("{}-color", c));
                    candidates.push(c.clone());
                }
            }

            // 3. Direct model_id (if a specific icon exists for this exact model)
            candidates.push(format!("{}-color", model_id));
            candidates.push(model_id);

            // 4. Cloud fallback
            if is_cloud {
                candidates.push("weather-overcast-symbolic".to_string());
            }

            // 5. Default fallback (to avoid any false brand attribution)
            candidates.push("default-color".to_string());
            candidates.push("default".to_string());
            candidates.push("network-server-symbolic".to_string());
        }

        if let Some(display) = gtk4::gdk::Display::default() {
            let theme = gtk4::IconTheme::for_display(&display);
            candidates
                .into_iter()
                .find(|c| theme.has_icon(c))
                .unwrap_or_else(|| {
                    if is_cloud {
                        "weather-overcast-symbolic".to_string()
                    } else {
                        "network-server-symbolic".to_string()
                    }
                })
        } else {
            "default".to_string()
        }
    }
}

impl Drop for ModelCard {
    fn drop(&mut self) {
        if let Some(source_id) = self.pulse_source.borrow_mut().take() {
            source_id.remove();
        }
        if let Some(source_id) = self.confirm_timeout.borrow_mut().take() {
            source_id.remove();
        }
    }
}

