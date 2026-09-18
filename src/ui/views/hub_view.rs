use crate::api::types::ModelTag;
use crate::core::hardware::HardwareSnapshot;
use crate::core::hub::{HubManager, HubModelInfo, SortCriterion};
use crate::t;
use crate::ui::components::HubModelCard;
use adw::prelude::*;
use glib;
use gtk4::{
    Align, Box, Button, DropDown, FlowBox, FlowBoxChild, Label, ListBox,
    Orientation, ScrolledWindow, SearchEntry, StringList, ToggleButton,
};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

type CatalogCardEntry = (String, String, Rc<HubModelCard>);

pub struct HubView {
    container: ScrolledWindow,
    search_entry: SearchEntry,
    btn_pull_action: Button,
    category_box: FlowBox,
    catalog_bar: Box,
    sort_dropdown: DropDown,
    btn_compat_filter: ToggleButton,
    catalog_list: ListBox,
    catalog_empty_banner: adw::StatusPage,
    btn_sync_online: Button,
    catalog_cards: Rc<RefCell<Vec<CatalogCardEntry>>>, // (category, model_id, card)
    raw_catalog: Rc<RefCell<Vec<HubModelInfo>>>,
    last_snap: Rc<RefCell<HardwareSnapshot>>,
    installed_names: Rc<RefCell<HashSet<String>>>,
    active_category_filter: Rc<RefCell<Option<String>>>,
    current_sort: Rc<RefCell<SortCriterion>>,
    compat_only_filter: Rc<RefCell<bool>>,
    catalog_loaded: Rc<RefCell<bool>>,
}

impl Default for HubView {
    fn default() -> Self {
        Self::new()
    }
}

