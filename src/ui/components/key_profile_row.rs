use crate::core::config::{ApiProfile, ApiProviderType};
use crate::core::vault::ApiVault;
use crate::t;
use adw::prelude::*;
use gtk4::{Box, Button, Orientation};

pub struct KeyProfileRow {
    row: adw::ActionRow,
    btn_test: Option<Button>,
    btn_inject_systemd: Option<Button>,
    btn_copy_ssh: Option<Button>,
    btn_activate: Button,
    btn_delete: Button,
    profile_id: String,
}

impl KeyProfileRow {
    pub fn new(profile: &ApiProfile, is_active: bool) -> Self {
        let escaped_name = glib::markup_escape_text(&profile.name);
        let row = adw::ActionRow::builder()
            .title(escaped_name.as_str())
            .build();

        // Subtitle with provider type and masked token
        let masked_token = if profile.provider_type == ApiProviderType::OllamaSsh
            || profile.token.starts_with('/')
            || profile.token.starts_with('~')
        {
            let home_str = std::env::var("HOME").unwrap_or_default();
            profile.token.replace(&home_str, "~")
        } else if profile.token_in_keyring {
            "•••••••• (🔐 Keyring)".to_string()
        } else if profile.token.len() > 12 {
            format!("{}••••{} (🔓 Local)", &profile.token[..4], &profile.token[profile.token.len() - 4..])
        } else if !profile.token.is_empty() {
            "•••••••• (🔓 Local)".to_string()
        } else {
            t!("key_profile.not_configured")
        };

        row.set_subtitle(&format!("{} • {}", profile.provider_type.label(), masked_token));

        // Prefix: Icon
        let icon = gtk4::Image::from_icon_name(profile.provider_type.icon_name());
        icon.set_pixel_size(24);
        row.add_prefix(&icon);

        // Actions on the right
        let actions_box = Box::new(Orientation::Horizontal, 6);
        actions_box.set_valign(gtk4::Align::Center);

        let mut btn_test = None;
        let mut btn_inject_systemd = None;
        let mut btn_copy_ssh = None;

        if profile.provider_type == ApiProviderType::OllamaSsh {
            // Copy SSH public key button
            let btn_copy = Button::builder()
                .icon_name("edit-copy-symbolic")
                .tooltip_text(t!("key_profile.btn_copy_ssh_tooltip"))
                .css_classes(["flat"])
                .build();

            let prof_clone = profile.clone();
            let btn_c = btn_copy.clone();
            btn_copy.connect_clicked(move |_| {
                if let Some(clean_key) = ApiVault::get_public_key_for_profile(&prof_clone) {
                    if let Some(display) = gtk4::gdk::Display::default() {
                        display.clipboard().set_text(&clean_key);
                        btn_c.set_icon_name("object-select-symbolic");
                        btn_c.set_tooltip_text(Some(&t!("key_profile.ssh_copied")));
                        let b = btn_c.clone();
                        glib::timeout_add_seconds_local(2, move || {
                            b.set_icon_name("edit-copy-symbolic");
                            b.set_tooltip_text(Some(&t!("key_profile.btn_copy_ssh_tooltip")));
                            glib::ControlFlow::Break
                        });
                    }
                }
            });
            actions_box.append(&btn_copy);
            btn_copy_ssh = Some(btn_copy);

            // Link button to https://ollama.com/settings/keys
            let btn_open_keys = Button::builder()
                .icon_name("web-browser-symbolic")
                .tooltip_text(t!("key_profile.open_keys_tooltip"))
                .css_classes(["flat"])
                .build();
            btn_open_keys.connect_clicked(move |_| {
                let _ = gtk4::gio::AppInfo::launch_default_for_uri(
                    "https://ollama.com/settings/keys",
                    None::<&gtk4::gio::AppLaunchContext>,
                );
            });
            actions_box.append(&btn_open_keys);
        } else {
            // Test button (color changes based on status)
            let btn_t = Button::builder()
                .icon_name("network-transmit-receive-symbolic")
                .build();

            match profile.is_valid {
                Some(true) => {
                    btn_t.set_css_classes(&["flat", "key-test-btn", "valid"]);
                    btn_t.set_tooltip_text(Some(&t!("key_profile.btn_test_valid")));
                }
                Some(false) => {
                    btn_t.set_css_classes(&["flat", "key-test-btn", "invalid"]);
                    btn_t.set_tooltip_text(Some(&t!("key_profile.btn_test_invalid")));
                }
                None => {
                    btn_t.set_css_classes(&["flat", "key-test-btn", "default"]);
                    btn_t.set_tooltip_text(Some(&t!("key_profile.btn_test_default")));
                }
            }
            actions_box.append(&btn_t);
            btn_test = Some(btn_t);

            // Inject into systemd button (for Ollama Cloud and remote API keys)
            if profile.provider_type == ApiProviderType::OllamaCloud || profile.provider_type == ApiProviderType::CustomCloud {
                let btn_inj = Button::builder()
                    .icon_name("system-run-symbolic")
                    .tooltip_text(t!("key_profile.btn_inject_tooltip"))
                    .css_classes(["flat"])
                    .build();
                actions_box.append(&btn_inj);
                btn_inject_systemd = Some(btn_inj);
            }
        }

        // Activate / Active button (only shown for Ollama identities)
        let btn_activate = Button::builder().build();
        if profile.provider_type.is_ollama_identity() {
            if is_active {
                btn_activate.set_label(&t!("key_profile.btn_active"));
                btn_activate.set_css_classes(&["suggested-action"]);
                btn_activate.set_sensitive(false);
            } else {
                btn_activate.set_label(&t!("key_profile.btn_activate"));
                btn_activate.set_css_classes(&[]);
                btn_activate.set_tooltip_text(Some(&t!("key_profile.btn_activate_tooltip")));
            }
            actions_box.append(&btn_activate);
        } else {
            btn_activate.set_visible(false);
        }

        // Delete button
        let btn_delete = Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text(t!("key_profile.btn_delete_tooltip"))
            .css_classes(["destructive-action", "flat", "circular"])
            .build();
        actions_box.append(&btn_delete);

        row.add_suffix(&actions_box);

        Self {
            row,
            btn_test,
            btn_inject_systemd,
            btn_copy_ssh,
            btn_activate,
            btn_delete,
            profile_id: profile.id.clone(),
        }
    }

