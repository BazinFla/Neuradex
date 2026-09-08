use crate::api::github::GitHubClient;
use crate::api::hub_remote::parse_hf_identifier;
use crate::api::types::{ChatMetrics, ChatMessage, ChatRequest, ChatStreamChunk, ModelPs, ModelTag};
use crate::api::OllamaClient;
use crate::core::config::{ApiProfile, ApiProviderType, AppConfig};
use crate::core::hardware::HardwareMonitor;
use crate::core::lifecycle::LifecycleManager;
use crate::core::spawn_async;
use crate::core::vault::ApiVault;
use crate::core::SecretStore;
use crate::t;
use crate::ui::components::{HfModelPickerDialog, ModelSettingsDialog};
use crate::ui::controllers::ModelController;
use crate::ui::header::Header;
use crate::ui::views::{ChatView, HubView, InstancesView, LogsView, SettingsView};
use adw::prelude::*;
use adw::{Application, ApplicationWindow, Toast, ToastOverlay, ViewStack};
use gtk4::{Box, Orientation};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Shared application state and UI context passed across event handlers and controllers.
#[derive(Clone)]
pub struct AppContext {
    pub window: ApplicationWindow,
    pub view_stack: ViewStack,
    pub toast_overlay: ToastOverlay,
    pub header: Rc<Header>,
    pub instances_view: Rc<InstancesView>,
    pub chat_view: Rc<ChatView>,
    pub hub_view: Rc<HubView>,
    pub logs_view: Rc<LogsView>,
    pub settings_view: Rc<SettingsView>,
    pub client: Rc<RefCell<OllamaClient>>,
    pub config: Rc<RefCell<AppConfig>>,
    pub hw_monitor: Rc<RefCell<HardwareMonitor>>,
    pub last_known_version: Rc<RefCell<Option<String>>>,
    pub active_cancellers: Rc<RefCell<HashMap<String, Arc<AtomicBool>>>>,
}

impl AppContext {
    /// Triggers asynchronous model download with progress feedback
    pub fn pull_model(&self, tag: &str) {
        let ctx = self.clone();
        let hf_token = crate::core::vault::ApiVault::get_hf_token(&self.config.borrow().profiles);
        self.client.borrow_mut().set_hf_token(hf_token);

        ModelController::pull_model(
            self.client.borrow().clone(),
            self.toast_overlay.clone(),
            self.hub_view.clone(),
            self.header.clone(),
            self.active_cancellers.clone(),
            tag,
            move |success| {
                if success {
                    ctx.refresh(false);
                }
            },
        );
    }

    /// Centralized application refresh polling and synchronizing daemon, models, and hardware metrics
    pub fn refresh(&self, manual: bool) {
        if manual {
            self.header.set_refreshing(true);
        }

        let ctx = self.clone();
        let client = self.client.borrow().clone();

        spawn_async(
            async move {
                let sys_state = LifecycleManager::check_systemd_status().await;
                let ver_res = client.get_version().await;
                let tags_res = client.list_tags().await;
                let running_res = client.list_running().await;

                (sys_state, ver_res, tags_res, running_res)
            },
            move |(sys_state, ver_res, tags_res, running_res)| {
                let snap = ctx.hw_monitor.borrow_mut().snapshot();
                ctx.instances_view.update_hardware(&snap);

                match ver_res {
                    Ok(ver) => {
                        *ctx.last_known_version.borrow_mut() = Some(ver.clone());
                        ctx.header.set_status_connected(&ver);
                        ctx.settings_view.set_daemon_status(&sys_state, Some(&ver));

                        let models = tags_res.unwrap_or_default();
                        let running = running_res.unwrap_or_default();

                        ctx.wire_views(&models, &running);

                        let ctx_pull = ctx.clone();
                        ctx.hub_view.update_installed_and_hardware(
                            &models,
                            &snap,
                            move |tag, is_cancel| {
                                if is_cancel {
                                    let map = ctx_pull.active_cancellers.borrow();
                                    if let Some(c) = map.get(&tag).or_else(|| {
                                        map.iter()
                                            .find(|(k, _)| k.starts_with(&tag) || tag.starts_with(*k))
                                            .map(|(_, v)| v)
                                    }) {
                                        c.store(true, Ordering::SeqCst);
                                    }
                                } else {
                                    ctx_pull.pull_model(&tag);
                                }
                            },
                        );
                    }
                    Err(err) => {
                        let err_str = err.to_string();
                        ctx.header.set_status_disconnected(&err_str);
                        ctx.settings_view.set_daemon_status(&sys_state, None);
                    }
                }

                ctx.header.set_refreshing(false);
            },
        );
    }

    /// Background polling loop triggering soft UI syncs
    pub fn start_polling_loop(&self) {
        let ctx = self.clone();
        glib::timeout_add_local(Duration::from_secs(3), move || {
            ctx.refresh(false);
            glib::ControlFlow::Continue
        });
    }

