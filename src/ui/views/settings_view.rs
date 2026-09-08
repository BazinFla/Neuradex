use crate::api::UpdateCheckResult;
use crate::core::config::{ApiProfile, ApiProviderType, AppConfig, DaemonMode};
use crate::core::hardware::estimator::format_gib;
use crate::core::i18n::detect_system_language;
use crate::core::lifecycle::{LifecycleManager, ServiceState};
use crate::core::spawn_async;
use crate::t;
use crate::ui::components::KeyProfileRow;

use adw::prelude::*;
use gtk4::{
    Align, Box, Button, FileDialog, Label, ListBox, Orientation, ProgressBar, StringList,
};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

struct DirtyCheckWidgets<'a> {
    host_entry: &'a adw::EntryRow,
    origins_entry: &'a adw::EntryRow,
    exposure_combo: &'a adw::ComboRow,
    flash_attn_switch: &'a adw::SwitchRow,
    daemon_combo: &'a adw::ComboRow,
    keep_alive_combo: &'a adw::ComboRow,
    staged_models_dir: &'a Option<String>,
}

pub struct SettingsView {
    page: adw::PreferencesPage,
    // Daemon
    daemon_status_row: adw::ActionRow,
    btn_start: Button,
    btn_stop: Button,
    btn_restart: Button,
    mode_combo: adw::ComboRow,
    update_row: adw::ActionRow,
    btn_check_update: Button,
    lang_row: adw::ComboRow,
    // Cloud connection & SSH profiles
    btn_wizard_help: Button,
    member_name_entry: adw::EntryRow,
    member_key_path_entry: adw::EntryRow,
    btn_generate_member_ssh: Button,
    btn_browse_member_ssh: Button,
    btn_save_member: Button,
    // Consolidated Save & Apply Bar
    save_row: adw::ActionRow,
    save_icon: gtk4::Image,
    btn_save_apply: Button,
    is_dirty: Rc<RefCell<bool>>,
    saved_models_dir: Rc<RefCell<Option<String>>>,
    // Network exposure
    exposure_combo: adw::ComboRow,
    entry_host: adw::EntryRow,
    entry_origins: adw::EntryRow,
    // Storage
    storage_row: adw::ActionRow,
    btn_browse_storage: Button,
    migration_row: adw::ActionRow,
    btn_migrate: Button,
    migration_source: Rc<RefCell<Option<PathBuf>>>,
    migration_progress_row: adw::ActionRow,
    mig_progress_bar: ProgressBar,
    mig_progress_status_label: Label,
    disk_space_label: Label,
    disk_bar: ProgressBar,
    switch_flash_att: adw::SwitchRow,
    keep_alive_combo: adw::ComboRow,

    // API Vault
    new_profile_card: ListBox,
    profiles_card: ListBox,
    btn_new_profile: Button,
    new_profile_type_combo: adw::ComboRow,
    new_profile_name_entry: adw::EntryRow,
    new_profile_token_entry: adw::PasswordEntryRow,
    new_profile_endpoint_entry: adw::EntryRow,
    btn_save_new_profile: Button,
    btn_cancel_new_profile: Button,
    // State
    config: Rc<RefCell<AppConfig>>,
    profile_rows: Rc<RefCell<Vec<Rc<KeyProfileRow>>>>,
}