impl HubView {
    pub fn new() -> Self {
        let container = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .build();

        let clamp = adw::Clamp::builder()
            .maximum_size(1100)
            .tightening_threshold(850)
            .margin_top(18)
            .margin_bottom(24)
            .margin_start(16)
            .margin_end(16)
            .build();

        let root_box = Box::new(Orientation::Vertical, 16);

        // ========================================================
        // SECTION: Model Catalog, Smart Omnibar & Filters
        // ========================================================
        let catalog_group = adw::PreferencesGroup::builder()
            .title(t!("hub.catalog_title"))
            .description(t!("hub.catalog_desc"))
            .build();

        let btn_sync_online = Button::builder()
            .label(t!("hub.sync_btn"))
            .css_classes(["flat"])
            .tooltip_text(t!("hub.sync_tooltip"))
            .valign(Align::Center)
            .build();
        catalog_group.set_header_suffix(Some(&btn_sync_online));

        // Unified Omnibar: SearchEntry + Contextual Action Button (Hugging Face / Ollama Pull)
        let search_hbox = Box::new(Orientation::Horizontal, 8);
        search_hbox.set_margin_bottom(8);

        let search_entry = SearchEntry::builder()
            .placeholder_text(t!("hub.search_placeholder"))
            .hexpand(true)
            .can_focus(true)
            .focus_on_click(true)
            .build();

        let btn_pull_action = Button::builder()
            .label(t!("hub.pull_btn"))
            .css_classes(["suggested-action", "pill"])
            .tooltip_text(t!("hub.pull_tooltip"))
            .valign(Align::Center)
            .visible(false)
            .build();

        search_hbox.append(&search_entry);
        search_hbox.append(&btn_pull_action);
        catalog_group.add(&search_hbox);

        // Category Filter Buttons (Pills)
        let category_box = FlowBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .max_children_per_line(10)
            .min_children_per_line(3)
            .column_spacing(6)
            .row_spacing(6)
            .margin_bottom(10)
            .css_classes(["category-filter-box"])
            .build();

        let categories = [
            (t!("hub.categories.all"), None),
            (t!("hub.categories.reasoning"), Some("reasoning")),
            (t!("hub.categories.vision"), Some("vision")),
            (t!("hub.categories.code"), Some("code")),
            (t!("hub.categories.lightweight"), Some("lightweight")),
            (t!("hub.categories.rag"), Some("rag")),
            (t!("hub.categories.cybersecurity"), Some("cybersecurity")),
            (t!("hub.categories.tools"), Some("tools")),
            (t!("hub.categories.cloud"), Some("cloud")),
        ];

        let active_cat_rc = Rc::new(RefCell::new(None::<String>));

        for (label, cat_id) in categories {
            let btn = ToggleButton::builder()
                .label(&label)
                .css_classes(["flat", "pill", "category-pill", "pill-button"])
                .build();

            if cat_id.is_none() {
                btn.set_active(true);
            }

            let child = FlowBoxChild::builder()
                .child(&btn)
                .can_focus(false)
                .build();
            category_box.append(&child);
        }

        catalog_group.add(&category_box);

        // Catalog Action Bar (Sort + Compatibility Filter)
        let catalog_bar = Box::new(Orientation::Horizontal, 10);
        catalog_bar.set_margin_bottom(8);

        let sort_label = Label::builder()
            .label(t!("hub.sort_label"))
            .css_classes(["caption", "dim-label"])
            .valign(Align::Center)
            .build();
        catalog_bar.append(&sort_label);

        let sort_labels = SortCriterion::all_labels();
        let sort_refs: Vec<&str> = sort_labels.iter().map(|s| s.as_str()).collect();
        let sort_options = StringList::new(&sort_refs);

        let sort_dropdown = DropDown::builder()
            .model(&sort_options)
            .selected(1)
            .valign(Align::Center)
            .build();
        catalog_bar.append(&sort_dropdown);

        let spacer_catalog = Box::new(Orientation::Horizontal, 0);
        spacer_catalog.set_hexpand(true);
        catalog_bar.append(&spacer_catalog);

        let btn_compat_filter = ToggleButton::builder()
            .label(t!("hub.compat_only"))
            .tooltip_text(t!("hub.compat_only_tooltip"))
            .valign(Align::Center)
            .build();
        catalog_bar.append(&btn_compat_filter);

        catalog_group.add(&catalog_bar);

        // Empty state banner
        let catalog_empty_banner = adw::StatusPage::builder()
            .icon_name("cloud-download-symbolic")
            .title(t!("hub.empty_banner_title"))
            .description(t!("hub.empty_banner_desc"))
            .visible(false)
            .build();

        let btn_sync_from_banner = Button::builder()
            .label(t!("hub.sync_now_btn"))
            .css_classes(["suggested-action", "pill"])
            .halign(Align::Center)
            .margin_top(12)
            .build();
        catalog_empty_banner.set_child(Some(&btn_sync_from_banner));
        catalog_group.add(&catalog_empty_banner);

        // Model cards list
        let catalog_list = ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .focus_on_click(false)
            .can_focus(false)
            .css_classes(["catalog-model-list"])
            .build();
        catalog_group.add(&catalog_list);

        root_box.append(&catalog_group);

        clamp.set_child(Some(&root_box));
        container.set_child(Some(&clamp));

        let catalog_cards_rc = Rc::new(RefCell::new(Vec::new()));
        let raw_catalog_rc = Rc::new(RefCell::new(Vec::new()));
        let last_snap_rc = Rc::new(RefCell::new(HardwareSnapshot::default()));
        let installed_names_rc = Rc::new(RefCell::new(HashSet::new()));
        let current_sort_rc = Rc::new(RefCell::new(SortCriterion::Newest));
        let compat_only_rc = Rc::new(RefCell::new(false));

        let btn_sync_clone = btn_sync_online.clone();
        btn_sync_from_banner.connect_clicked(move |_| {
            btn_sync_clone.emit_clicked();
        });

        let view = Self {
            container,
            search_entry,
            btn_pull_action,
            category_box,
            catalog_bar,
            sort_dropdown,
            btn_compat_filter,
            catalog_list,
            catalog_empty_banner,
            btn_sync_online,
            catalog_cards: catalog_cards_rc,
            raw_catalog: raw_catalog_rc,
            last_snap: last_snap_rc,
            installed_names: installed_names_rc,
            active_category_filter: active_cat_rc,
            current_sort: current_sort_rc,
            compat_only_filter: compat_only_rc,
            catalog_loaded: Rc::new(RefCell::new(false)),
        };

        view.setup_internal_events();
        view
    }