    /// Wires installed and loaded models across views
    pub fn wire_views(&self, models: &[ModelTag], running: &[ModelPs]) {
        let mut running_names = HashSet::new();
        for r in running {
            running_names.insert(r.name.clone());
            if let Some(ref m) = r.model {
                running_names.insert(m.clone());
            }
        }

        let ctx_action = self.clone();
        let ctx_chat = self.clone();
        let ctx_del = self.clone();
        let ctx_settings = self.clone();

        self.instances_view.update_installed_models(
            models,
            &running_names,
            move |name, is_loaded| {
                ctx_action.instances_view.set_model_loading(name, true);
                if is_loaded {
                    ctx_action.toast_overlay.add_toast(Toast::new(&t!("toasts.vram_freeing", name = name)));
                } else {
                    ctx_action.toast_overlay.add_toast(Toast::new(&t!("toasts.loading_vram", name = name)));
                }

                let ctx_refresh = ctx_action.clone();
                let custom_settings = ctx_action.config.borrow().get_model_settings(name).cloned();

                ModelController::load_or_unload_model(
                    ctx_action.client.borrow().clone(),
                    ctx_action.instances_view.clone(),
                    ctx_action.toast_overlay.clone(),
                    name,
                    is_loaded,
                    custom_settings,
                    move |success| {
                        if success {
                            ctx_refresh.refresh(false);
                        }
                    },
                );
            },
            move |name| {
                ctx_chat.view_stack.set_visible_child_name("chat");
                ctx_chat.header.set_view_subtitle("Chat & Playground");
                ctx_chat.chat_view.select_model(name);
            },
            move |name| {
                ModelController::delete_model(
                    ctx_del.client.borrow().clone(),
                    ctx_del.toast_overlay.clone(),
                    name,
                    |_| {},
                );
            },
            move |name, model_size| {
                let ctx_dlg = ctx_settings.clone();
                let client_c = ctx_settings.client.borrow().clone();

                ModelSettingsDialog::show(
                    Some(&ctx_settings.window),
                    name,
                    model_size,
                    &client_c,
                    &ctx_settings.config,
                    &ctx_settings.hw_monitor,
                    move |variant_name, modelfile| {
                        let ctx_res = ctx_dlg.clone();
                        ModelController::create_custom_variant(
                            ctx_dlg.client.borrow().clone(),
                            ctx_dlg.toast_overlay.clone(),
                            &variant_name,
                            &modelfile,
                            move |success| {
                                if success {
                                    ctx_res.refresh(false);
                                }
                            },
                        );
                    },
                );
            },
        );

        let ctx_running = self.clone();
        self.instances_view.update_running(running, move |name| {
            ctx_running.instances_view.set_model_loading(name, true);
            ctx_running.toast_overlay.add_toast(Toast::new(&t!("toasts.vram_freeing", name = name)));

            let ctx_res = ctx_running.clone();
            ModelController::load_or_unload_model(
                ctx_running.client.borrow().clone(),
                ctx_running.instances_view.clone(),
                ctx_running.toast_overlay.clone(),
                name,
                true,
                None,
                move |success| {
                    if success {
                        ctx_res.refresh(false);
                    }
                },
            );
        });

        // Update chat model list (indicators ● / ○, VRAM first, fitness colors)
        let snap_chat = self.hw_monitor.borrow_mut().snapshot();
        self.chat_view.update_models(models, running, &snap_chat);
    }