impl SettingsView {
    pub fn new(config: Rc<RefCell<AppConfig>>) -> Self {
        let page = adw::PreferencesPage::builder()
            .title(t!("settings.page_title"))
            .icon_name("preferences-system-symbolic")
            .build();

        // ==========================================
        // TOP GROUP: Save & Apply Configuration Bar
        // ==========================================
        let save_group = adw::PreferencesGroup::builder().build();

        let save_row = adw::ActionRow::builder()
            .title(t!("settings.save_row_title"))
            .subtitle(t!("settings.save_row_sub_clean"))
            .build();

        let save_icon = gtk4::Image::from_icon_name("emblem-ok-symbolic");
        save_icon.set_pixel_size(24);
        save_row.add_prefix(&save_icon);

        let btn_save_apply = Button::builder()
            .label(t!("settings.btn_save_apply"))
            .valign(Align::Center)
            .sensitive(false)
            .build();
        save_row.add_suffix(&btn_save_apply);
        save_group.add(&save_row);
        page.add(&save_group);

        // ==========================================
        // GROUP 1: Daemon & Ollama Service Supervision
        // ==========================================
        let daemon_group = adw::PreferencesGroup::builder()
            .title(t!("settings.daemon_group_title"))
            .description(t!("settings.daemon_group_desc"))
            .build();

        let daemon_status_row = adw::ActionRow::builder()
            .title(t!("settings.service_status"))
            .subtitle(t!("instances.calculating"))
            .build();

        let daemon_icon = gtk4::Image::from_icon_name("system-run-symbolic");
        daemon_icon.set_pixel_size(24);
        daemon_status_row.add_prefix(&daemon_icon);

        let daemon_actions = Box::new(Orientation::Horizontal, 6);
        daemon_actions.set_valign(Align::Center);

        let btn_start = Button::builder()
            .label(t!("settings.btn_start"))
            .css_classes(["suggested-action"])
            .valign(Align::Center)
            .build();
        let btn_stop = Button::builder()
            .label(t!("settings.btn_stop"))
            .css_classes(["destructive-action"])
            .valign(Align::Center)
            .build();
        let btn_restart = Button::builder()
            .label(t!("settings.btn_restart"))
            .valign(Align::Center)
            .build();

        daemon_actions.append(&btn_start);
        daemon_actions.append(&btn_stop);
        daemon_actions.append(&btn_restart);
        daemon_status_row.add_suffix(&daemon_actions);
        daemon_group.add(&daemon_status_row);

        let mode_list = StringList::new(&[
            &t!("settings.mgmt_modes.0"),
            &t!("settings.mgmt_modes.1"),
            &t!("settings.mgmt_modes.2"),
        ]);
        let mode_combo = adw::ComboRow::builder()
            .title(t!("settings.mgmt_mode"))
            .subtitle(t!("settings.mgmt_mode_sub"))
            .model(&mode_list)
            .build();
        daemon_group.add(&mode_combo);

        let update_row = adw::ActionRow::builder()
            .title(t!("settings.updates_title"))
            .subtitle(t!("settings.updates_sub", ver = "..."))
            .build();

        let btn_check_update = Button::builder()
            .label(t!("settings.btn_check_update"))
            .valign(Align::Center)
            .build();
        update_row.add_suffix(&btn_check_update);
        daemon_group.add(&update_row);

        // Language Row
        let sys_code = detect_system_language();
        let sys_name = crate::core::i18n::get_language_name(sys_code);
        let lang_auto_label = t!("settings.lang_auto", lang = sys_name);

        let mut lang_labels = vec![lang_auto_label];
        for l in crate::core::i18n::AVAILABLE_LANGUAGES {
            lang_labels.push(format!("{} {}", l.flag, l.name));
        }

        let lang_refs: Vec<&str> = lang_labels.iter().map(|s| s.as_str()).collect();
        let lang_model = StringList::new(&lang_refs);
        let lang_row = adw::ComboRow::builder()
            .title(t!("settings.lang_title"))
            .subtitle(t!("settings.lang_sub"))
            .model(&lang_model)
            .build();

        let current_lang = config.borrow().language.clone();
        let initial_lang_idx = if current_lang == "auto" {
            0
        } else {
            crate::core::i18n::AVAILABLE_LANGUAGES
                .iter()
                .position(|l| l.code == current_lang)
                .map(|pos| pos as u32 + 1)
                .unwrap_or(0)
        };
        lang_row.set_selected(initial_lang_idx);

        let config_lang = config.clone();
        lang_row.connect_selected_notify(move |combo| {
            let selected = combo.selected();
            let new_lang = if selected == 0 {
                "auto"
            } else {
                crate::core::i18n::AVAILABLE_LANGUAGES
                    .get(selected as usize - 1)
                    .map(|l| l.code)
                    .unwrap_or("auto")
            };
            {
                let mut cfg = config_lang.borrow_mut();
                if cfg.language != new_lang {
                    cfg.language = new_lang.to_string();
                    let _ = cfg.save();
                }
            }
            crate::core::i18n::set_language(new_lang);
        });
        daemon_group.add(&lang_row);

        page.add(&daemon_group);

        // ==========================================
        // GROUP: Network Exposure & Remote Access
        // ==========================================
        let exposure_group = adw::PreferencesGroup::builder()
            .title(t!("settings.exposure_group_title"))
            .description(t!("settings.exposure_group_desc"))
            .build();

        let exposure_combo = adw::ComboRow::builder()
            .title(t!("settings.exposure_mode"))
            .subtitle(t!("settings.exposure_mode_sub"))
            .build();

        let exposure_model = StringList::new(&[
            &t!("settings.exposure_options.0"),
            &t!("settings.exposure_options.1"),
            &t!("settings.exposure_options.2"),
        ]);
        exposure_combo.set_model(Some(&exposure_model));

        let initial_exposure_idx = match config.borrow().exposure_mode {
            crate::core::config::ExposureMode::LocalHost => 0,
            crate::core::config::ExposureMode::Lan => 1,
            crate::core::config::ExposureMode::Exposed => 2,
        };
        exposure_combo.set_selected(initial_exposure_idx);
        exposure_group.add(&exposure_combo);

        let entry_host = adw::EntryRow::builder()
            .title(t!("settings.entry_host"))
            .text(&config.borrow().ollama_host)
            .build();
        exposure_group.add(&entry_host);

        let entry_origins = adw::EntryRow::builder()
            .title(t!("settings.entry_origins"))
            .text(config.borrow().ollama_origins.as_deref().unwrap_or(""))
            .build();
        exposure_group.add(&entry_origins);

        page.add(&exposure_group);

        // ==========================================
        // GROUP 2: Storage & Environment Variables
        // ==========================================
        let storage_group = adw::PreferencesGroup::builder()
            .title(t!("settings.storage_group_title"))
            .description(t!("settings.storage_group_desc"))
            .build();

        let current_models_dir = config.borrow().models_directory.clone();
        let initial_storage_sub = if let Some(dir) = current_models_dir {
            dir
        } else if let Some(sysd_dir) = LifecycleManager::get_active_systemd_models_dir() {
            let s = sysd_dir.to_string_lossy().to_string();
            config.borrow_mut().models_directory = Some(s.clone());
            s
        } else if Path::new("/usr/share/ollama").exists() {
            format!("{} (/usr/share/ollama/.ollama/models)", t!("settings.default_label"))
        } else {
            format!("{} (~/.ollama/models)", t!("settings.default_label"))
        };

        let storage_row = adw::ActionRow::builder()
            .title(t!("settings.storage_dir_title"))
            .subtitle(&initial_storage_sub)
            .build();

        let btn_browse_storage = Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text(t!("settings.storage_btn_browse_tooltip"))
            .css_classes(["flat"])
            .valign(Align::Center)
            .build();
        storage_row.add_suffix(&btn_browse_storage);
        storage_group.add(&storage_row);

        let migration_row = adw::ActionRow::builder()
            .title(t!("settings.migration_detected_title"))
            .subtitle(t!("settings.migration_detected_sub"))
            .build();
        migration_row.set_visible(false);

        let icon_pkg = gtk4::Image::from_icon_name("package-x-generic-symbolic");
        icon_pkg.set_pixel_size(24);
        migration_row.add_prefix(&icon_pkg);

        let btn_migrate = Button::builder()
            .label(t!("settings.btn_migrate"))
            .tooltip_text(t!("settings.btn_migrate_tooltip"))
            .css_classes(["suggested-action"])
            .valign(Align::Center)
            .build();
        migration_row.add_suffix(&btn_migrate);
        storage_group.add(&migration_row);

        // Realtime migration progress row
        let migration_progress_row = adw::ActionRow::builder()
            .title(t!("settings.migration_progress_title"))
            .subtitle(t!("settings.migration_progress_sub"))
            .build();
        migration_progress_row.set_visible(false);

        let mig_prog_icon = gtk4::Image::from_icon_name("network-transmit-receive-symbolic");
        mig_prog_icon.set_pixel_size(24);
        migration_progress_row.add_prefix(&mig_prog_icon);

        let mig_prog_box = Box::new(Orientation::Vertical, 4);
        mig_prog_box.set_valign(Align::Center);
        mig_prog_box.set_size_request(240, -1);

        let mig_progress_bar = ProgressBar::builder()
            .show_text(false)
            .fraction(0.0)
            .build();
        mig_progress_bar.set_css_classes(&["storage-progress"]);

        let mig_progress_status_label = Label::builder()
            .label(t!("settings.migration_progress_sub"))
            .css_classes(["caption", "dim-label"])
            .halign(Align::End)
            .build();

        mig_prog_box.append(&mig_progress_bar);
        mig_prog_box.append(&mig_progress_status_label);
        migration_progress_row.add_suffix(&mig_prog_box);
        storage_group.add(&migration_progress_row);

        let migration_source = Rc::new(RefCell::new(None));

        let disk_row = adw::ActionRow::builder()
            .title(t!("settings.disk_space_title"))
            .build();

        let disk_box = Box::new(Orientation::Vertical, 4);
        disk_box.set_valign(Align::Center);
        disk_box.set_size_request(200, -1);

        let disk_bar = ProgressBar::builder()
            .show_text(false)
            .fraction(0.0)
            .build();
        disk_bar.set_css_classes(&["storage-progress"]);

        let disk_space_label = Label::builder()
            .label(t!("instances.calculating"))
            .css_classes(["caption", "dim-label"])
            .halign(Align::End)
            .build();

        disk_box.append(&disk_bar);
        disk_box.append(&disk_space_label);
        disk_row.add_suffix(&disk_box);
        storage_group.add(&disk_row);

        let switch_flash_att = adw::SwitchRow::builder()
            .title(t!("settings.switch_flash_att_title"))
            .subtitle(t!("settings.switch_flash_att_sub"))
            .build();
        storage_group.add(&switch_flash_att);

        let keep_alive_values_vec = [t!("settings.keep_alive_default"),
            "1m".to_string(),
            "5m".to_string(),
            "10m".to_string(),
            "30m".to_string(),
            "1h".to_string(),
            "24h".to_string(),
            t!("settings.keep_alive_infinite")];
        let keep_alive_refs: Vec<&str> = keep_alive_values_vec.iter().map(|s| s.as_str()).collect();
        let keep_alive_list = StringList::new(&keep_alive_refs);
        let keep_alive_combo = adw::ComboRow::builder()
            .title(t!("settings.keep_alive_title"))
            .subtitle(t!("settings.keep_alive_sub"))
            .model(&keep_alive_list)
            .build();

        let initial_ka_idx = match config.borrow().keep_alive.as_deref() {
            None | Some("") => 0,
            Some("-1") => 7,
            Some("1m") => 1,
            Some("5m") => 2,
            Some("10m") => 3,
            Some("30m") => 4,
            Some("1h") => 5,
            Some("24h") => 6,
            _ => 0,
        };
        keep_alive_combo.set_selected(initial_ka_idx);
        storage_group.add(&keep_alive_combo);

        page.add(&storage_group);

        // ==========================================
        // GROUP: Cloud Accounts & User Profiles
        // ==========================================
        let wizard_group = adw::PreferencesGroup::builder()
            .title(t!("settings.wizard_group_title"))
            .description(t!("settings.wizard_group_desc"))
            .build();

        let btn_wizard_help = Button::builder()
            .icon_name("help-about-symbolic")
            .tooltip_text(t!("settings.wizard_help_tooltip"))
            .css_classes(["flat", "circular"])
            .valign(Align::Center)
            .build();
        wizard_group.set_header_suffix(Some(&btn_wizard_help));

        let member_name_entry = adw::EntryRow::builder()
            .title(t!("settings.member_name"))
            .build();
        wizard_group.add(&member_name_entry);

        let member_key_path_entry = adw::EntryRow::builder()
            .title(t!("settings.member_key_path"))
            .build();

        let member_key_actions = Box::new(Orientation::Horizontal, 6);
        member_key_actions.set_valign(Align::Center);

        let btn_generate_member_ssh = Button::builder()
            .label(t!("settings.btn_generate_ssh"))
            .icon_name("document-new-symbolic")
            .tooltip_text(t!("settings.btn_generate_ssh_tooltip"))
            .valign(Align::Center)
            .build();

        let btn_browse_member_ssh = Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text(t!("settings.btn_browse_ssh_tooltip"))
            .css_classes(["flat"])
            .valign(Align::Center)
            .build();

        member_key_actions.append(&btn_generate_member_ssh);
        member_key_actions.append(&btn_browse_member_ssh);
        member_key_path_entry.add_suffix(&member_key_actions);
        wizard_group.add(&member_key_path_entry);

        let member_action_row = adw::ActionRow::builder()
            .title(t!("settings.save_member_title"))
            .subtitle(t!("settings.save_member_sub"))
            .build();

        let btn_save_member = Button::builder()
            .label(t!("settings.btn_save_member"))
            .icon_name("list-add-symbolic")
            .tooltip_text(t!("settings.btn_save_member_tooltip"))
            .css_classes(["suggested-action"])
            .valign(Align::Center)
            .build();

        member_action_row.add_suffix(&btn_save_member);
        wizard_group.add(&member_action_row);

        page.add(&wizard_group);

        // ==========================================
        // GROUP 3: API Key Vault & Profiles
        // ==========================================
        let profiles_group = adw::PreferencesGroup::builder()
            .title(t!("settings.profiles_group_title"))
            .description(t!("settings.profiles_group_desc"))
            .build();

        let btn_new_profile = Button::builder()
            .label(t!("settings.btn_new_profile"))
            .tooltip_text(t!("settings.btn_new_profile_tooltip"))
            .css_classes(["suggested-action"])
            .valign(Align::Center)
            .build();
        profiles_group.set_header_suffix(Some(&btn_new_profile));

        // Form for adding new API Key
        let new_profile_card = ListBox::builder()
            .css_classes(["boxed-list"])
            .selection_mode(gtk4::SelectionMode::None)
            .margin_bottom(16)
            .visible(false)
            .build();

        let provider_list = StringList::new(&[
            &t!("settings.providers.0"),
            &t!("settings.providers.1"),
            &t!("settings.providers.2"),
            &t!("settings.providers.3"),
        ]);
        let new_profile_type_combo = adw::ComboRow::builder()
            .title(t!("settings.provider_type"))
            .model(&provider_list)
            .build();
        new_profile_card.append(&new_profile_type_combo);

        let new_profile_name_entry = adw::EntryRow::builder()
            .title(t!("settings.profile_name"))
            .build();
        new_profile_card.append(&new_profile_name_entry);

        let new_profile_token_entry = adw::PasswordEntryRow::builder()
            .title(t!("settings.profile_token"))
            .build();
        new_profile_card.append(&new_profile_token_entry);

        let new_profile_endpoint_entry = adw::EntryRow::builder()
            .title(t!("settings.profile_endpoint"))
            .text("https://ollama.com")
            .build();
        new_profile_card.append(&new_profile_endpoint_entry);

        let new_profile_action_row = adw::ActionRow::builder()
            .title(t!("settings.validate_key_title"))
            .build();

        let btn_cancel_new_profile = Button::builder()
            .label(t!("settings.btn_cancel"))
            .css_classes(["flat"])
            .valign(Align::Center)
            .build();
        let btn_save_new_profile = Button::builder()
            .label(t!("settings.btn_save_key"))
            .icon_name("list-add-symbolic")
            .css_classes(["suggested-action"])
            .valign(Align::Center)
            .build();

        let form_btn_box = Box::new(Orientation::Horizontal, 8);
        form_btn_box.set_valign(Align::Center);
        form_btn_box.append(&btn_cancel_new_profile);
        form_btn_box.append(&btn_save_new_profile);
        new_profile_action_row.add_suffix(&form_btn_box);
        new_profile_card.append(&new_profile_action_row);

        profiles_group.add(&new_profile_card);

        // Profiles list container
        let profiles_card = ListBox::builder()
            .css_classes(["boxed-list"])
            .selection_mode(gtk4::SelectionMode::None)
            .build();

        profiles_group.add(&profiles_card);

        page.add(&profiles_group);

        // ==========================================
        // GROUP: System Diagnostic & Support
        // ==========================================
        let diag_group = adw::PreferencesGroup::builder()
            .title(t!("settings.diagnostic_group_title"))
            .description(t!("settings.diagnostic_group_desc"))
            .build();

        let diag_row = adw::ActionRow::builder()
            .title(t!("settings.diagnostic_row_title"))
            .subtitle(t!("settings.diagnostic_row_sub"))
            .build();

        let diag_icon = gtk4::Image::from_icon_name("utilities-system-monitor-symbolic");
        diag_icon.set_pixel_size(24);
        diag_row.add_prefix(&diag_icon);

        let diag_actions = Box::new(Orientation::Horizontal, 8);
        diag_actions.set_valign(Align::Center);

        let btn_copy_diag = Button::builder()
            .label(t!("settings.btn_copy_diagnostic"))
            .tooltip_text(t!("settings.btn_copy_diagnostic_tooltip"))
            .css_classes(["suggested-action"])
            .valign(Align::Center)
            .build();

        let btn_view_diag = Button::builder()
            .label(t!("settings.btn_view_diagnostic"))
            .tooltip_text(t!("settings.btn_view_diagnostic_tooltip"))
            .valign(Align::Center)
            .build();

        diag_actions.append(&btn_copy_diag);
        diag_actions.append(&btn_view_diag);
        diag_row.add_suffix(&diag_actions);
        diag_group.add(&diag_row);

        page.add(&diag_group);

        // Wire Diagnostic Actions
        let config_diag = config.clone();
        let btn_copy_label = btn_copy_diag.clone();
        btn_copy_diag.connect_clicked(move |_| {
            let cfg = config_diag.borrow().clone();
            let btn = btn_copy_label.clone();
            btn.set_sensitive(false);
            btn.set_label(&t!("settings.diagnostic_loading"));

            spawn_async(
                async move {
                    crate::core::generate_diagnostic_report(&cfg).await
                },
                move |report| {
                    if let Some(display) = gtk4::gdk::Display::default() {
                        display.clipboard().set_text(&report);
                    }
                    btn.set_sensitive(true);
                    btn.set_label(&t!("settings.btn_copy_diagnostic_copied"));
                    let btn_reset = btn.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_secs(2), move || {
                        btn_reset.set_label(&t!("settings.btn_copy_diagnostic"));
                    });
                }
            );
        });