    pub fn widget(&self) -> &ScrolledWindow {
        &self.container
    }

    fn setup_internal_events(&self) {
        let cards_rc = self.catalog_cards.clone();
        let search_ent = self.search_entry.clone();
        let active_cat_rc = self.active_category_filter.clone();
        let compat_only_rc = self.compat_only_filter.clone();
        let snap_rc = self.last_snap.clone();

        let apply_filters = move || {
            let query = search_ent.text().to_lowercase();
            let cat_filter = active_cat_rc.borrow().clone();
            let compat_only = *compat_only_rc.borrow();
            let snap = snap_rc.borrow().clone();

            for (_category, _model_id, card) in cards_rc.borrow().iter() {
                let visible = card.matches_filter(cat_filter.as_deref(), &query, compat_only, &snap);
                card.widget().set_visible(visible);
                if let Some(parent_row) = card.widget().parent() {
                    parent_row.set_visible(visible);
                }
                if visible && (cat_filter.as_deref() == Some("cloud") || cat_filter.as_deref() == Some("Cloud")) {
                    card.select_cloud_variant_if_available();
                }
            }
        };

        let filter_clone1 = apply_filters.clone();
        let btn_pull = self.btn_pull_action.clone();
        self.search_entry.connect_search_changed(move |entry| {
            let text = entry.text().to_string();
            let trimmed = text.trim();

            if trimmed.is_empty() {
                btn_pull.set_visible(false);
            } else {
                match crate::api::hub_remote::parse_model_input(trimmed) {
                    crate::api::hub_remote::ModelInputKind::ExplicitHf(_, _) => {
                        btn_pull.set_visible(true);
                        btn_pull.set_label(&t!("hub.inspect_hf_btn"));
                        btn_pull.set_tooltip_text(Some(&t!("hub.inspect_hf_tooltip")));
                        btn_pull.set_css_classes(&["suggested-action", "pill"]);
                    }
                    _ => {
                        btn_pull.set_visible(true);
                        btn_pull.set_label(&t!("hub.pull_btn"));
                        btn_pull.set_tooltip_text(Some(&t!("hub.pull_tooltip")));
                        btn_pull.set_css_classes(&["suggested-action", "pill"]);
                    }
                }
            }

            filter_clone1();
        });

        let filter_clone2 = apply_filters.clone();
        let compat_setter = self.compat_only_filter.clone();
        self.btn_compat_filter.connect_toggled(move |btn| {
            *compat_setter.borrow_mut() = btn.is_active();
            filter_clone2();
        });

        let categories = [
            (t!("hub.categories.all"), None),
            (t!("hub.categories.reasoning"), Some("reasoning")),
            (t!("hub.categories.vision"), Some("vision")),
            (t!("hub.categories.code"), Some("code")),
            (t!("hub.categories.lightweight"), Some("lightweight")),
            (t!("hub.categories.rag"), Some("rag")),
            (t!("hub.categories.cybersecurity"), Some("cybersecurity")),
            (t!("hub.categories.tools"), Some("tools")),
            (t!("hub.categories.cloud"), Some("cloud")),
        ];

        let mut child_opt = self.category_box.first_child();
        let mut idx = 0;
        while let Some(child) = child_opt {
            if let Some(flow_child) = child.downcast_ref::<FlowBoxChild>() {
                if let Some(btn) = flow_child.child().and_then(|w| w.downcast::<ToggleButton>().ok()) {
                    if idx < categories.len() {
                        let cat_id = categories[idx].1.map(|s| s.to_string());
                        let active_setter = self.active_category_filter.clone();
                        let filter_exec = apply_filters.clone();
                        let cat_box_clone = self.category_box.clone();
                        let current_btn = btn.clone();

                        btn.connect_toggled(move |toggled_btn| {
                            if toggled_btn.is_active() {
                                let mut sibling = cat_box_clone.first_child();
                                while let Some(sib_child) = sibling {
                                    if let Some(sib_flow) = sib_child.downcast_ref::<FlowBoxChild>() {
                                        if let Some(sib_btn) = sib_flow.child().and_then(|w| w.downcast::<ToggleButton>().ok()) {
                                            if !sib_btn.eq(&current_btn) && sib_btn.is_active() {
                                                sib_btn.set_active(false);
                                            }
                                        }
                                    }
                                    sibling = sib_child.next_sibling();
                                }

                                *active_setter.borrow_mut() = cat_id.clone();
                                filter_exec();
                            } else {
                                let mut any_active = false;
                                let mut sibling = cat_box_clone.first_child();
                                while let Some(sib_child) = sibling {
                                    if let Some(sib_flow) = sib_child.downcast_ref::<FlowBoxChild>() {
                                        if let Some(sib_btn) = sib_flow.child().and_then(|w| w.downcast::<ToggleButton>().ok()) {
                                            if sib_btn.is_active() {
                                                any_active = true;
                                                break;
                                            }
                                        }
                                    }
                                    sibling = sib_child.next_sibling();
                                }

                                if !any_active {
                                    *active_setter.borrow_mut() = None;
                                    filter_exec();
                                }
                            }
                        });
                    }
                }
            }
            idx += 1;
            child_opt = child.next_sibling();
        }
    }