    /// Synchronizes API and SSH profiles between AppConfig, Header, and SettingsView
    pub fn sync_profiles(&self) {
        let (profiles_list, active_id_opt) = {
            let cfg = self.config.borrow();
            (cfg.profiles.clone(), cfg.active_profile_id.clone())
        };

        self.header.update_profiles(&profiles_list, active_id_opt.as_deref());

        let ctx_test = self.clone();
        let ctx_act = self.clone();
        let ctx_del = self.clone();
        let ctx_copy = self.clone();
        let ctx_inj = self.clone();

        self.settings_view.update_profiles(
            move |id| {
                let cfg = ctx_test.config.borrow();
                if let Some(p) = cfg.profiles.iter().find(|p| p.id == id).cloned() {
                    ctx_test.settings_view.set_profile_testing(id, true);
                    let id_s = id.to_string();
                    let set_v = ctx_test.settings_view.clone();
                    spawn_async(
                        async move {
                            let res = ApiVault::validate_profile(&p).await;
                            (id_s, res.is_valid, res.message)
                        },
                        move |(id_res, is_valid, msg)| {
                            set_v.set_profile_testing(&id_res, false);
                            set_v.set_profile_validation(&id_res, is_valid, &msg);
                        },
                    );
                }
            },
            move |id| {
                let (changed, p_name, p_tok) = {
                    let mut cfg = ctx_act.config.borrow_mut();
                    if cfg.active_profile_id.as_deref() != Some(id) {
                        cfg.set_active_profile(Some(id.to_string()));
                        let _ = cfg.save();
                        let name = cfg.active_profile().map(|p| p.name.clone()).unwrap_or_default();
                        let tok = cfg.active_profile().and_then(|p| {
                            if p.provider_type == ApiProviderType::OllamaSsh {
                                None
                            } else {
                                let t = SecretStore::resolve_token(p);
                                if t.trim().is_empty() {
                                    None
                                } else {
                                    Some(t)
                                }
                            }
                        });
                        (true, name, tok)
                    } else {
                        (false, String::new(), None)
                    }
                };

                if changed {
                    ctx_act.client.borrow_mut().set_api_key(p_tok);
                    ctx_act.header.update_profiles(&ctx_act.config.borrow().profiles, Some(id));
                    ctx_act.settings_view.set_active_profile_row(id);
                    ctx_act.toast_overlay.add_toast(Toast::new(&t!("toasts.profile_active", name = p_name)));

                    let cfg_snap = ctx_act.config.borrow().clone();
                    let t = ctx_act.toast_overlay.clone();
                    let name_clone = p_name.clone();
                    spawn_async(
                        async move { LifecycleManager::apply_systemd_override(&cfg_snap).await },
                        move |res| match res {
                            Ok(_) => t.add_toast(Toast::new(&t!("toasts.systemd_synced", name = name_clone))),
                            Err(e) => t.add_toast(Toast::new(&t!("toasts.systemd_error", err = e))),
                        },
                    );
                }
            },
            move |id| {
                SecretStore::delete_token(id);
                let mut cfg = ctx_del.config.borrow_mut();
                cfg.delete_profile(id);
                let _ = cfg.save();
                drop(cfg);
                ctx_del.toast_overlay.add_toast(Toast::new(&t!("toasts.profile_deleted")));
                ctx_del.sync_profiles();
            },
            move |id| {
                let cfg = ctx_copy.config.borrow();
                if let Some(p) = cfg.profiles.iter().find(|p| p.id == id) {
                    if let Some(pub_key) = ApiVault::get_public_key_for_profile(p) {
                        if let Some(display) = gtk4::gdk::Display::default() {
                            display.clipboard().set_text(&pub_key);
                            ctx_copy.toast_overlay.add_toast(Toast::new(&t!(
                                "toasts.ssh_key_copied",
                                name = p.name
                            )));
                        }
                    } else {
                        ctx_copy.toast_overlay.add_toast(Toast::new(&t!(
                            "toasts.ssh_key_not_found",
                            name = p.name
                        )));
                    }
                }
            },
            move |id| {
                let mut cfg = ctx_inj.config.borrow_mut();
                cfg.set_active_profile(Some(id.to_string()));
                let _ = cfg.save();
                let prof_name = cfg.active_profile().map(|p| p.name.clone()).unwrap_or_else(|| "Profil".to_string());
                let cfg_snap = cfg.clone();
                drop(cfg);

                let t = ctx_inj.toast_overlay.clone();
                let name_clone = prof_name.clone();
                spawn_async(
                    async move { LifecycleManager::apply_systemd_override(&cfg_snap).await },
                    move |res| match res {
                        Ok(_) => t.add_toast(Toast::new(&t!("toasts.key_injected_systemd", name = name_clone))),
                        Err(e) => t.add_toast(Toast::new(&t!("toasts.key_inject_failed", err = e))),
                    },
                );
            },
        );
    }