    pub fn widget(&self) -> &adw::ActionRow {
        &self.row
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub fn set_active(&self, is_active: bool) {
        if !self.btn_activate.is_visible() {
            return;
        }
        if is_active {
            self.btn_activate.set_label(&t!("key_profile.btn_active"));
            self.btn_activate.set_css_classes(&["suggested-action"]);
            self.btn_activate.set_sensitive(false);
            self.btn_activate.set_tooltip_text(None);
        } else {
            self.btn_activate.set_label(&t!("key_profile.btn_activate"));
            self.btn_activate.set_css_classes(&[]);
            self.btn_activate.set_sensitive(true);
            self.btn_activate.set_tooltip_text(Some(&t!("key_profile.btn_activate_tooltip")));
        }
    }

    pub fn set_testing(&self, testing: bool) {
        if let Some(ref btn) = self.btn_test {
            btn.set_sensitive(!testing);
            if testing {
                btn.set_css_classes(&["flat", "key-test-btn", "default"]);
                btn.set_tooltip_text(Some(&t!("key_profile.btn_testing")));
            }
        }
    }

    pub fn set_validation_status(&self, is_valid: bool, message: &str) {
        if let Some(ref btn) = self.btn_test {
            if is_valid {
                btn.set_css_classes(&["flat", "key-test-btn", "valid"]);
                btn.set_tooltip_text(Some(&t!("key_profile.test_valid_msg", msg = message)));
            } else {
                btn.set_css_classes(&["flat", "key-test-btn", "invalid"]);
                btn.set_tooltip_text(Some(&t!("key_profile.test_invalid_msg", msg = message)));
            }
        }
        self.row.set_tooltip_text(Some(message));
    }

    pub fn connect_test<F: Fn(&str) + 'static>(&self, f: F) {
        if let Some(ref btn) = self.btn_test {
            let id = self.profile_id.clone();
            btn.connect_clicked(move |_| {
                f(&id);
            });
        }
    }

    pub fn connect_inject_systemd<F: Fn(&str) + 'static>(&self, f: F) {
        if let Some(ref btn) = self.btn_inject_systemd {
            let id = self.profile_id.clone();
            btn.connect_clicked(move |_| {
                f(&id);
            });
        }
    }

    pub fn connect_copy_ssh<F: Fn(&str) + 'static>(&self, f: F) {
        if let Some(ref btn) = self.btn_copy_ssh {
            let id = self.profile_id.clone();
            btn.connect_clicked(move |_| {
                f(&id);
            });
        }
    }

    pub fn connect_activate<F: Fn(&str) + 'static>(&self, f: F) {
        let id = self.profile_id.clone();
        self.btn_activate.connect_clicked(move |_| {
            f(&id);
        });
    }

    pub fn connect_delete<F: Fn(&str) + 'static>(&self, f: F) {
        let id = self.profile_id.clone();
        self.btn_delete.connect_clicked(move |_| {
            f(&id);
        });
    }
}
