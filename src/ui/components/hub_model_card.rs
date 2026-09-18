use crate::api::types::ModelNameUtils;
use crate::core::capabilities::detect_icons_for_hub_model;
use crate::core::hardware::estimator::format_bytes;
use crate::core::hardware::HardwareSnapshot;
use crate::core::hub::{HardwareFitness, HubManager, HubModelInfo, HubModelVariant};
use crate::t;
use adw::prelude::*;
use gtk4::{
    Align, Box, Button, DropDown, Label, ListItem, Orientation, ProgressBar,
    SignalListItemFactory, StringList, StringObject,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

type ActiveDownloadMap = HashMap<String, (String, Option<f64>)>;

pub struct HubModelCard {
    container: Box,
    btn_download: Button,
    progress_bar: ProgressBar,
    progress_label: Label,
    fitness_badge: Label,
    model_info: HubModelInfo,
    variant_dropdown: DropDown,
    selected_variant_idx: Rc<RefCell<usize>>,
    installed_variants: Rc<RefCell<HashSet<String>>>,
    active_downloads: Rc<RefCell<ActiveDownloadMap>>,
    snap_rc: Rc<RefCell<HardwareSnapshot>>,
}

impl HubModelCard {
    pub fn new(mut model_info: HubModelInfo, snap: &HardwareSnapshot, installed_set: &HashSet<String>) -> Self {
        let container = Box::new(Orientation::Vertical, 0);
        container.set_css_classes(&["card"]);
        container.set_margin_bottom(8);

        let main_box = Box::new(Orientation::Vertical, 6);
        main_box.set_margin_top(12);
        main_box.set_margin_bottom(12);
        main_box.set_margin_start(16);
        main_box.set_margin_end(16);

        // Header: Model Logo + Title + Creator + Capabilities + Cloud + Fitness badge
        let header_row = Box::new(Orientation::Horizontal, 8);
        header_row.set_valign(gtk4::Align::Center);

        // Model Icon resolution (Prioritize $CREATOR-$FAMILY-color -> $CREATOR-$FAMILY -> $CREATOR-color -> $CREATOR -> default-color -> default)
        let icon_name_resolved = {
            let mut candidates = Vec::new();

            let creator_str = model_info.creator.as_deref().or(model_info.publisher.as_deref());
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
            } else if !model_info.namespace.is_empty() && model_info.namespace != "library" {
                vec![model_info.namespace.trim().to_lowercase().replace(' ', "-")]
            } else {
                Vec::new()
            };

            let family_slug = model_info.family.as_deref().map(|f| f.trim().to_lowercase().replace(' ', "-"));

            // 1. Dynamic $CREATOR-$FAMILY candidates
            if let Some(ref f) = family_slug {
                if !f.is_empty() {
                    for c in &creator_slugs {
                        let combined = format!("{}-{}", c, f);
                        candidates.push(format!("{}-color", combined));
                        candidates.push(combined);
                    }
                }
            }

            // 2. Fallback / explicit icon_name (if specified and non-default)
            let target = model_info.icon_name.trim();
            if !target.is_empty() && target != "network-server-symbolic" && target != "default" {
                if !target.ends_with("-color") {
                    candidates.push(format!("{}-color", target));
                }
                candidates.push(target.to_string());
            }

            // 3. Creator logo fallback
            for c in &creator_slugs {
                candidates.push(format!("{}-color", c));
                candidates.push(c.clone());
            }

            // 4. Default fallbacks
            candidates.push("default-color".to_string());
            candidates.push("default".to_string());
            candidates.push("network-server-symbolic".to_string());

            if let Some(display) = gtk4::gdk::Display::default() {
                let theme = gtk4::IconTheme::for_display(&display);
                candidates
                    .into_iter()
                    .find(|c| theme.has_icon(c))
                    .unwrap_or_else(|| "network-server-symbolic".to_string())
            } else {
                candidates.into_iter().next().unwrap_or_else(|| "default".to_string())
            }
        };

        let model_icon = gtk4::Image::from_icon_name(&icon_name_resolved);
        model_icon.set_pixel_size(24);
        model_icon.set_valign(gtk4::Align::Center);
        header_row.append(&model_icon);

        let title_btn = Button::builder()
            .label(&model_info.name)
            .css_classes(["flat", "heading"])
            .halign(gtk4::Align::Start)
            .tooltip_text(t!("hub.open_page_tooltip"))
            .build();

        let url_clone = model_info.page_url.clone();
        title_btn.connect_clicked(move |_| {
            if !url_clone.is_empty() {
                let _ = gtk4::gio::AppInfo::launch_default_for_uri(&url_clone, None::<&gtk4::gio::AppLaunchContext>);
            }
        });
        header_row.append(&title_btn);

        // Creator / Publisher badge with logo
        if let Some(creator) = model_info.creator.as_deref().or(model_info.publisher.as_deref()) {
            if !creator.is_empty() {
                let creator_box = Box::new(Orientation::Horizontal, 4);
                creator_box.set_valign(gtk4::Align::Center);

                // Resolve creator icon (prioritize color, then monochrome)
                let creator_icon_name = {
                    let clean = creator.trim().to_lowercase().replace(' ', "-");
                    let compact = clean.replace('-', "");
                    let first_word = clean.split('-').next().unwrap_or("").to_string();

                    let mut cand = vec![
                        format!("{}-color", clean),
                        clean.clone(),
                    ];
                    if compact != clean && !compact.is_empty() {
                        cand.push(format!("{}-color", compact));
                        cand.push(compact);
                    }
                    if !first_word.is_empty() && first_word != clean {
                        cand.push(format!("{}-color", first_word));
                        cand.push(first_word);
                    }

                    if let Some(display) = gtk4::gdk::Display::default() {
                        let theme = gtk4::IconTheme::for_display(&display);
                        cand.into_iter().find(|c| theme.has_icon(c))
                    } else {
                        None
                    }
                };

                if let Some(ref c_icon) = creator_icon_name {
                    let c_img = gtk4::Image::from_icon_name(c_icon);
                    c_img.set_pixel_size(14);
                    c_img.set_valign(gtk4::Align::Center);
                    creator_box.append(&c_img);
                }

                let creator_badge = Label::builder()
                    .label(creator)
                    .css_classes(["caption", "dim-label"])
                    .valign(gtk4::Align::Center)
                    .build();
                creator_box.append(&creator_badge);

                header_row.append(&creator_box);
            }
        }

        // Capabilities icons
        let caps = crate::core::capabilities::detect_for_hub_model(&model_info, "");
        if !caps.is_empty() {
            let icons_str = caps.iter().map(|c| c.emoji()).collect::<Vec<_>>().join(" ");
            let labels_str = caps.iter().map(|c| c.label()).collect::<Vec<_>>().join(" • ");
            let caps_badge = Label::builder()
                .label(&icons_str)
                .tooltip_text(&labels_str)
                .css_classes(["caption"])
                .valign(gtk4::Align::Center)
                .build();
            header_row.append(&caps_badge);
        }

        let is_model_cloud = model_info.is_cloud
            || model_info.badges.iter().any(|b| b == "cloud")
            || HubManager::is_cloud_model(
                &model_info.id,
                &model_info.name,
                &model_info.description,
                &model_info.category,
                &model_info.variants,
            );

        if is_model_cloud {
            let has_local_variants = model_info.variants.iter().any(|v| {
                !v.is_cloud
                    && !v.tag.contains("cloud")
                    && v.size_bytes > 0
                    && !v.parameter_size.trim().is_empty()
            });

            if has_local_variants {
                let cloud_tag = format!("{}:cloud", model_info.id);
                if !model_info.variants.iter().any(|v| v.tag == cloud_tag || v.tag.ends_with(":cloud") || v.tag.ends_with("-cloud")) {
                    let mut v_cloud = HubModelVariant::new(&cloud_tag, 0, "Cloud", "API", 128_000);
                    v_cloud.is_cloud = true;
                    v_cloud.size_formatted = "Cloud".to_string();
                    model_info.variants.push(v_cloud);
                }
            } else {
                for v in &mut model_info.variants {
                    v.is_cloud = true;
                    v.size_bytes = 0;
                    v.quantization = "Cloud".to_string();
                    v.size_formatted = "Cloud".to_string();
                }
            }
        }

        // Cloud badge if cloud model
        if is_model_cloud {
            let cloud_badge = Label::builder()
                .label("☁️ Cloud")
                .tooltip_text(t!("model_card.btn_cloud_tooltip"))
                .css_classes(["caption", "accent"])
                .valign(gtk4::Align::Center)
                .build();
            header_row.append(&cloud_badge);
        }

        let spacer = Box::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        header_row.append(&spacer);

        // Fitness badge
        let fitness_badge = Label::builder()
            .css_classes(["badge"])
            .valign(gtk4::Align::Center)
            .build();
        header_row.append(&fitness_badge);

        main_box.append(&header_row);

        // Description
        let desc_label = Label::builder()
            .label(&model_info.description)
            .css_classes(["body", "dim-label"])
            .wrap(true)
            .halign(gtk4::Align::Start)
            .build();
        main_box.append(&desc_label);

        // Controls row
        let controls_row = Box::new(Orientation::Horizontal, 8);
        controls_row.set_valign(gtk4::Align::Center);
        controls_row.set_margin_top(4);

        let button_factory = SignalListItemFactory::new();
        button_factory.connect_setup(|_, list_item| {
            let hbox = Box::new(Orientation::Horizontal, 8);
            hbox.set_hexpand(true);

            let label_left = Label::builder()
                .xalign(0.0)
                .halign(Align::Start)
                .hexpand(true)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .build();

            let label_right = Label::builder()
                .xalign(1.0)
                .halign(Align::End)
                .hexpand(false)
                .css_classes(["caption", "dim-label"])
                .build();

            hbox.append(&label_left);
            hbox.append(&label_right);
            if let Some(item) = list_item.downcast_ref::<ListItem>() {
                item.set_child(Some(&hbox));
            }
        });
        button_factory.connect_bind(|_, list_item| {
            if let Some(list_item) = list_item.downcast_ref::<ListItem>() {
                if let Some(str_obj) = list_item.item().and_then(|item| item.downcast::<StringObject>().ok()) {
                    if let Some(hbox) = list_item.child().and_then(|c| c.downcast::<Box>().ok()) {
                        let text = str_obj.string();
                        let (left, right) = text.split_once('\t').unwrap_or((&text, ""));
                        if let Some(lbl_left) = hbox.first_child().and_then(|c| c.downcast::<Label>().ok()) {
                            lbl_left.set_label(left);
                        }
                        if let Some(lbl_right) = hbox.last_child().and_then(|c| c.downcast::<Label>().ok()) {
                            lbl_right.set_label(right);
                        }
                    }
                }
            }
        });

        let list_factory = SignalListItemFactory::new();
        list_factory.connect_setup(|_, list_item| {
            let hbox = Box::new(Orientation::Horizontal, 12);
            hbox.set_hexpand(true);
            hbox.set_margin_top(3);
            hbox.set_margin_bottom(3);
            hbox.set_margin_start(4);
            hbox.set_margin_end(4);

            let label_left = Label::builder()
                .xalign(0.0)
                .halign(Align::Start)
                .hexpand(true)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .build();

            let label_right = Label::builder()
                .xalign(1.0)
                .halign(Align::End)
                .hexpand(false)
                .css_classes(["caption", "dim-label"])
                .build();

            hbox.append(&label_left);
            hbox.append(&label_right);
            if let Some(item) = list_item.downcast_ref::<ListItem>() {
                item.set_child(Some(&hbox));
            }
        });
        list_factory.connect_bind(|_, list_item| {
            if let Some(list_item) = list_item.downcast_ref::<ListItem>() {
                if let Some(str_obj) = list_item.item().and_then(|item| item.downcast::<StringObject>().ok()) {
                    if let Some(hbox) = list_item.child().and_then(|c| c.downcast::<Box>().ok()) {
                        let text = str_obj.string();
                        let (left, right) = text.split_once('\t').unwrap_or((&text, ""));
                        if let Some(lbl_left) = hbox.first_child().and_then(|c| c.downcast::<Label>().ok()) {
                            lbl_left.set_label(left);
                        }
                        if let Some(lbl_right) = hbox.last_child().and_then(|c| c.downcast::<Label>().ok()) {
                            lbl_right.set_label(right);
                        }
                    }
                }
            }
        });

        let variant_dropdown = DropDown::builder()
            .factory(&button_factory)
            .list_factory(&list_factory)
            .valign(gtk4::Align::Center)
            .hexpand(true)
            .focus_on_click(false)
            .can_focus(false)
            .build();
        controls_row.append(&variant_dropdown);

        let btn_download = Button::builder().build();
        controls_row.append(&btn_download);

        main_box.append(&controls_row);

        let progress_label = Label::builder()
            .visible(false)
            .halign(gtk4::Align::Start)
            .css_classes(["caption", "dim-label"])
            .build();
        main_box.append(&progress_label);

        let progress_bar = ProgressBar::builder()
            .visible(false)
            .height_request(4)
            .css_classes(["accent"])
            .build();

        container.append(&main_box);
        container.append(&progress_bar);

        let selected_idx_rc = Rc::new(RefCell::new(0));
        let installed_variants = Rc::new(RefCell::new(installed_set.clone()));

        let card = Self {
            container,
            btn_download,
            progress_bar,
            progress_label,
            fitness_badge,
            model_info,
            variant_dropdown,
            selected_variant_idx: selected_idx_rc,
            installed_variants,
            active_downloads: Rc::new(RefCell::new(HashMap::new())),
            snap_rc: Rc::new(RefCell::new(snap.clone())),
        };

        card.update_dropdown_items();
        card.update_ui_for_current_variant();

        // Connect variant change
        let idx_rc = card.selected_variant_idx.clone();
        let card_dropdown = card.variant_dropdown.clone();
        let fit_badge = card.fitness_badge.clone();
        let btn_dl = card.btn_download.clone();
        let p_bar = card.progress_bar.clone();
        let p_lbl = card.progress_label.clone();
        let active_dl = card.active_downloads.clone();
        let inst_set = card.installed_variants.clone();
        let snap_ref = card.snap_rc.clone();
        let model_variants = card.model_info.variants.clone();

        card_dropdown.connect_selected_notify(move |dd| {
            let idx = dd.selected() as usize;
            *idx_rc.borrow_mut() = idx;
            if let Some(variant) = model_variants.get(idx) {
                let snap_curr = snap_ref.borrow().clone();
                let is_cloud_var = variant.is_cloud
                    || variant.tag.ends_with(":cloud")
                    || variant.tag.ends_with("-cloud")
                    || variant.tag.contains("cloud")
                    || variant.size_bytes == 0
                    || variant.quantization.eq_ignore_ascii_case("cloud");
                let tooltip_text = if is_cloud_var {
                    fit_badge.set_label("☁️ Cloud");
                    fit_badge.set_tooltip_text(Some(&t!("model_card.btn_cloud_tooltip")));
                    t!("model_card.btn_cloud_tooltip")
                } else {
                    let fitness = HubManager::evaluate_fitness(variant.size_bytes, &snap_curr);
                    fit_badge.set_label(&fitness.badge_label());
                    fit_badge.set_tooltip_text(Some(&fitness.tooltip()));
                    fitness.tooltip()
                };
                dd.set_tooltip_text(Some(&tooltip_text));

                let active_map = active_dl.borrow();
                let dl_entry = active_map.get(&variant.tag)
                    .or_else(|| active_map.iter().find(|(k, _)| k.starts_with(&variant.tag) || variant.tag.starts_with(*k)).map(|(_, v)| v));

                if let Some((status, frac_opt)) = dl_entry {
                    btn_dl.set_label(&t!("hub.cancel_btn"));
                    btn_dl.set_css_classes(&["flat", "destructive-action"]);
                    btn_dl.set_sensitive(true);
                    btn_dl.set_tooltip_text(Some(&t!("hub.cancel_tooltip")));
                    p_lbl.set_label(status);
                    p_lbl.set_visible(true);
                    p_bar.set_visible(true);
                    if let Some(frac) = frac_opt {
                        p_bar.set_fraction(*frac);
                    } else {
                        p_bar.pulse();
                    }
                } else if ModelNameUtils::is_tag_installed(inst_set.borrow().iter().map(|s| s.as_str()), &variant.tag) {
                    btn_dl.set_label(&t!("hub.already_installed"));
                    btn_dl.set_css_classes(&["flat"]);
                    btn_dl.set_sensitive(false);
                    btn_dl.set_tooltip_text(Some(&t!("hub.already_installed_tooltip")));
                    p_lbl.set_visible(false);
                    p_bar.set_visible(false);
                } else {
                    btn_dl.set_label(&t!("hub.download_btn"));
                    btn_dl.set_css_classes(&["suggested-action"]);
                    btn_dl.set_sensitive(true);
                    btn_dl.set_tooltip_text(Some(&t!("hub.custom_pull_tooltip")));
                    p_lbl.set_visible(false);
                    p_bar.set_visible(false);
                }
            }
        });

        card
    }

    pub fn widget(&self) -> &Box {
        &self.container
    }

    pub fn selected_variant(&self) -> Option<HubModelVariant> {
        let idx = *self.selected_variant_idx.borrow();
        self.model_info.variants.get(idx).cloned()
    }

    fn update_dropdown_items(&self) {
        let snap = self.snap_rc.borrow().clone();
        let installed = self.installed_variants.borrow();
        let active = self.active_downloads.borrow();

        let variant_labels: Vec<String> = self.model_info
            .variants
            .iter()
            .map(|v| {
                let is_dl = active.contains_key(&v.tag)
                    || active.iter().any(|(k, _)| k.starts_with(&v.tag) || v.tag.starts_with(k));
                let is_inst = ModelNameUtils::is_tag_installed(installed.iter().map(|s| s.as_str()), &v.tag);
                let is_cloud_var = self.is_variant_cloud(v);

                let prefix_circle = if is_cloud_var {
                    "☁️"
                } else {
                    let fitness = HubManager::evaluate_fitness(v.size_bytes, &snap);
                    match fitness {
                        HardwareFitness::Insufficient => "🔴",
                        HardwareFitness::GpuOffload => "🟡",
                        HardwareFitness::FullGpu => "🟢",
                    }
                };

                let params_quant = if is_cloud_var {
                    if v.parameter_size.trim().is_empty() {
                        "(Cloud)".to_string()
                    } else {
                        format!("({} · Cloud)", v.parameter_size)
                    }
                } else {
                    format!("({} · {})", v.parameter_size, v.quantization)
                };

                let status_suffix = if is_dl {
                    " ⬇️"
                } else if is_inst {
                    " ✓"
                } else {
                    ""
                };

                let caps = detect_icons_for_hub_model(&self.model_info, &v.tag);

                let left_part = format!(
                    "{} {} {}{}{}",
                    prefix_circle, v.tag, params_quant, status_suffix, caps
                );

                let right_part = if is_cloud_var {
                    "☁️ Cloud".to_string()
                } else {
                    format!("📦 {}", format_bytes(v.size_bytes))
                };

                format!("{}\t{}", left_part, right_part)
            })
            .collect();

        let current_selected = *self.selected_variant_idx.borrow();
        let variant_refs: Vec<&str> = variant_labels.iter().map(|s| s.as_str()).collect();
        let variant_strings = StringList::new(&variant_refs);
        self.variant_dropdown.set_model(Some(&variant_strings));
        if current_selected < self.model_info.variants.len() {
            self.variant_dropdown.set_selected(current_selected as u32);
        }
    }

    fn update_ui_for_current_variant(&self) {
        let idx = *self.selected_variant_idx.borrow();
        if let Some(variant) = self.model_info.variants.get(idx) {
            let snap = self.snap_rc.borrow().clone();
            let is_cloud_var = self.is_variant_cloud(variant);
            let tooltip_text = if is_cloud_var {
                self.fitness_badge.set_label("☁️ Cloud");
                self.fitness_badge.set_tooltip_text(Some(&t!("model_card.btn_cloud_tooltip")));
                t!("model_card.btn_cloud_tooltip")
            } else {
                let fitness = HubManager::evaluate_fitness(variant.size_bytes, &snap);
                self.fitness_badge.set_label(&fitness.badge_label());
                self.fitness_badge.set_tooltip_text(Some(&fitness.tooltip()));
                fitness.tooltip()
            };
            self.variant_dropdown.set_tooltip_text(Some(&tooltip_text));

            let active = self.active_downloads.borrow();
            let dl_entry = active.get(&variant.tag)
                .or_else(|| active.iter().find(|(k, _)| k.starts_with(&variant.tag) || variant.tag.starts_with(*k)).map(|(_, v)| v));

            if let Some((status, frac_opt)) = dl_entry {
                self.btn_download.set_label(&t!("hub.cancel_btn"));
                self.btn_download.set_css_classes(&["flat", "destructive-action"]);
                self.btn_download.set_sensitive(true);
                self.btn_download.set_tooltip_text(Some(&t!("hub.cancel_tooltip")));
                self.progress_label.set_label(status);
                self.progress_label.set_visible(true);
                self.progress_bar.set_visible(true);
                if let Some(frac) = frac_opt {
                    self.progress_bar.set_fraction(*frac);
                } else {
                    self.progress_bar.pulse();
                }
            } else if ModelNameUtils::is_tag_installed(self.installed_variants.borrow().iter().map(|s| s.as_str()), &variant.tag) {
                self.btn_download.set_label(&t!("hub.already_installed"));
                self.btn_download.set_css_classes(&["flat"]);
                self.btn_download.set_sensitive(false);
                self.btn_download.set_tooltip_text(Some(&t!("hub.already_installed_tooltip")));
                self.progress_label.set_visible(false);
                self.progress_bar.set_visible(false);
            } else {
                self.btn_download.set_label(&t!("hub.download_btn"));
                self.btn_download.set_css_classes(&["suggested-action"]);
                self.btn_download.set_sensitive(true);
                self.btn_download.set_tooltip_text(Some(&t!("hub.custom_pull_tooltip")));
                self.progress_label.set_visible(false);
                self.progress_bar.set_visible(false);
            }
        }
    }

    pub fn set_variant_download_progress(&self, tag: &str, downloading: bool, status_text: &str, fraction_opt: Option<f64>) {
        let was_downloading = self.active_downloads.borrow().contains_key(tag);
        if downloading {
            self.active_downloads.borrow_mut().insert(tag.to_string(), (status_text.to_string(), fraction_opt));
        } else {
            self.active_downloads.borrow_mut().remove(tag);
        }
        if was_downloading != downloading {
            self.update_dropdown_items();
        }
        self.update_ui_for_current_variant();
    }

    pub fn connect_download<F: Fn(String, bool) + 'static>(&self, f: F) {
        let variants = self.model_info.variants.clone();
        let idx_rc = self.selected_variant_idx.clone();
        let active_dl_rc = self.active_downloads.clone();
        let installed_rc = self.installed_variants.clone();

        self.btn_download.connect_clicked(move |_| {
            let idx = *idx_rc.borrow();
            if let Some(variant) = variants.get(idx) {
                let is_dl = active_dl_rc.borrow().contains_key(&variant.tag)
                    || active_dl_rc.borrow().iter().any(|(k, _)| k.starts_with(&variant.tag) || variant.tag.starts_with(k));
                let is_inst = ModelNameUtils::is_tag_installed(installed_rc.borrow().iter().map(|s| s.as_str()), &variant.tag);

                if is_dl {
                    f(variant.tag.clone(), true);
                } else if !is_inst {
                    f(variant.tag.clone(), false);
                }
            }
        });
    }

    pub fn update_installed_set(&self, installed_set: &HashSet<String>, snap: &HardwareSnapshot) {
        let changed = *self.installed_variants.borrow() != *installed_set;
        *self.installed_variants.borrow_mut() = installed_set.clone();
        *self.snap_rc.borrow_mut() = snap.clone();
        if changed {
            self.update_dropdown_items();
        }
        self.update_ui_for_current_variant();
    }

    pub fn model_info(&self) -> &HubModelInfo {
        &self.model_info
    }

    pub fn current_fitness(&self, snap: &HardwareSnapshot) -> crate::core::hub::HardwareFitness {
        let idx = *self.selected_variant_idx.borrow();
        if let Some(variant) = self.model_info.variants.get(idx) {
            if self.is_variant_cloud(variant) {
                crate::core::hub::HardwareFitness::FullGpu
            } else {
                HubManager::evaluate_fitness(variant.size_bytes, snap)
            }
        } else {
            crate::core::hub::HardwareFitness::Insufficient
        }
    }

    pub fn is_cloud(&self) -> bool {
        self.model_info.is_cloud
            || self.model_info.badges.iter().any(|b| b == "cloud")
            || HubManager::is_cloud_model(
                &self.model_info.id,
                &self.model_info.name,
                &self.model_info.description,
                &self.model_info.category,
                &self.model_info.variants,
            )
    }

    pub fn is_variant_cloud(&self, variant: &HubModelVariant) -> bool {
        variant.is_cloud
            || variant.tag.ends_with(":cloud")
            || variant.tag.ends_with("-cloud")
            || variant.tag.contains("cloud")
            || variant.size_bytes == 0
            || variant.quantization.eq_ignore_ascii_case("cloud")
    }

    pub fn select_cloud_variant_if_available(&self) {
        if let Some(pos) = self.model_info.variants.iter().position(|v| self.is_variant_cloud(v)) {
            self.variant_dropdown.set_selected(pos as u32);
            *self.selected_variant_idx.borrow_mut() = pos;
            self.update_ui_for_current_variant();
        }
    }

    pub fn matches_filter(
        &self,
        cat_filter: Option<&str>,
        query: &str,
        compat_only: bool,
        snap: &HardwareSnapshot,
    ) -> bool {
        let matches_cat = match cat_filter {
            Some(cat) => {
                let info = &self.model_info;
                let c_low = info.category.to_lowercase();
                let id_low = info.id.to_lowercase();
                let badges = &info.badges;
                let flags = info.capabilities.as_ref();

                match cat {
                    "Cloud" | "cloud" => self.is_cloud() || badges.iter().any(|b| b == "cloud"),
                    "reasoning" | "Raisonnement" => {
                        flags.map(|c| c.think).unwrap_or(false)
                            || badges.iter().any(|b| b == "thinking" || b == "reasoning")
                            || c_low.contains("reason")
                    }
                    "code" | "Code" => {
                        flags.map(|c| c.code).unwrap_or(false)
                            || badges.iter().any(|b| b == "code" || b == "coding")
                            || c_low.contains("code")
                    }
                    "vision" | "Vision" => {
                        flags.map(|c| c.vision).unwrap_or(false)
                            || badges.iter().any(|b| b == "vision" || b == "multimodal")
                            || c_low.contains("vision")
                    }
                    "audio" | "Audio" => {
                        flags.map(|c| c.audio).unwrap_or(false)
                            || badges.iter().any(|b| b == "audio" || b == "voice" || b == "speech")
                            || c_low.contains("audio")
                    }
                    "tools" | "Tools" | "outils" | "Outils" => {
                        flags.map(|c| c.tools).unwrap_or(false)
                            || badges.iter().any(|b| b == "tools")
                            || c_low.contains("tool")
                    }
                    "rag" | "embed" | "Embeddings" => {
                        flags.map(|c| c.embedding).unwrap_or(false)
                            || badges.iter().any(|b| b == "embedding" || b == "embed")
                            || c_low.contains("embed")
                    }
                    "lightweight" | "Léger" => {
                        let has_light_variant = info.variants.iter().any(|v| {
                            (v.size_bytes > 0 && v.size_bytes <= 3_600_000_000)
                                || v.parameter_size.starts_with("0.")
                                || v.parameter_size.starts_with('1')
                                || v.parameter_size.starts_with('2')
                                || v.parameter_size.starts_with('3')
                        });
                        let has_light_badge = badges.iter().any(|b| {
                            b.ends_with('b')
                                && (b.starts_with("0.")
                                    || b.starts_with('1')
                                    || b.starts_with('2')
                                    || b.starts_with('3')
                                    || b.starts_with("3.8")
                                    || b.starts_with('4'))
                        });
                        let is_light_named = id_low.contains("mini")
                            || id_low.contains("small")
                            || id_low.contains("tiny")
                            || id_low.contains("nano")
                            || id_low.contains("smollm")
                            || id_low.contains("pico")
                            || id_low.contains("1b")
                            || id_low.contains("2b")
                            || id_low.contains("3b");

                        (has_light_variant || has_light_badge || is_light_named) && !self.is_cloud()
                    }
                    "cybersecurity" | "cyber" => {
                        c_low.contains("cyber") || c_low.contains("security")
                    }
                    _ => c_low.contains(&cat.to_lowercase()),
                }
            }
            None => true,
        };

        let matches_query = if query.is_empty() {
            true
        } else {
            let id_match = self.model_info.id.to_lowercase().contains(query);
            let name_match = self.model_info.name.to_lowercase().contains(query);
            let creator_match = self
                .model_info
                .creator
                .as_ref()
                .is_some_and(|c| c.to_lowercase().contains(query))
                || self
                    .model_info
                    .publisher
                    .as_ref()
                    .is_some_and(|p| p.to_lowercase().contains(query));
            let variant_match = self
                .selected_variant()
                .map(|v| v.tag.to_lowercase().contains(query))
                .unwrap_or(false);
            let desc_match = self.model_info.description.to_lowercase().contains(query);
            id_match || name_match || creator_match || variant_match || desc_match
        };

        let matches_compat = if compat_only {
            if cat_filter == Some("cloud") || cat_filter == Some("Cloud") {
                true
            } else {
                self.current_fitness(snap) != HardwareFitness::Insufficient
                    || self.model_info.variants.iter().any(|v| {
                        self.is_variant_cloud(v)
                            || HubManager::evaluate_fitness(v.size_bytes, snap) != HardwareFitness::Insufficient
                    })
            }
        } else {
            true
        };

        matches_cat && matches_query && matches_compat
    }
}