        let config_view_diag = config.clone();
        let page_for_dialog = page.clone();
        btn_view_diag.connect_clicked(move |_| {
            let cfg = config_view_diag.borrow().clone();
            let page_ref = page_for_dialog.clone();

            spawn_async(
                async move {
                    crate::core::generate_diagnostic_report(&cfg).await
                },
                move |report| {
                    show_diagnostic_dialog(&page_ref, &report);
                }
            );
        });

        let initial_models_dir = config.borrow().models_directory.clone();

        let view = Self {
            page,
            daemon_status_row,
            btn_start,
            btn_stop,
            btn_restart,
            mode_combo,
            update_row,
            btn_check_update,
            lang_row,
            btn_wizard_help,
            member_name_entry,
            member_key_path_entry,
            btn_generate_member_ssh,
            btn_browse_member_ssh,
            btn_save_member,
            save_row,
            save_icon,
            btn_save_apply,
            is_dirty: Rc::new(RefCell::new(false)),
            saved_models_dir: Rc::new(RefCell::new(initial_models_dir)),
            exposure_combo,
            entry_host,
            entry_origins,
            storage_row,
            btn_browse_storage,
            migration_row,
            btn_migrate,
            migration_source,
            migration_progress_row,
            mig_progress_bar,
            mig_progress_status_label,
            disk_space_label,
            disk_bar,
            switch_flash_att,
            keep_alive_combo,

            new_profile_card,
            profiles_card,
            btn_new_profile,
            new_profile_type_combo,
            new_profile_name_entry,
            new_profile_token_entry,
            new_profile_endpoint_entry,
            btn_save_new_profile,
            btn_cancel_new_profile,
            config,
            profile_rows: Rc::new(RefCell::new(Vec::new())),
        };