    pub fn update_installed_and_hardware<FP>(
        &self,
        installed: &[ModelTag],
        snap: &HardwareSnapshot,
        on_pull: FP,
    ) where
        FP: Fn(String, bool) + Clone + 'static,
    {
        *self.last_snap.borrow_mut() = snap.clone();

        let mut installed_set = HashSet::new();
        for m in installed {
            installed_set.insert(m.name.clone());
            if let Some(ref tag) = m.model {
                installed_set.insert(tag.clone());
            }
            if m.is_cloud() {
                if let Some(ref rem) = m.remote_model {
                    if !rem.ends_with(":cloud") && !rem.ends_with("-cloud") {
                        installed_set.insert(format!("{}:cloud", rem));
                        installed_set.insert(format!("{}-cloud", rem));
                    } else {
                        installed_set.insert(rem.clone());
                    }
                }
            } else if let Some(ref rem) = m.remote_model {
                installed_set.insert(rem.clone());
            }
        }
        *self.installed_names.borrow_mut() = installed_set;

        let already_loaded = *self.catalog_loaded.borrow();
        if !already_loaded {
            *self.catalog_loaded.borrow_mut() = true;
            let mut catalog = HubManager::load_catalog();
            *self.raw_catalog.borrow_mut() = catalog.clone();

            let criterion = *self.current_sort.borrow();
            HubManager::sort_models(&mut catalog, criterion);
            self.populate_catalog_list(catalog, snap, on_pull);
        } else {
            self.refresh_installed_state(snap);
        }
    }

    pub fn refresh_installed_state(&self, snap: &HardwareSnapshot) {
        *self.last_snap.borrow_mut() = snap.clone();
        let installed_set = self.installed_names.borrow().clone();

        for (_cat, _id, card) in self.catalog_cards.borrow().iter() {
            card.update_installed_set(&installed_set, snap);
        }
    }