    /// Sets up global signals, buttons, and user interaction callbacks
    pub fn setup_events(&self) {
        // Profile selection change in Header
        let ctx_hdr = self.clone();
        self.header.connect_profile_changed(move |id| {
            let (changed, p_name, p_tok) = {
                let mut cfg = ctx_hdr.config.borrow_mut();
                if cfg.active_profile_id.as_deref() != Some(id) {
                    cfg.active_profile_id = Some(id.to_string());
                    let _ = cfg.save();
                    let name = cfg.active_profile().map(|p| p.name.clone()).unwrap_or_default();
                    let tok = cfg.active_profile().and_then(|p| {
                        if p.provider_type == ApiProviderType::OllamaSsh {
                            None
                        } else {
                            let t = SecretStore::resolve_token(p);
                            if t.trim().is_empty() {
                                None
                            } else {
                                Some(t)
                            }
                        }
                    });
                    (true, name, tok)
                } else {
                    (false, String::new(), None)
                }
            };

            if changed {
                ctx_hdr.client.borrow_mut().set_api_key(p_tok);
                ctx_hdr.toast_overlay.add_toast(Toast::new(&t!("toasts.profile_active", name = p_name)));
                ctx_hdr.settings_view.set_active_profile_row(id);

                let cfg_snap = ctx_hdr.config.borrow().clone();
                let t = ctx_hdr.toast_overlay.clone();
                let name_clone = p_name.clone();
                spawn_async(
                    async move { LifecycleManager::apply_systemd_override(&cfg_snap).await },
                    move |res| match res {
                        Ok(_) => t.add_toast(Toast::new(&t!("toasts.systemd_synced", name = name_clone))),
                        Err(e) => t.add_toast(Toast::new(&t!("toasts.systemd_error", err = e))),
                    },
                );
            }
        });

        // Daemon controls in SettingsView
        let ctx_start = self.clone();
        self.settings_view.connect_start(move || {
            let t = ctx_start.toast_overlay.clone();
            t.add_toast(Toast::new(&t!("toasts.daemon_starting")));
            spawn_async(
                async move { LifecycleManager::start_service().await },
                move |res| match res {
                    Ok(_) => t.add_toast(Toast::new(&t!("toasts.daemon_started"))),
                    Err(e) => t.add_toast(Toast::new(&t!("toasts.daemon_start_failed", err = e))),
                },
            );
        });

        let ctx_stop = self.clone();
        self.settings_view.connect_stop(move || {
            let t = ctx_stop.toast_overlay.clone();
            t.add_toast(Toast::new(&t!("toasts.daemon_stopping")));
            spawn_async(
                async move { LifecycleManager::stop_service().await },
                move |res| match res {
                    Ok(_) => t.add_toast(Toast::new(&t!("toasts.daemon_stopped"))),
                    Err(e) => t.add_toast(Toast::new(&t!("toasts.daemon_stop_failed", err = e))),
                },
            );
        });

        let ctx_restart = self.clone();
        self.settings_view.connect_restart(move || {
            let t = ctx_restart.toast_overlay.clone();
            t.add_toast(Toast::new(&t!("toasts.daemon_restarting")));
            spawn_async(
                async move { LifecycleManager::restart_service().await },
                move |res| match res {
                    Ok(_) => t.add_toast(Toast::new(&t!("toasts.daemon_restarted"))),
                    Err(e) => t.add_toast(Toast::new(&t!("toasts.daemon_restart_failed", err = e))),
                },
            );
        });

        // Apply Systemd Override configuration
        let ctx_sys = self.clone();
        self.settings_view.connect_apply_systemd(move || {
            let cfg = ctx_sys.config.borrow().clone();
            let t = ctx_sys.toast_overlay.clone();
            t.add_toast(Toast::new(&t!("toasts.sys_applying")));
            spawn_async(
                async move { LifecycleManager::apply_systemd_override(&cfg).await },
                move |res| match res {
                    Ok(_) => t.add_toast(Toast::new(&t!("toasts.sys_applied"))),
                    Err(e) => t.add_toast(Toast::new(&t!("toasts.sys_apply_failed", err = e))),
                },
            );
        });

        // Navigation guard for unsaved Settings changes
        let ctx_nav = self.clone();
        let prev_page = Rc::new(RefCell::new(String::from("instances")));
        let guarding = Rc::new(RefCell::new(false));

        self.view_stack.connect_visible_child_name_notify(move |stack| {
            if *guarding.borrow() {
                return;
            }

            let current_child = stack.visible_child_name().map(|s| s.to_string()).unwrap_or_default();
            let previous = prev_page.borrow().clone();

            if previous == "settings" && current_child != "settings" && ctx_nav.settings_view.has_unsaved_changes() {
                // Intercept navigation and remain on settings page
                *guarding.borrow_mut() = true;
                stack.set_visible_child_name("settings");
                ctx_nav.header.set_view_subtitle(&t!("nav.settings"));
                *guarding.borrow_mut() = false;

                let target_page = current_child.clone();
                let stack_clone = stack.clone();
                let ctx_dlg = ctx_nav.clone();
                let prev_clone = prev_page.clone();
                let guarding_clone = guarding.clone();

                let alert = adw::AlertDialog::builder()
                    .heading(t!("settings.unsaved_dialog_title"))
                    .body(t!("settings.unsaved_dialog_body"))
                    .build();

                alert.add_response("cancel", &t!("settings.dialog_cancel"));
                alert.add_response("discard", &t!("settings.dialog_discard"));
                alert.add_response("save", &t!("settings.dialog_save_apply"));
                alert.set_response_appearance("save", adw::ResponseAppearance::Suggested);
                alert.set_response_appearance("discard", adw::ResponseAppearance::Destructive);

                alert.choose(&ctx_nav.window, gtk4::gio::Cancellable::NONE, move |response| {
                    match response.as_str() {
                        "save" => {
                            ctx_dlg.settings_view.save_and_apply();
                            *prev_clone.borrow_mut() = target_page.clone();
                            *guarding_clone.borrow_mut() = true;
                            stack_clone.set_visible_child_name(&target_page);
                            let subtitle = match target_page.as_str() {
                                "instances" => t!("nav.home"),
                                "chat" => t!("nav.chat"),
                                "hub" => t!("nav.hub"),
                                "logs" => t!("nav.logs"),
                                _ => t!("nav.settings"),
                            };
                            ctx_dlg.header.set_view_subtitle(&subtitle);
                            *guarding_clone.borrow_mut() = false;
                        }
                        "discard" => {
                            ctx_dlg.settings_view.revert_changes();
                            *prev_clone.borrow_mut() = target_page.clone();
                            *guarding_clone.borrow_mut() = true;
                            stack_clone.set_visible_child_name(&target_page);
                            let subtitle = match target_page.as_str() {
                                "instances" => t!("nav.home"),
                                "chat" => t!("nav.chat"),
                                "hub" => t!("nav.hub"),
                                "logs" => t!("nav.logs"),
                                _ => t!("nav.settings"),
                            };
                            ctx_dlg.header.set_view_subtitle(&subtitle);
                            *guarding_clone.borrow_mut() = false;
                        }
                        _ => {
                            // "cancel": Stay on settings view, keep unsaved changes
                        }
                    }
                });
            } else {
                *prev_page.borrow_mut() = current_child;
            }
        });

        // Check for Ollama updates
        let ctx_upd = self.clone();
        self.settings_view.connect_check_updates(move || {
            let t = ctx_upd.toast_overlay.clone();
            let s = ctx_upd.settings_view.clone();
            let cur_ver = ctx_upd.last_known_version.borrow().clone().unwrap_or_else(|| "0.0.0".to_string());
            t.add_toast(Toast::new(&t!("toasts.checking_updates")));

            spawn_async(
                async move {
                    GitHubClient::check_latest_release(&cur_ver).await
                },
                move |res| match res {
                    Ok(info) => {
                        s.set_update_result(&info);
                        if info.has_update {
                            t.add_toast(Toast::new(&t!("toasts.update_available", ver = info.latest_version)));
                        } else {
                            t.add_toast(Toast::new(&t!("toasts.already_latest")));
                        }
                    }
                    Err(e) => {
                        t.add_toast(Toast::new(&t!("toasts.check_failed", err = e)));
                    }
                },
            );
        });

        // Language change in Settings
        let ctx_lang = self.clone();
        self.settings_view.connect_language_changed(move |lang| {
            let msg = if lang == "auto" {
                t!("settings.lang_auto_toast")
            } else {
                t!("settings.lang_restart_hint")
            };
            ctx_lang.toast_overlay.add_toast(Toast::new(&msg));
        });


        // Save / Add profile
        let ctx_save = self.clone();
        self.settings_view.connect_save_profile(move |mut profile| {
            let mut cfg = ctx_save.config.borrow_mut();
            let existing = cfg.profiles.iter().find(|p| p.id == profile.id).cloned();
            let is_ssh = profile.provider_type == ApiProviderType::OllamaSsh;

            if let Some(ref existing) = existing {
                if profile.token.trim().is_empty() {
                    profile.token_in_keyring = existing.token_in_keyring;
                    if !existing.token_in_keyring {
                        profile.token = existing.token.clone();
                    }
                }
            }

            let mut keyring_warn = None;
            if !is_ssh && !profile.token.trim().is_empty() {
                match SecretStore::store_token(&profile.id, profile.token.trim()) {
                    Ok(()) => {
                        profile.token_in_keyring = true;
                    }
                    Err(e) => {
                        profile.token_in_keyring = false;
                        tracing::warn!("Failed to store token in keyring: {}. Fallback to config.json", e);
                        keyring_warn = Some(e);
                    }
                }
            }

            cfg.add_or_update_profile(profile.clone());

            let _ = cfg.save();
            drop(cfg);
            ctx_save.toast_overlay.add_toast(Toast::new(&t!("toasts.profile_saved", name = profile.name)));
            if let Some(err) = keyring_warn {
                ctx_save.toast_overlay.add_toast(Toast::new(&format!("Keyring: {} (fallback local)", err)));
            }
            ctx_save.sync_profiles();
        });

        // Automatically generate SSH key for a member / profile
        let ctx_gen = self.clone();
        let path_entry_set = self.settings_view.member_key_path_entry();
        self.settings_view.connect_generate_member_key(move |member_name| {
            let t = ctx_gen.toast_overlay.clone();
            if member_name.is_empty() {
                t.add_toast(Toast::new(&t!("toasts.member_name_required")));
                return;
            }

            match ApiVault::generate_ssh_key_pair(&member_name) {
                Ok(key_info) => {
                    let priv_path = key_info.private_key_path.to_string_lossy().to_string();
                    path_entry_set.set_text(&priv_path);

                    // Copy public key to clipboard
                    if let Some(display) = gtk4::gdk::Display::default() {
                        display.clipboard().set_text(key_info.public_key_content.as_str());
                    }

                    t.add_toast(Toast::new(&t!(
                        "toasts.ssh_key_generated",
                        name = member_name
                    )));
                }
                Err(e) => {
                    t.add_toast(Toast::new(&t!("toasts.ssh_gen_failed", err = e)));
                }
            }
        });

        // Save and activate SSH member profile
        let ctx_mem = self.clone();
        self.settings_view.connect_save_member_profile(move |member_name, key_path| {
            let t = ctx_mem.toast_overlay.clone();
            let pub_path = PathBuf::from(format!("{}.pub", key_path.trim()));
            let pub_content = std::fs::read_to_string(&pub_path)
                .map(|s| ApiVault::clean_public_key(&s))
                .unwrap_or_default();

            let profile = ApiProfile::new(
                member_name.clone(),
                ApiProviderType::OllamaSsh,
                key_path,
                None,
            );

            let mut cfg = ctx_mem.config.borrow_mut();
            cfg.add_or_update_profile(profile.clone());
            cfg.set_active_profile(Some(profile.id.clone()));
            let _ = cfg.save();
            drop(cfg);

            if !pub_content.is_empty() {
                if let Some(display) = gtk4::gdk::Display::default() {
                    display.clipboard().set_text(pub_content.as_str());
                }
            }

            t.add_toast(Toast::new(&t!(
                "toasts.member_saved_activated",
                name = member_name
            )));

            ctx_mem.sync_profiles();

            // Immediately sync with systemd
            let cfg_snap = ctx_mem.config.borrow().clone();
            let t_sub = t.clone();
            let name_clone = member_name.clone();
            spawn_async(
                async move { LifecycleManager::apply_systemd_override(&cfg_snap).await },
                move |res| match res {
                    Ok(_) => t_sub.add_toast(Toast::new(&t!("toasts.systemd_synced", name = name_clone))),
                    Err(e) => t_sub.add_toast(Toast::new(&t!("toasts.systemd_error", err = e))),
                },
            );
        });

        // Click on "Flush All" in InstancesView
        let ctx_un_all = self.clone();
        self.instances_view.connect_unload_all(move || {
            let ctx = ctx_un_all.clone();
            let client = ctx.client.borrow().clone();

            ctx.toast_overlay.add_toast(Toast::new(&t!("toasts.unloading_all")));
            ctx.instances_view.set_unloading_all(true);

            spawn_async(
                async move {
                    let running = client.list_running().await.unwrap_or_default();
                    let mut unloads = 0;
                    for m in running {
                        if client.unload_model(&m.name).await.is_ok() {
                            unloads += 1;
                        }
                    }
                    unloads
                },
                move |unloaded_count| {
                    ctx.instances_view.set_unloading_all(false);
                    if unloaded_count > 0 {
                        ctx.toast_overlay.add_toast(Toast::new(&t!(
                            "toasts.models_unloaded_count",
                            count = unloaded_count
                        )));
                    } else {
                        ctx.toast_overlay.add_toast(Toast::new(&t!("toasts.no_model_was_loaded")));
                    }
                    ctx.refresh(false);
                },
            );
        });

        // Smart custom download from Hub search entry
        let ctx_hub_inspect = self.clone();
        self.hub_view.connect_custom_pull(move |input_str| {
            let ctx = ctx_hub_inspect.clone();

            if let Some((repo_id, tag_opt)) = parse_hf_identifier(&input_str) {
                // If a specific tag is already provided (e.g. hf.co/...:Q8_0 or user/repo:Q4_K_M), download directly
                if let Some(tag) = tag_opt {
                    let full_tag = format!("hf.co/{}:{}", repo_id, tag);
                    ctx.pull_model(&full_tag);
                    return;
                }

                ctx.toast_overlay.add_toast(Toast::new(&t!("toasts.inspecting_hf", name = repo_id)));

                let hf_token = ApiVault::get_hf_token(&ctx.config.borrow().profiles);
                let repo_id_async = repo_id.clone();
                let ctx_cb = ctx.clone();

                spawn_async(
                    async move {
                        let web_client = crate::api::OllamaWebClient::new();
                        web_client.fetch_hf_repo_details(&repo_id_async, hf_token.as_deref()).await
                    },
                    move |res| {
                        match res {
                            Ok(details) => {
                                let snap = ctx_cb.hw_monitor.borrow_mut().snapshot();
                                let installed = ctx_cb.hub_view.installed_names();
                                let ctx_pull = ctx_cb.clone();
                                HfModelPickerDialog::show(
                                    Some(&ctx_cb.window),
                                    details,
                                    &snap,
                                    &installed,
                                    move |chosen_tag| {
                                        ctx_pull.pull_model(&chosen_tag);
                                    },
                                );
                            }
                            Err(e) => {
                                ctx_cb.toast_overlay.add_toast(Toast::new(&t!("toasts.hf_fallback_standard", err = e)));
                                ctx_cb.pull_model(&format!("hf.co/{}", repo_id));
                            }
                        }
                    },
                );
            } else {
                // Standard official Ollama model (e.g. llama3.2:3b, mistral)
                ctx.pull_model(&input_str);
            }
        });

        // Click on Refresh Hub catalog
        let ctx_cat_refresh = self.clone();
        self.hub_view.connect_refresh_catalog(move || {
            let ctx = ctx_cat_refresh.clone();

            ctx.hub_view.set_syncing(true);
            ctx.toast_overlay.add_toast(Toast::new(&t!("toasts.catalog_refreshing")));

            spawn_async(
                async move {
                    let cache_path = crate::core::hub::HubManager::cache_path();
                    let _ = std::fs::remove_file(&cache_path);

                    let web_client = crate::api::OllamaWebClient::new();
                    let res = web_client.fetch_library_catalog().await;
                    if let Ok(ref models) = res {
                        crate::core::hub::HubManager::save_cache(models);
                    }
                    res
                },
                move |res| {
                    ctx.hub_view.set_syncing(false);
                    match res {
                        Ok(models) => {
                            let count = models.len();
                            let snap = ctx.hw_monitor.borrow_mut().snapshot();
                            let ctx_pull = ctx.clone();
                            ctx.hub_view.reset_catalog_loaded();
                            ctx.hub_view.populate_catalog_list(models, &snap, move |tag, is_cancel| {
                                if is_cancel {
                                    let map = ctx_pull.active_cancellers.borrow();
                                    if let Some(c) = map.get(&tag).or_else(|| map.iter().find(|(k, _)| k.starts_with(&tag) || tag.starts_with(*k)).map(|(_, v)| v)) {
                                        c.store(true, Ordering::SeqCst);
                                    }
                                } else {
                                    ctx_pull.pull_model(&tag);
                                }
                            });
                            ctx.toast_overlay.add_toast(Toast::new(&t!("toasts.catalog_refreshed", count = count)));
                        }
                        Err(e) => {
                            ctx.toast_overlay.add_toast(Toast::new(&t!("toasts.catalog_refresh_failed", err = e)));
                        }
                    }
                },
            );
        });

        // Change catalog sort criterion
        let ctx_sort = self.clone();
        self.hub_view.connect_sort_changed(move |criterion| {
            let mut raw = ctx_sort.hub_view.raw_catalog();
            if !raw.is_empty() {
                crate::core::hub::HubManager::sort_models(&mut raw, criterion);
                let snap = ctx_sort.hw_monitor.borrow_mut().snapshot();
                let ctx_pull = ctx_sort.clone();

                ctx_sort.hub_view.populate_catalog_list(raw, &snap, move |tag, is_cancel| {
                    if is_cancel {
                        let map = ctx_pull.active_cancellers.borrow();
                        if let Some(c) = map.get(&tag).or_else(|| map.iter().find(|(k, _)| k.starts_with(&tag) || tag.starts_with(*k)).map(|(_, v)| v)) {
                            c.store(true, Ordering::SeqCst);
                        }
                    } else {
                        ctx_pull.pull_model(&tag);
                    }
                });
            }
        });

        // Chat & Playground events
        let ctx_chat = self.clone();
        self.chat_view.connect_send(move |model_name, messages, system_prompt, temperature, num_ctx| {
            let client = ctx_chat.client.borrow().clone();
            let cv = ctx_chat.chat_view.clone();
            let cancelled = Arc::new(AtomicBool::new(false));
            ctx_chat.active_cancellers.borrow_mut().insert(format!("chat:{}", model_name), cancelled.clone());

            let mut options_map = serde_json::Map::new();

            // 1. Retrieve saved custom settings for this model (AppConfig)
            let custom_settings = ctx_chat.config.borrow().get_model_settings(&model_name).cloned();
            if let Some(ref cs) = custom_settings {
                if let serde_json::Value::Object(map) = cs.to_options_map() {
                    options_map = map;
                }
            }

            // 2. Apply direct overrides from chat / popover
            if let Some(t) = temperature {
                options_map.insert("temperature".to_string(), serde_json::json!(t));
            }
            if let Some(c) = num_ctx {
                options_map.insert("num_ctx".to_string(), serde_json::json!(c));
            }

            let final_system_prompt = system_prompt
                .filter(|s| !s.trim().is_empty())
                .or_else(|| custom_settings.as_ref().and_then(|s| s.system_prompt.clone()));

            let keep_alive_val = custom_settings
                .as_ref()
                .and_then(|s| s.keep_alive.clone())
                .unwrap_or_else(|| "15m".to_string());

            let mut req = ChatRequest {
                model: model_name.clone(),
                messages,
                stream: true,
                options: if options_map.is_empty() { None } else { Some(serde_json::Value::Object(options_map)) },
                keep_alive: if keep_alive_val == "-1" {
                    Some(serde_json::json!(-1))
                } else {
                    Some(serde_json::json!(keep_alive_val))
                },
            };

            if let Some(sp) = final_system_prompt {
                if !sp.trim().is_empty() {
                    req.messages.insert(0, ChatMessage::system(sp));
                }
            }

            let (tx, rx) = async_channel::unbounded::<ChatStreamChunk>();
            let cv_stream = cv.clone();
            let hw_stream = ctx_chat.hw_monitor.clone();
            let start_time = Instant::now();
            let first_token_time: Rc<RefCell<Option<Instant>>> = Rc::new(RefCell::new(None));

            glib::timeout_add_local(Duration::from_millis(50), move || {
                let mut chunk_count = 0;
                loop {
                    match rx.try_recv() {
                        Ok(chunk) => {
                            chunk_count += 1;
                            if let Some(err) = chunk.error {
                                cv_stream.handle_stream_error(&err);
                                return glib::ControlFlow::Break;
                            }

                            if let Some(msg) = chunk.message {
                                if !msg.content.is_empty() {
                                    if first_token_time.borrow().is_none() {
                                        *first_token_time.borrow_mut() = Some(Instant::now());
                                    }

                                    let elapsed = start_time.elapsed().as_secs_f64();
                                    let realtime_tok_sec = if elapsed > 0.0 {
                                        Some(chunk_count as f64 / elapsed)
                                    } else {
                                        None
                                    };

                                    let ttft = first_token_time.borrow().map(|t| t.duration_since(start_time).as_secs_f64() * 1000.0);
                                    cv_stream.handle_stream_chunk(&msg.content, realtime_tok_sec, ttft);
                                }
                            }

                            if chunk.done {
                                let total_dur = chunk.total_duration.unwrap_or(0);
                                let eval_dur = chunk.eval_duration.unwrap_or(0);
                                let prompt_eval_dur = chunk.prompt_eval_duration.unwrap_or(0);
                                let eval_count = chunk.eval_count.unwrap_or(0);
                                let prompt_eval_count = chunk.prompt_eval_count.unwrap_or(0);

                                let tokens_per_sec = if eval_dur > 0 {
                                    eval_count as f64 / (eval_dur as f64 / 1_000_000_000.0)
                                } else {
                                    0.0
                                };

                                let ttft_ms = if prompt_eval_dur > 0 {
                                    prompt_eval_dur as f64 / 1_000_000.0
                                } else {
                                    match *first_token_time.borrow() {
                                        Some(first_instant) => first_instant.duration_since(start_time).as_secs_f64() * 1000.0,
                                        None => 0.0,
                                    }
                                };

                                let vram_bytes = {
                                    let snap = hw_stream.borrow_mut().snapshot();
                                    snap.gpu.as_ref().map(|g| g.vram_used).unwrap_or(0)
                                };

                                let metrics = ChatMetrics {
                                    total_duration_ms: total_dur as f64 / 1_000_000.0,
                                    ttft_ms,
                                    eval_count,
                                    prompt_eval_count,
                                    tokens_per_sec,
                                    vram_bytes,
                                };

                                cv_stream.handle_stream_done(&metrics);
                                return glib::ControlFlow::Break;
                            }
                        }
                        Err(async_channel::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                        Err(async_channel::TryRecvError::Closed) => return glib::ControlFlow::Break,
                    }
                }
            });

            let cancel_for_task = cancelled.clone();
            let tx_err = tx.clone();

            crate::core::runtime().spawn(async move {
                let res = client.chat_stream(req, move |chunk| {
                    if cancel_for_task.load(Ordering::SeqCst) {
                        return false;
                    }
                    let is_done = chunk.done;
                    let _ = tx.send_blocking(chunk);
                    !is_done
                }).await;

                if let Err(e) = res {
                    let _ = tx_err.send(ChatStreamChunk {
                        error: Some(e.to_string()),
                        done: true,
                        ..Default::default()
                    }).await;
                }
            });
        });

        let ctx_cancel = self.clone();
        self.chat_view.connect_cancel(move || {
            let model_opt = ctx_cancel.chat_view.selected_model_name();
            if let Some(m) = model_opt {
                let map = ctx_cancel.active_cancellers.borrow();
                if let Some(c) = map.get(&format!("chat:{}", m)) {
                    c.store(true, Ordering::SeqCst);
                }
            }
        });

        self.chat_view.connect_clear();
    }
}

pub struct MainWindow {
    ctx: AppContext,
}

impl MainWindow {
    pub fn new(app: &Application) -> Self {
        let config = Rc::new(RefCell::new(AppConfig::load()));

        // Migrate legacy plaintext tokens from config.json to native keyring if available
        let migrated = SecretStore::migrate_from_config(&mut config.borrow_mut().profiles);
        if migrated > 0 {
            let _ = config.borrow().save();
        }

        let mut client_inst = OllamaClient::new(format!("http://{}", config.borrow().ollama_host));
        let hf_tok = crate::core::vault::ApiVault::get_hf_token(&config.borrow().profiles);
        client_inst.set_hf_token(hf_tok);
        if let Some(active_p) = config.borrow().active_profile() {
            if active_p.provider_type != ApiProviderType::OllamaSsh {
                let tok = SecretStore::resolve_token(active_p);
                if !tok.trim().is_empty() {
                    client_inst.set_api_key(Some(tok));
                }
            }
        }
        let client = Rc::new(RefCell::new(client_inst));
        let hw_monitor = Rc::new(RefCell::new(HardwareMonitor::new()));
        let active_cancellers = Rc::new(RefCell::new(HashMap::new()));

        let window = ApplicationWindow::builder()
            .application(app)
            .title("NeuraDex")
            .icon_name("io.github.bazinfla.NeuraDex")
            .default_width(1180)
            .default_height(780)
            .build();

        let toast_overlay = ToastOverlay::new();
        let view_stack = ViewStack::new();

        let header = Rc::new(Header::new());
        let instances_view = Rc::new(InstancesView::new());
        let chat_view = Rc::new(ChatView::new(config.clone(), client.clone()));
        let hub_view = Rc::new(HubView::new());
        let logs_view = Rc::new(LogsView::new());
        let settings_view = Rc::new(SettingsView::new(config.clone()));

        view_stack.add_titled(instances_view.widget(), Some("instances"), &t!("nav.instances"));
        view_stack.add_titled(chat_view.widget(), Some("chat"), &t!("nav.chat"));
        view_stack.add_titled(hub_view.widget(), Some("hub"), &t!("nav.hub"));
        view_stack.add_titled(logs_view.widget(), Some("logs"), &t!("nav.logs"));
        view_stack.add_titled(settings_view.widget(), Some("settings"), &t!("nav.settings"));

        let main_box = Box::new(Orientation::Vertical, 0);
        main_box.append(header.widget());
        main_box.append(&view_stack);
        toast_overlay.set_child(Some(&main_box));
        window.set_content(Some(&toast_overlay));

        Header::setup_actions(&window, &view_stack, &header);

        let ctx = AppContext {
            window,
            view_stack,
            toast_overlay,
            header,
            instances_view,
            chat_view,
            hub_view,
            logs_view,
            settings_view,
            client,
            config,
            hw_monitor,
            last_known_version: Rc::new(RefCell::new(None)),
            active_cancellers,
        };

        ctx.setup_events();
        ctx.sync_profiles();
        ctx.refresh(false);
        ctx.start_polling_loop();

        Self { ctx }
    }

    pub fn present(&self) {
        self.ctx.window.present();
    }
}