        view.setup_internal_events();
        view.refresh_disk_space();
        view.refresh_migration_status();

        view
    }

    pub fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }

    fn setup_internal_events(&self) {
        // Track dirty state on settings widgets
        let cfg_dirty = self.config.clone();
        let h_dirty = self.entry_host.clone();
        let o_dirty = self.entry_origins.clone();
        let ec_dirty = self.exposure_combo.clone();
        let fa_dirty = self.switch_flash_att.clone();
        let cm_dirty = self.mode_combo.clone();
        let ka_dirty = self.keep_alive_combo.clone();
        let is_dirty_state = self.is_dirty.clone();
        let save_row_state = self.save_row.clone();
        let save_icon_state = self.save_icon.clone();
        let btn_save_state = self.btn_save_apply.clone();
        let smd_dirty = self.saved_models_dir.clone();

        let check_and_update = Rc::new({
            let cfg = cfg_dirty.clone();
            let h = h_dirty.clone();
            let o = o_dirty.clone();
            let ec = ec_dirty.clone();
            let fa = fa_dirty.clone();
            let cm = cm_dirty.clone();
            let ka = ka_dirty.clone();
            let is_dirty = is_dirty_state.clone();
            let save_row = save_row_state.clone();
            let save_icon = save_icon_state.clone();
            let btn_save = btn_save_state.clone();
            let smd = smd_dirty.clone();
            move || {
                let widgets = DirtyCheckWidgets {
                    host_entry: &h,
                    origins_entry: &o,
                    exposure_combo: &ec,
                    flash_attn_switch: &fa,
                    daemon_combo: &cm,
                    keep_alive_combo: &ka,
                    staged_models_dir: &smd.borrow(),
                };
                let dirty = Self::check_is_dirty(&cfg.borrow(), &widgets);
                Self::update_dirty_ui(&is_dirty, dirty, &save_row, &save_icon, &btn_save);
            }
        });

        let cb = check_and_update.clone();
        self.entry_host.connect_changed(move |_| cb());

        let cb = check_and_update.clone();
        self.entry_origins.connect_changed(move |_| cb());

        let cb = check_and_update.clone();
        self.exposure_combo.connect_selected_notify(move |_| cb());

        let cb = check_and_update.clone();
        self.switch_flash_att.connect_active_notify(move |_| cb());

        let cb = check_and_update.clone();
        self.mode_combo.connect_selected_notify(move |_| cb());

        let cb = check_and_update.clone();
        self.keep_alive_combo.connect_selected_notify(move |_| cb());

        // Help popup for cloud accounts & profiles
        let btn_help = self.btn_wizard_help.clone();
        let page_clone_help = self.page.clone();
        btn_help.connect_clicked(move |_| {
            let parent = page_clone_help.root().and_then(|r| r.downcast::<gtk4::Window>().ok());

            let dialog = adw::Dialog::builder()
                .title(t!("settings.wizard_help_title"))
                .content_width(540)
                .content_height(480)
                .build();

            let toolbar_view = adw::ToolbarView::new();
            let header_bar = adw::HeaderBar::new();
            toolbar_view.add_top_bar(&header_bar);

            let clamp = adw::Clamp::builder()
                .maximum_size(500)
                .tightening_threshold(400)
                .margin_top(16)
                .margin_bottom(24)
                .margin_start(16)
                .margin_end(16)
                .build();

            let scroll = gtk4::ScrolledWindow::builder()
                .hscrollbar_policy(gtk4::PolicyType::Never)
                .vscrollbar_policy(gtk4::PolicyType::Automatic)
                .build();

            let content_box = Box::new(Orientation::Vertical, 16);

            // Popup header
            let header_box = Box::new(Orientation::Horizontal, 12);
            header_box.set_halign(Align::Center);
            header_box.set_margin_top(8);
            header_box.set_margin_bottom(4);

            let icon = gtk4::Image::from_icon_name("system-users-symbolic");
            icon.set_pixel_size(36);
            icon.set_css_classes(&["accent"]);
            header_box.append(&icon);

            let title_box = Box::new(Orientation::Vertical, 2);
            let title_lbl = Label::builder()
                .label(t!("settings.wizard_help_header_title"))
                .css_classes(["title-2"])
                .halign(Align::Start)
                .build();
            let sub_lbl = Label::builder()
                .label(t!("settings.wizard_help_header_desc"))
                .css_classes(["caption", "dim-label"])
                .halign(Align::Start)
                .build();
            title_box.append(&title_lbl);
            title_box.append(&sub_lbl);
            header_box.append(&title_box);
            content_box.append(&header_box);

            // Group 1: Why profiles?
            let group_why = adw::PreferencesGroup::builder()
                .title(t!("settings.wizard_help_why_title"))
                .build();

            let row_why = adw::ActionRow::builder()
                .title(t!("settings.wizard_help_why_title"))
                .subtitle(t!("settings.wizard_help_why_desc"))
                .subtitle_lines(0)
                .build();
            let icon_users = gtk4::Image::from_icon_name("avatar-default-symbolic");
            icon_users.set_pixel_size(24);
            row_why.add_prefix(&icon_users);
            group_why.add(&row_why);
            content_box.append(&group_why);

            // Group 2: How does it work?
            let group_how = adw::PreferencesGroup::builder()
                .title(t!("settings.wizard_help_how_title"))
                .build();

            let row_how = adw::ActionRow::builder()
                .title(t!("settings.wizard_help_how_title"))
                .subtitle(t!("settings.wizard_help_how_desc"))
                .subtitle_lines(0)
                .build();
            let icon_help = gtk4::Image::from_icon_name("dialog-information-symbolic");
            icon_help.set_pixel_size(24);
            row_how.add_prefix(&icon_help);
            group_how.add(&row_how);
            content_box.append(&group_how);

            clamp.set_child(Some(&content_box));
            scroll.set_child(Some(&clamp));
            toolbar_view.set_content(Some(&scroll));
            dialog.set_child(Some(&toolbar_view));

            dialog.present(parent.as_ref());
        });

        // Browse for existing SSH key
        let path_entry_clone = self.member_key_path_entry.clone();
        let page_clone = self.page.clone();
        self.btn_browse_member_ssh.connect_clicked(move |_| {
            let parent = page_clone.root().and_then(|r| r.downcast::<gtk4::Window>().ok());
            let file_dialog = FileDialog::builder()
                .title("Select Private SSH Key")
                .modal(true)
                .build();

            let home = std::env::var("HOME").unwrap_or_default();
            let initial_dir = PathBuf::from(&home).join(".ollama");
            if initial_dir.is_dir() {
                let gfile = gtk4::gio::File::for_path(&initial_dir);
                file_dialog.set_initial_folder(Some(&gfile));
            }

            let entry_inner = path_entry_clone.clone();
            file_dialog.open(parent.as_ref(), gtk4::gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        let path_str = path.to_string_lossy().to_string();
                        let clean_path = if path_str.ends_with(".pub") {
                            path_str.trim_end_matches(".pub").to_string()
                        } else {
                            path_str
                        };
                        entry_inner.set_text(&clean_path);
                    }
                }
            });
        });

        let np_card = self.new_profile_card.clone();
        self.btn_new_profile.connect_clicked(move |_| {
            let vis = np_card.get_visible();
            np_card.set_visible(!vis);
        });

        let np_card_cancel = self.new_profile_card.clone();
        self.btn_cancel_new_profile.connect_clicked(move |_| {
            np_card_cancel.set_visible(false);
        });

        // Dynamic form updates based on selected provider type
        let ep_entry = self.new_profile_endpoint_entry.clone();
        let name_entry = self.new_profile_name_entry.clone();
        let token_entry = self.new_profile_token_entry.clone();

        self.new_profile_type_combo.connect_selected_notify(move |combo| {
            match combo.selected() {
                0 => {
                    // Ollama Cloud (API Key)
                    token_entry.set_visible(true);
                    ep_entry.set_visible(true);
                    ep_entry.set_text("https://ollama.com");
                    token_entry.set_title("Ollama Cloud API Key (Bearer Token)");
                    if name_entry.text().is_empty() {
                        name_entry.set_text("Ollama Cloud");
                    }
                }
                1 => {
                    // Hugging Face
                    token_entry.set_visible(true);
                    ep_entry.set_visible(true);
                    ep_entry.set_text("https://huggingface.co");
                    token_entry.set_title("Hugging Face Token (hf_...)");
                    if name_entry.text().is_empty() {
                        name_entry.set_text("HF Account");
                    }
                }
                2 => {
                    // Remote Ollama Server
                    token_entry.set_visible(true);
                    ep_entry.set_visible(true);
                    ep_entry.set_text("http://127.0.0.1:11434");
                    token_entry.set_title("API Key / Access Token (Optional)");
                    if name_entry.text().is_empty() {
                        name_entry.set_text("Remote Server");
                    }
                }
                _ => {
                    // Custom Cloud / Proxy
                    token_entry.set_visible(true);
                    ep_entry.set_visible(true);
                    ep_entry.set_text("https://api.openai.com");
                    token_entry.set_title("API Key (Bearer Token)");
                    if name_entry.text().is_empty() {
                        name_entry.set_text("Custom Proxy");
                    }
                }
            }
        });

        // Storage Directory Selection
        let config_clone = self.config.clone();
        let storage_row_clone = self.storage_row.clone();
        let disk_bar_clone = self.disk_bar.clone();
        let disk_space_label_clone = self.disk_space_label.clone();
        let page_clone = self.page.clone();
        let migration_row_browse = self.migration_row.clone();
        let migration_source_browse = self.migration_source.clone();

        let cb_browse = check_and_update.clone();

        self.btn_browse_storage.connect_clicked(move |_| {
            let parent = page_clone.root().and_then(|r| r.downcast::<gtk4::Window>().ok());
            let file_dialog = FileDialog::builder()
                .title("Select Models Storage Directory")
                .modal(true)
                .build();

            let config_inner = config_clone.clone();
            let storage_row_inner = storage_row_clone.clone();
            let disk_bar_inner = disk_bar_clone.clone();
            let disk_space_label_inner = disk_space_label_clone.clone();
            let page_inner = page_clone.clone();
            let mig_row_inner = migration_row_browse.clone();
            let mig_source_inner = migration_source_browse.clone();
            let cb_browse_inner = cb_browse.clone();

            file_dialog.select_folder(parent.as_ref(), gtk4::gio::Cancellable::NONE, move |res| {
                if let Ok(folder) = res {
                    let path_opt = folder.path().or_else(|| {
                        let uri = folder.uri();
                        let decoded = glib::uri_unescape_string(&uri, Option::<&str>::None)?;
                        decoded.strip_prefix("file://").map(PathBuf::from)
                    });

                    if let Some(path) = path_opt {
                        let path_str = path.to_string_lossy().to_string();
                        let previous_dir = config_inner.borrow().models_directory.as_ref().map(PathBuf::from);

                        storage_row_inner.set_subtitle(&path_str);
                        config_inner.borrow_mut().models_directory = Some(path_str.clone());
                        let _ = config_inner.borrow().save();
                        cb_browse_inner();

                        // Refresh disk space immediately
                        if let Some(info) = LifecycleManager::get_disk_space_for_path(&path) {
                            let total = info.total_bytes;
                            let avail = info.available_bytes;
                            let used = total.saturating_sub(avail);
                            let ratio = if total > 0 {
                                (used as f64 / total as f64).clamp(0.0, 1.0)
                            } else {
                                0.0
                            };

                            disk_bar_inner.set_fraction(ratio);
                            disk_space_label_inner.set_label(&format!(
                                "{} free of {} ({:.0}% used)",
                                format_gib(avail),
                                format_gib(total),
                                ratio * 100.0
                            ));
                        }

                        // Refresh migration check
                        let custom_dir_pb = Some(path.clone());
                        if let Some((src_path, count, size_bytes)) = LifecycleManager::detect_default_models_store(previous_dir.as_deref(), custom_dir_pb.as_deref()) {
                            *mig_source_inner.borrow_mut() = Some(src_path.clone());
                            let size_str = if size_bytes > 0 {
                                format!(" (~{})", format_gib(size_bytes))
                            } else {
                                String::new()
                            };
                            mig_row_inner.set_subtitle(&format!(
                                "{} model(s) detected{} in {}",
                                count,
                                size_str,
                                src_path.display()
                            ));
                            mig_row_inner.set_visible(true);
                        } else {
                            *mig_source_inner.borrow_mut() = None;
                            mig_row_inner.set_visible(false);
                        }

                        // Check directory permissions for Ollama
                        if let Err(perm_err) = LifecycleManager::check_directory_permissions_for_ollama(&path) {
                            let alert = adw::AlertDialog::builder()
                                .heading(t!("settings.perm_dialog_title"))
                                .body(t!(
                                    "settings.perm_dialog_body",
                                    path = path_str.clone(),
                                    error = perm_err
                                ))
                                .build();

                            alert.add_response("cancel", &t!("settings.perm_btn_cancel"));
                            alert.add_response("fix", &t!("settings.perm_btn_fix"));
                            alert.set_response_appearance("fix", adw::ResponseAppearance::Suggested);

                            let path_to_fix = path.clone();
                            let parent_win = page_inner.root().and_then(|r| r.downcast::<gtk4::Window>().ok());
                            let page_for_alert = page_inner.clone();
                            
                            let alert_cb = move |response: glib::GString| {
                                if response.as_str() == "fix" {
                                    let page_toast = page_for_alert.clone();
                                    let p_str = path_str.clone();
                                    spawn_async(
                                        async move {
                                            LifecycleManager::fix_directory_permissions(&path_to_fix).await
                                        },
                                        move |fix_res| {
                                            match fix_res {
                                                Ok(msg) => {
                                                    let info_dialog = adw::AlertDialog::builder()
                                                        .heading(t!("settings.perm_success_title"))
                                                        .body(&msg)
                                                        .build();
                                                    info_dialog.add_response("ok", &t!("settings.btn_ok"));
                                                    info_dialog.choose(&page_toast, gtk4::gio::Cancellable::NONE, |_| {});
                                                }
                                                Err(err) => {
                                                    let err_dialog = adw::AlertDialog::builder()
                                                        .heading(t!("settings.perm_error_title"))
                                                        .body(t!(
                                                            "settings.perm_error_body",
                                                            err = err,
                                                            path = p_str
                                                        ))
                                                        .build();
                                                    err_dialog.add_response("ok", &t!("settings.btn_close"));
                                                    err_dialog.choose(&page_toast, gtk4::gio::Cancellable::NONE, |_| {});
                                                }
                                            }
                                        }
                                    );
                                }
                            };

                            if let Some(ref win) = parent_win {
                                alert.choose(win, gtk4::gio::Cancellable::NONE, alert_cb);
                            } else {
                                alert.choose(&page_inner, gtk4::gio::Cancellable::NONE, alert_cb);
                            }
                        }
                    }
                }
            });
        });

        // Migrate models from default directory to new directory
        let btn_migrate_clone = self.btn_migrate.clone();
        let migration_source_clone = self.migration_source.clone();
        let config_mig = self.config.clone();
        let page_mig = self.page.clone();
        let migration_row_mig = self.migration_row.clone();
        let mig_progress_row_clone = self.migration_progress_row.clone();
        let mig_progress_bar_clone = self.mig_progress_bar.clone();
        let mig_progress_status_clone = self.mig_progress_status_label.clone();
        let disk_bar_mig = self.disk_bar.clone();
        let disk_space_label_mig = self.disk_space_label.clone();

        self.btn_migrate.connect_clicked(move |_| {
            let src_opt = migration_source_clone.borrow().clone();
            let dst_opt = config_mig.borrow().models_directory.as_ref().map(PathBuf::from);

            if let (Some(src), Some(dst)) = (src_opt, dst_opt) {
                let alert = adw::AlertDialog::builder()
                    .heading(t!("settings.mig_dialog_title"))
                    .body(t!(
                        "settings.mig_dialog_body",
                        src = src.display().to_string(),
                        dst = dst.display().to_string()
                    ))
                    .build();

                alert.add_response("cancel", &t!("settings.mig_btn_cancel"));
                alert.add_response("migrate", &t!("settings.mig_btn_now"));
                alert.set_response_appearance("migrate", adw::ResponseAppearance::Suggested);

                let btn_mig_inner = btn_migrate_clone.clone();
                let page_inner = page_mig.clone();
                let src_inner = src.clone();
                let dst_inner = dst.clone();
                let mig_source_inner = migration_source_clone.clone();
                let mig_row_inner = migration_row_mig.clone();
                let mig_prog_row_inner = mig_progress_row_clone.clone();
                let mig_prog_bar_inner = mig_progress_bar_clone.clone();
                let mig_prog_status_inner = mig_progress_status_clone.clone();
                let disk_bar_inner = disk_bar_mig.clone();
                let disk_space_label_inner = disk_space_label_mig.clone();
                let config_mig_inner = config_mig.clone();

                alert.choose(&page_mig, gtk4::gio::Cancellable::NONE, move |response| {
                    if response == "migrate" {
                        btn_mig_inner.set_sensitive(false);
                        btn_mig_inner.set_label(&t!("settings.migration_progress_sub"));

                        mig_prog_row_inner.set_visible(true);
                        mig_prog_bar_inner.set_fraction(0.0);
                        mig_prog_status_inner.set_label(&t!("settings.migration_progress_sub"));

                        let (tx, rx) = async_channel::unbounded::<crate::core::lifecycle::MigrationProgress>();
                        let is_finished = std::rc::Rc::new(std::cell::Cell::new(false));
                        let is_finished_timer = is_finished.clone();

                        let prog_row_timer = mig_prog_row_inner.clone();
                        let prog_bar_timer = mig_prog_bar_inner.clone();
                        let prog_status_timer = mig_prog_status_inner.clone();

                        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                            if is_finished_timer.get() {
                                return glib::ControlFlow::Break;
                            }
                            let mut latest_progress = None;
                            while let Ok(progress) = rx.try_recv() {
                                latest_progress = Some(progress);
                            }
                            if let Some(progress) = latest_progress {
                                prog_row_timer.set_subtitle(&progress.step);
                                if let Some(frac) = progress.fraction {
                                    prog_bar_timer.set_fraction(frac);
                                }
                                let mut details = Vec::new();
                                if let Some(frac) = progress.fraction {
                                    details.push(format!("{:.0}%", frac * 100.0));
                                }
                                if let Some(bytes) = progress.transferred_bytes {
                                    details.push(format_gib(bytes));
                                }
                                if let Some(ref speed) = progress.speed {
                                    details.push(speed.clone());
                                }
                                if let Some(ref eta) = progress.eta {
                                    details.push(format!("ETA {}", eta));
                                }
                                if !details.is_empty() {
                                    prog_status_timer.set_label(&details.join(" • "));
                                } else {
                                    prog_status_timer.set_label(&progress.step);
                                }
                            }
                            glib::ControlFlow::Continue
                        });

                        let btn_done = btn_mig_inner.clone();
                        let page_done = page_inner.clone();
                        let dst_done = dst_inner.clone();
                        let prog_row_done = mig_prog_row_inner.clone();
                        let cfg_snap = config_mig_inner.borrow().clone();

                        spawn_async(
                            async move {
                                let res = LifecycleManager::migrate_models(&src_inner, &dst_inner, Some(tx)).await;
                                if res.is_ok() {
                                    let _ = LifecycleManager::apply_systemd_override(&cfg_snap).await;
                                }
                                res
                            },
                            move |res| {
                                is_finished.set(true);
                                btn_done.set_sensitive(true);
                                btn_done.set_label(&t!("settings.btn_migrate"));
                                prog_row_done.set_visible(false);

                                match res {
                                    Ok(msg) => {
                                        let success_dlg = adw::AlertDialog::builder()
                                            .heading(t!("settings.mig_success_title"))
                                            .body(&msg)
                                            .build();
                                        success_dlg.add_response("ok", &t!("settings.btn_ok"));
                                        success_dlg.choose(&page_done, gtk4::gio::Cancellable::NONE, |_| {});

                                        *mig_source_inner.borrow_mut() = None;
                                        mig_row_inner.set_visible(false);

                                        if let Some(info) = LifecycleManager::get_disk_space_for_path(&dst_done) {
                                            let total = info.total_bytes;
                                            let avail = info.available_bytes;
                                            let used = total.saturating_sub(avail);
                                            let ratio = if total > 0 {
                                                (used as f64 / total as f64).clamp(0.0, 1.0)
                                            } else {
                                                0.0
                                            };

                                            disk_bar_inner.set_fraction(ratio);
                                            disk_space_label_inner.set_label(&format!(
                                                "{} free of {} ({:.0}% used)",
                                                format_gib(avail),
                                                format_gib(total),
                                                ratio * 100.0
                                            ));
                                        }
                                    }
                                    Err(err) => {
                                        let err_dlg = adw::AlertDialog::builder()
                                            .heading(t!("settings.mig_error_title"))
                                            .body(t!("settings.mig_error_body", err = err))
                                            .build();
                                        err_dlg.add_response("ok", &t!("settings.btn_close"));
                                        err_dlg.choose(&page_done, gtk4::gio::Cancellable::NONE, |_| {});
                                    }
                                }
                            }
                        );
                    }
                });
            }
        });
    }

    pub fn set_daemon_status(&self, state: &ServiceState, version_opt: Option<&str>) {
        match state {
            ServiceState::Active => {
                let ver_str = version_opt.unwrap_or("unknown");
                let sub = format!("🟢 Active &amp; Connected (v{})", glib::markup_escape_text(ver_str));
                self.daemon_status_row.set_subtitle(&sub);
                self.btn_start.set_sensitive(false);
                self.btn_stop.set_sensitive(true);
                self.btn_restart.set_sensitive(true);
            }
            ServiceState::Inactive => {
                self.daemon_status_row
                    .set_subtitle("🔴 Service Inactive / Disconnected");
                self.btn_start.set_sensitive(true);
                self.btn_stop.set_sensitive(false);
                self.btn_restart.set_sensitive(false);
            }
            ServiceState::Failed => {
                self.daemon_status_row
                    .set_subtitle("⚠️ Service Failed");
                self.btn_start.set_sensitive(true);
                self.btn_stop.set_sensitive(false);
                self.btn_restart.set_sensitive(true);
            }
            ServiceState::NotInstalled => {
                self.daemon_status_row
                    .set_subtitle("❌ Ollama binary not found on system");
                self.btn_start.set_sensitive(false);
                self.btn_stop.set_sensitive(false);
                self.btn_restart.set_sensitive(false);
            }
            ServiceState::Unknown(msg) => {
                let sub = format!("🟡 Unknown state : {}", glib::markup_escape_text(msg));
                self.daemon_status_row.set_subtitle(&sub);
            }
        }
    }

    pub fn set_update_result(&self, res: &UpdateCheckResult) {
        if res.has_update {
            let sub = format!(
                "🎉 New version available : {} (Current: {})",
                glib::markup_escape_text(&res.latest_version),
                glib::markup_escape_text(&res.current_version)
            );
            self.update_row.set_subtitle(&sub);
            self.btn_check_update.set_label("Update Available");
            self.btn_check_update.set_css_classes(&["suggested-action"]);
        } else {
            let sub = format!(
                "✅ Ollama is up to date ({})",
                glib::markup_escape_text(&res.current_version)
            );
            self.update_row.set_subtitle(&sub);
            self.btn_check_update.set_label("Up to date");
            self.btn_check_update.set_css_classes(&[]);
        }
    }

    pub fn refresh_disk_space(&self) {
        let models_dir = self
            .config
            .borrow()
            .models_directory
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                if Path::new("/usr/share/ollama").exists() {
                    PathBuf::from("/usr/share/ollama/.ollama/models")
                } else if let Ok(home) = std::env::var("HOME") {
                    PathBuf::from(home).join(".ollama/models")
                } else {
                    PathBuf::from("/")
                }
            });

        let target = if models_dir.exists() {
            models_dir
        } else {
            models_dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("/"))
        };

        if let Some(info) = LifecycleManager::get_disk_space_for_path(&target) {
            let total = info.total_bytes;
            let avail = info.available_bytes;
            let used = total.saturating_sub(avail);
            let ratio = if total > 0 {
                (used as f64 / total as f64).clamp(0.0, 1.0)
            } else {
                0.0
            };

            self.disk_bar.set_fraction(ratio);
            self.disk_space_label.set_label(&format!(
                "{} free of {} ({:.0}% used)",
                format_gib(avail),
                format_gib(total),
                ratio * 100.0
            ));
        } else {
            self.disk_space_label.set_label("Disk space unavailable");
        }
    }

    pub fn refresh_migration_status(&self) {
        let custom_dir = self.config.borrow().models_directory.as_ref().map(PathBuf::from);
        let Some(target) = custom_dir else {
            *self.migration_source.borrow_mut() = None;
            self.migration_row.set_visible(false);
            return;
        };

        if let Some((src_path, count, size_bytes)) = LifecycleManager::detect_default_models_store(None, Some(&target)) {
            *self.migration_source.borrow_mut() = Some(src_path.clone());

            let size_str = if size_bytes > 0 {
                format!(" (~{})", format_gib(size_bytes))
            } else {
                String::new()
            };
            self.migration_row.set_subtitle(&format!(
                "{} model(s) detected{} in {}",
                count,
                size_str,
                src_path.display()
            ));
            self.migration_row.set_visible(true);
            return;
        }
        *self.migration_source.borrow_mut() = None;
        self.migration_row.set_visible(false);
    }

    pub fn update_profiles<FT, FA, FD, FC, FI>(&self, on_test: FT, on_activate: FA, on_delete: FD, on_copy_ssh: FC, on_inject_systemd: FI)
    where
        FT: Fn(&str) + Clone + 'static,
        FA: Fn(&str) + Clone + 'static,
        FD: Fn(&str) + Clone + 'static,
        FC: Fn(&str) + Clone + 'static,
        FI: Fn(&str) + Clone + 'static,
    {
        for row in self.profile_rows.borrow().iter() {
            self.profiles_card.remove(row.widget());
        }
        self.profile_rows.borrow_mut().clear();

        let cfg = self.config.borrow();
        let active_id = cfg.active_profile_id.clone();

        for profile in &cfg.profiles {
            let is_active = Some(&profile.id) == active_id.as_ref();
            let row = Rc::new(KeyProfileRow::new(profile, is_active));

            let on_test_c = on_test.clone();
            let on_act_c = on_activate.clone();
            let on_del_c = on_delete.clone();
            let on_copy_c = on_copy_ssh.clone();
            let on_inj_c = on_inject_systemd.clone();

            row.connect_test(move |id| on_test_c(id));
            row.connect_activate(move |id| on_act_c(id));
            row.connect_delete(move |id| on_del_c(id));
            row.connect_copy_ssh(move |id| on_copy_c(id));
            row.connect_inject_systemd(move |id| on_inj_c(id));

            self.profiles_card.append(row.widget());
            self.profile_rows.borrow_mut().push(row);
        }
    }

    pub fn set_profile_testing(&self, profile_id: &str, testing: bool) {
        for r in self.profile_rows.borrow().iter() {
            if r.profile_id() == profile_id {
                r.set_testing(testing);
            }
        }
    }

    pub fn set_profile_validation(&self, profile_id: &str, is_valid: bool, message: &str) {
        for r in self.profile_rows.borrow().iter() {
            if r.profile_id() == profile_id {
                r.set_validation_status(is_valid, message);
            }
        }
    }

    pub fn set_active_profile_row(&self, active_id: &str) {
        for r in self.profile_rows.borrow().iter() {
            let is_act = r.profile_id() == active_id;
            r.set_active(is_act);
        }
    }

    pub fn member_key_path_entry(&self) -> adw::EntryRow {
        self.member_key_path_entry.clone()
    }

    pub fn connect_generate_member_key<F: Fn(String) + 'static>(&self, f: F) {
        let name_entry = self.member_name_entry.clone();
        self.btn_generate_member_ssh.connect_clicked(move |_| {
            let name = name_entry.text().to_string();
            f(name.trim().to_string());
        });
    }

    pub fn connect_save_member_profile<F: Fn(String, String) + 'static>(&self, f: F) {
        let name_entry = self.member_name_entry.clone();
        let path_entry = self.member_key_path_entry.clone();
        self.btn_save_member.connect_clicked(move |_| {
            let name = name_entry.text().to_string();
            let key_path = path_entry.text().to_string();
            if !name.trim().is_empty() && !key_path.trim().is_empty() {
                f(name.trim().to_string(), key_path.trim().to_string());
                name_entry.set_text("");
                path_entry.set_text("");
            }
        });
    }

    // Daemon control callbacks
    pub fn connect_start<F: Fn() + 'static>(&self, f: F) {
        self.btn_start.connect_clicked(move |_| f());
    }

    pub fn connect_stop<F: Fn() + 'static>(&self, f: F) {
        self.btn_stop.connect_clicked(move |_| f());
    }

    pub fn connect_restart<F: Fn() + 'static>(&self, f: F) {
        self.btn_restart.connect_clicked(move |_| f());
    }

    pub fn connect_check_updates<F: Fn() + 'static>(&self, f: F) {
        self.btn_check_update.connect_clicked(move |_| f());
    }

    pub fn connect_language_changed<F: Fn(&str) + 'static>(&self, f: F) {
        self.lang_row.connect_selected_notify(move |combo| {
            let selected = combo.selected();
            let new_lang = if selected == 0 {
                "auto"
            } else {
                crate::core::i18n::AVAILABLE_LANGUAGES
                    .get(selected as usize - 1)
                    .map(|l| l.code)
                    .unwrap_or("auto")
            };
            f(new_lang);
        });
    }


    fn sync_config_fields(
        cfg: &Rc<RefCell<AppConfig>>,
        h: &adw::EntryRow,
        o: &adw::EntryRow,
        ec: &adw::ComboRow,
        fa: &adw::SwitchRow,
        cm: &adw::ComboRow,
        ka: &adw::ComboRow,
    ) {
        let mut c = cfg.borrow_mut();
        c.ollama_host = h.text().to_string();
        let orig = o.text().to_string();
        c.ollama_origins = if orig.trim().is_empty() {
            None
        } else {
            Some(orig.trim().to_string())
        };
        c.exposure_mode = match ec.selected() {
            0 => crate::core::config::ExposureMode::LocalHost,
            1 => crate::core::config::ExposureMode::Lan,
            2 => crate::core::config::ExposureMode::Exposed,
            _ => crate::core::config::ExposureMode::LocalHost,
        };
        c.flash_attention = fa.is_active();
        c.keep_alive = match ka.selected() {
            0 => None,
            1 => Some("1m".to_string()),
            2 => Some("5m".to_string()),
            3 => Some("10m".to_string()),
            4 => Some("30m".to_string()),
            5 => Some("1h".to_string()),
            6 => Some("24h".to_string()),
            7 => Some("-1".to_string()),
            _ => None,
        };
        c.daemon_mode = match cm.selected() {
            0 => DaemonMode::SystemdService,
            1 => DaemonMode::SupervisedProcess,
            _ => DaemonMode::RemoteServer,
        };
        let _ = c.save();
    }

    fn check_is_dirty(cfg: &AppConfig, w: &DirtyCheckWidgets) -> bool {
        let current_host = w.host_entry.text().to_string();
        if current_host != cfg.ollama_host {
            return true;
        }
        let current_orig = w.origins_entry.text().to_string();
        let saved_orig = cfg.ollama_origins.as_deref().unwrap_or("");
        if current_orig.trim() != saved_orig.trim() {
            return true;
        }
        let current_exp = match w.exposure_combo.selected() {
            0 => crate::core::config::ExposureMode::LocalHost,
            1 => crate::core::config::ExposureMode::Lan,
            2 => crate::core::config::ExposureMode::Exposed,
            _ => crate::core::config::ExposureMode::LocalHost,
        };
        if current_exp != cfg.exposure_mode {
            return true;
        }
        if w.flash_attn_switch.is_active() != cfg.flash_attention {
            return true;
        }
        let current_ka = match w.keep_alive_combo.selected() {
            0 => None,
            1 => Some("1m".to_string()),
            2 => Some("5m".to_string()),
            3 => Some("10m".to_string()),
            4 => Some("30m".to_string()),
            5 => Some("1h".to_string()),
            6 => Some("24h".to_string()),
            7 => Some("-1".to_string()),
            _ => None,
        };
        if current_ka != cfg.keep_alive {
            return true;
        }
        let current_mode = match w.daemon_combo.selected() {
            0 => DaemonMode::SystemdService,
            1 => DaemonMode::SupervisedProcess,
            _ => DaemonMode::RemoteServer,
        };
        if current_mode != cfg.daemon_mode {
            return true;
        }
        if &cfg.models_directory != w.staged_models_dir {
            return true;
        }
        false
    }

    fn update_dirty_ui(
        is_dirty: &Rc<RefCell<bool>>,
        dirty: bool,
        save_row: &adw::ActionRow,
        save_icon: &gtk4::Image,
        btn_save: &Button,
    ) {
        *is_dirty.borrow_mut() = dirty;
        btn_save.set_sensitive(dirty);
        if dirty {
            btn_save.add_css_class("suggested-action");
            save_icon.set_icon_name(Some("document-save-symbolic"));
            save_row.set_subtitle(&t!("settings.save_row_sub_dirty"));
        } else {
            btn_save.remove_css_class("suggested-action");
            save_icon.set_icon_name(Some("emblem-ok-symbolic"));
            save_row.set_subtitle(&t!("settings.save_row_sub_clean"));
        }
    }

    pub fn has_unsaved_changes(&self) -> bool {
        *self.is_dirty.borrow()
    }

    pub fn save_and_apply(&self) {
        self.btn_save_apply.emit_clicked();
    }

    pub fn mark_clean(&self) {
        Self::update_dirty_ui(
            &self.is_dirty,
            false,
            &self.save_row,
            &self.save_icon,
            &self.btn_save_apply,
        );
    }

    pub fn revert_changes(&self) {
        let cfg = self.config.borrow();
        self.entry_host.set_text(&cfg.ollama_host);
        self.entry_origins.set_text(cfg.ollama_origins.as_deref().unwrap_or(""));
        let exp_idx = match cfg.exposure_mode {
            crate::core::config::ExposureMode::LocalHost => 0,
            crate::core::config::ExposureMode::Lan => 1,
            crate::core::config::ExposureMode::Exposed => 2,
        };
        self.exposure_combo.set_selected(exp_idx);
        self.switch_flash_att.set_active(cfg.flash_attention);
        let ka_idx = match cfg.keep_alive.as_deref() {
            None | Some("") => 0,
            Some("-1") => 7,
            Some("1m") => 1,
            Some("5m") => 2,
            Some("10m") => 3,
            Some("30m") => 4,
            Some("1h") => 5,
            Some("24h") => 6,
            _ => 0,
        };
        self.keep_alive_combo.set_selected(ka_idx);
        let mode_idx = match cfg.daemon_mode {
            DaemonMode::SystemdService => 0,
            DaemonMode::SupervisedProcess => 1,
            _ => 2,
        };
        self.mode_combo.set_selected(mode_idx);

        let saved_dir = self.saved_models_dir.borrow().clone();
        drop(cfg);
        self.config.borrow_mut().models_directory = saved_dir.clone();
        let _ = self.config.borrow().save();
        if let Some(dir) = saved_dir {
            self.storage_row.set_subtitle(&dir);
        }

        Self::update_dirty_ui(
            &self.is_dirty,
            false,
            &self.save_row,
            &self.save_icon,
            &self.btn_save_apply,
        );
    }

    pub fn connect_apply_systemd<F: Fn() + Clone + 'static>(&self, f: F) {
        let cfg = self.config.clone();
        let h = self.entry_host.clone();
        let o = self.entry_origins.clone();
        let ec = self.exposure_combo.clone();
        let fa = self.switch_flash_att.clone();
        let cm = self.mode_combo.clone();
        let ka = self.keep_alive_combo.clone();
        let is_dirty = self.is_dirty.clone();
        let save_row = self.save_row.clone();
        let save_icon = self.save_icon.clone();
        let btn_save = self.btn_save_apply.clone();
        let smd = self.saved_models_dir.clone();

        self.btn_save_apply.connect_clicked(move |_| {
            Self::sync_config_fields(&cfg, &h, &o, &ec, &fa, &cm, &ka);
            *smd.borrow_mut() = cfg.borrow().models_directory.clone();
            Self::update_dirty_ui(&is_dirty, false, &save_row, &save_icon, &btn_save);
            f();
        });
    }

    pub fn connect_save_profile<F: Fn(ApiProfile) + 'static>(&self, f: F) {
        let type_combo = self.new_profile_type_combo.clone();
        let name_entry = self.new_profile_name_entry.clone();
        let token_entry = self.new_profile_token_entry.clone();
        let endpoint_entry = self.new_profile_endpoint_entry.clone();
        let np_card_save = self.new_profile_card.clone();

        self.btn_save_new_profile.connect_clicked(move |_| {
            let name = name_entry.text().to_string();
            let selected_type = type_combo.selected();

            let (provider_type, token, endpoint) = match selected_type {
                0 => {
                    // Ollama Cloud (API Key)
                    let t = token_entry.text().to_string();
                    let ep_str = endpoint_entry.text().to_string();
                    let ep = if ep_str.trim().is_empty() { None } else { Some(ep_str) };
                    (ApiProviderType::OllamaCloud, t, ep)
                }
                1 => {
                    // Hugging Face
                    let t = token_entry.text().to_string();
                    let ep_str = endpoint_entry.text().to_string();
                    let ep = if ep_str.trim().is_empty() { None } else { Some(ep_str) };
                    (ApiProviderType::HuggingFace, t, ep)
                }
                2 => {
                    // Remote Ollama Server
                    let t = token_entry.text().to_string();
                    let ep_str = endpoint_entry.text().to_string();
                    let ep = if ep_str.trim().is_empty() { None } else { Some(ep_str) };
                    (ApiProviderType::OllamaRemote, t, ep)
                }
                _ => {
                    // Custom Cloud / Proxy
                    let t = token_entry.text().to_string();
                    let ep_str = endpoint_entry.text().to_string();
                    let ep = if ep_str.trim().is_empty() { None } else { Some(ep_str) };
                    (ApiProviderType::CustomCloud, t, ep)
                }
            };

            if !name.trim().is_empty() {
                let profile = ApiProfile::new(name, provider_type, token, endpoint);
                f(profile);

                name_entry.set_text("");
                token_entry.set_text("");
                np_card_save.set_visible(false);
            }
        });
    }
}