    pub fn populate_catalog_list<FP>(&self, catalog: Vec<HubModelInfo>, snap: &HardwareSnapshot, on_pull: FP)
    where
        FP: Fn(String, bool) + Clone + 'static,
    {
        while let Some(child) = self.catalog_list.first_child() {
            self.catalog_list.remove(&child);
        }
        self.catalog_cards.borrow_mut().clear();

        if catalog.is_empty() {
            self.catalog_empty_banner.set_visible(true);
            self.search_entry.set_visible(false);
            self.category_box.set_visible(false);
            self.catalog_bar.set_visible(false);
            self.catalog_list.set_visible(false);
            return;
        }
        self.catalog_empty_banner.set_visible(false);
        self.search_entry.set_visible(true);
        self.category_box.set_visible(true);
        self.catalog_bar.set_visible(true);
        self.catalog_list.set_visible(true);

        let installed_set = self.installed_names.borrow().clone();
        let installed_set_rc = Rc::new(installed_set);
        let on_pull_rc = Rc::new(on_pull);
        let cat_filter_rc = self.active_category_filter.clone();
        let search_ent = self.search_entry.clone();
        let compat_only_rc = self.compat_only_filter.clone();
        let snap_clone = snap.clone();

        glib::idle_add_local_once(
            glib::clone!(
                #[weak(rename_to = view_list)]
                self.catalog_list,
                #[weak(rename_to = cards_vec)]
                self.catalog_cards,
                move || {
                    for model_info in catalog {
                        let category = model_info.category.clone();
                        let model_id = model_info.id.clone();
                        let card = Rc::new(HubModelCard::new(model_info, &snap_clone, &installed_set_rc));

                        let on_pull_c = on_pull_rc.clone();
                        card.connect_download(move |tag, is_cancel| {
                            on_pull_c(tag, is_cancel);
                        });

                        let list_row = gtk4::ListBoxRow::builder()
                            .selectable(false)
                            .activatable(false)
                            .can_focus(false)
                            .child(card.widget())
                            .build();

                        let query = search_ent.text().to_lowercase();
                        let cat_filter = cat_filter_rc.borrow().clone();
                        let compat_only = *compat_only_rc.borrow();
                        let visible = card.matches_filter(cat_filter.as_deref(), &query, compat_only, &snap_clone);
                        card.widget().set_visible(visible);
                        list_row.set_visible(visible);

                        view_list.append(&list_row);
                        cards_vec.borrow_mut().push((category, model_id, card));
                    }
                }
            )
        );
    }

    pub fn set_pull_progress(&self, tag: &str, downloading: bool, status: &str, fraction: Option<f64>) {
        let tag_clean = tag.trim();
        for (_cat, _model_id, card) in self.catalog_cards.borrow().iter() {
            let matches = card.model_info().variants.iter().any(|v| {
                v.tag == tag_clean || tag_clean.starts_with(&v.tag) || v.tag.starts_with(tag_clean)
            });
            if matches {
                card.set_variant_download_progress(tag_clean, downloading, status, fraction);
            }
        }
    }

    pub fn set_syncing(&self, syncing: bool) {
        self.btn_sync_online.set_sensitive(!syncing);
        if syncing {
            self.btn_sync_online.set_label("⏳ Synchronisation...");
        } else {
            self.btn_sync_online.set_label(&t!("hub.sync_btn"));
        }
    }

    pub fn reset_catalog_loaded(&self) {
        *self.catalog_loaded.borrow_mut() = false;
    }

    pub fn connect_refresh_catalog<F: Fn() + 'static>(&self, f: F) {
        self.btn_sync_online.connect_clicked(move |_| {
            f();
        });
    }

    pub fn connect_sort_changed<F: Fn(SortCriterion) + 'static>(&self, f: F) {
        let sort_setter = self.current_sort.clone();
        self.sort_dropdown.connect_selected_notify(move |dd| {
            let crit = SortCriterion::from_index(dd.selected());
            *sort_setter.borrow_mut() = crit;
            f(crit);
        });
    }

    pub fn connect_custom_pull<F: Fn(String) + 'static>(&self, f: F) {
        let entry = self.search_entry.clone();
        let f_rc = Rc::new(f);

        let f_click = f_rc.clone();
        let entry_click = entry.clone();
        self.btn_pull_action.connect_clicked(move |_| {
            let text = entry_click.text().to_string();
            let trimmed = text.trim().to_string();
            if !trimmed.is_empty() {
                f_click(trimmed);
                entry_click.set_text("");
            }
        });

        let f_enter = f_rc;
        let entry_enter = entry;
        self.search_entry.connect_activate(move |_| {
            let text = entry_enter.text().to_string();
            let trimmed = text.trim().to_string();
            if !trimmed.is_empty() {
                f_enter(trimmed);
                entry_enter.set_text("");
            }
        });
    }

    pub fn focus_search(&self) {
        self.search_entry.grab_focus();
    }

    pub fn installed_names(&self) -> HashSet<String> {
        self.installed_names.borrow().clone()
    }

    pub fn raw_catalog(&self) -> Vec<HubModelInfo> {
        self.raw_catalog.borrow().clone()
    }
}