pub fn show_diagnostic_dialog(parent: &impl IsA<gtk4::Widget>, report: &str) {
    let dialog = adw::Dialog::builder()
        .title(t!("settings.diagnostic_dialog_title"))
        .content_width(640)
        .content_height(520)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    let header_bar = adw::HeaderBar::new();

    let report_copy = report.to_string();
    let btn_copy_modal = Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text(t!("settings.btn_copy_diagnostic_tooltip"))
        .css_classes(["suggested-action"])
        .valign(Align::Center)
        .build();

    let btn_c = btn_copy_modal.clone();
    btn_copy_modal.connect_clicked(move |_| {
        if let Some(display) = gtk4::gdk::Display::default() {
            display.clipboard().set_text(&report_copy);
        }
        btn_c.set_icon_name("object-select-symbolic");
        let btn_reset = btn_c.clone();
        glib::timeout_add_local_once(std::time::Duration::from_secs(2), move || {
            btn_reset.set_icon_name("edit-copy-symbolic");
        });
    });

    header_bar.pack_end(&btn_copy_modal);
    toolbar_view.add_top_bar(&header_bar);

    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .build();

    let text_view = gtk4::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk4::WrapMode::WordChar)
        .monospace(true)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(16)
        .margin_end(16)
        .build();

    text_view.buffer().set_text(report);
    scrolled.set_child(Some(&text_view));
    toolbar_view.set_content(Some(&scrolled));
    dialog.set_child(Some(&toolbar_view));

    dialog.present(Some(parent));
}
