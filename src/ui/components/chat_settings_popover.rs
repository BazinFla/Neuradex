use crate::api::client::OllamaClient;
use crate::core::config::AppConfig;
use crate::t;
use adw::prelude::*;
use gtk4::{
    Align, Box, Label, Orientation, Popover, Scale, SpinButton, TextBuffer, TextView,
    WrapMode,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Sub-component managing the Chat inference settings popover
/// (System Prompt, Temperature, Context window with synchronized Slider & SpinButton)
pub struct ChatSettingsPopover {
    popover: Popover,
    system_prompt_buffer: TextBuffer,
    temp_scale: Scale,
    ctx_header_label: Label,
    ctx_scale: Scale,
    ctx_spin: SpinButton,
}

impl Default for ChatSettingsPopover {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatSettingsPopover {
    pub fn new() -> Self {
        let popover = Popover::builder()
            .autohide(true)
            .build();

        let settings_box = Box::new(Orientation::Vertical, 12);
        settings_box.set_margin_start(16);
        settings_box.set_margin_end(16);
        settings_box.set_margin_top(16);
        settings_box.set_margin_bottom(16);
        settings_box.set_width_request(320);

        let settings_title = Label::builder()
            .label(t!("chat_settings.title"))
            .css_classes(["heading"])
            .halign(Align::Start)
            .build();
        settings_box.append(&settings_title);

        // 1. System Prompt
        let sys_label = Label::builder()
            .label(t!("chat_settings.system_prompt"))
            .css_classes(["caption", "dim-label"])
            .halign(Align::Start)
            .build();
        settings_box.append(&sys_label);

        let system_prompt_buffer = TextBuffer::new(None);
        let sys_view = TextView::builder()
            .buffer(&system_prompt_buffer)
            .wrap_mode(WrapMode::Word)
            .height_request(80)
            .css_classes(["card"])
            .tooltip_text(t!("chat_settings.system_prompt_tooltip"))
            .build();
        settings_box.append(&sys_view);

        // 2. Temperature
        let temp_header_box = Box::new(Orientation::Horizontal, 6);
        let temp_label = Label::builder()
            .label(t!("chat_settings.temperature"))
            .css_classes(["caption", "dim-label"])
            .halign(Align::Start)
            .hexpand(true)
            .build();
        let temp_val_label = Label::builder()
            .label("0.70")
            .css_classes(["caption", "numeric"])
            .build();
        temp_header_box.append(&temp_label);
        temp_header_box.append(&temp_val_label);
        settings_box.append(&temp_header_box);

        let temp_scale = Scale::with_range(Orientation::Horizontal, 0.0, 1.5, 0.05);
        temp_scale.set_value(0.70);
        let val_lbl_clone = temp_val_label.clone();
        temp_scale.connect_value_changed(move |s| {
            val_lbl_clone.set_label(&format!("{:.2}", s.value()));
        });
        settings_box.append(&temp_scale);

        // 3. Context Window (num_ctx): Slider + SpinButton
        let ctx_header_box = Box::new(Orientation::Horizontal, 6);
        let ctx_header_label = Label::builder()
            .label(t!("chat_settings.context_window", max = "128k"))
            .css_classes(["caption", "dim-label"])
            .halign(Align::Start)
            .hexpand(true)
            .build();
        let ctx_spin = SpinButton::with_range(512.0, 131072.0, 512.0);
        ctx_spin.set_digits(0);
        ctx_spin.set_value(4096.0);
        ctx_spin.set_valign(Align::Center);
        ctx_spin.set_width_chars(7);
        ctx_spin.set_css_classes(&["numeric"]);

        ctx_header_box.append(&ctx_header_label);
        ctx_header_box.append(&ctx_spin);
        settings_box.append(&ctx_header_box);

        let ctx_scale = Scale::with_range(Orientation::Horizontal, 512.0, 131072.0, 512.0);
        ctx_scale.set_value(4096.0);
        Self::apply_context_range_marks(&ctx_scale, &ctx_spin, 131072, &ctx_header_label);

        crate::ui::helpers::bind_slider_and_spin(&ctx_scale, &ctx_spin);

        settings_box.append(&ctx_scale);
        popover.set_child(Some(&settings_box));

        Self {
            popover,
            system_prompt_buffer,
            temp_scale,
            ctx_header_label,
            ctx_scale,
            ctx_spin,
        }
    }

    pub fn popover(&self) -> &Popover {
        &self.popover
    }

    pub fn system_prompt(&self) -> Option<String> {
        let text = self.system_prompt_buffer
            .text(&self.system_prompt_buffer.start_iter(), &self.system_prompt_buffer.end_iter(), false)
            .to_string()
            .trim()
            .to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    pub fn set_system_prompt(&self, text: &str) {
        self.system_prompt_buffer.set_text(text);
    }

    pub fn temperature(&self) -> Option<f64> {
        Some(self.temp_scale.value())
    }

    pub fn set_temperature(&self, val: f64) {
        self.temp_scale.set_value(val);
    }

    pub fn num_ctx(&self) -> Option<u32> {
        Some(self.ctx_spin.value() as u32)
    }

    pub fn set_num_ctx(&self, val: u32) {
        self.ctx_scale.set_value(val as f64);
        self.ctx_spin.set_value(val as f64);
    }

    fn apply_context_range_marks(
        ctx_scale: &Scale,
        ctx_spin: &SpinButton,
        max_ctx: u32,
        ctx_header_label: &Label,
    ) {
        let max_val = (max_ctx.max(2048)) as f64;
        ctx_scale.set_range(512.0, max_val);
        ctx_spin.set_range(512.0, max_val);

        ctx_scale.clear_marks();
        let marks = [
            (2048.0, "2k"),
            (4096.0, "4k"),
            (8192.0, "8k"),
            (16384.0, "16k"),
            (32768.0, "32k"),
            (65536.0, "64k"),
            (131072.0, "128k"),
        ];
        for (val, txt) in marks {
            if val <= max_val {
                ctx_scale.add_mark(val, gtk4::PositionType::Bottom, Some(txt));
            }
        }

        let max_display = if max_ctx >= 1024 {
            format!("{}k", max_ctx / 1024)
        } else {
            format!("{}", max_ctx)
        };
        ctx_header_label.set_label(&t!("chat_settings.context_window", max = max_display));
    }

    pub fn update_context_slider_range(&self, max_ctx: u32) {
        Self::apply_context_range_marks(&self.ctx_scale, &self.ctx_spin, max_ctx, &self.ctx_header_label);
    }

    pub fn detect_max_context_for_model(model_name: &str) -> u32 {
        let m = model_name.to_lowercase();
        if m.contains("llama3.1")
            || m.contains("llama3.2")
            || m.contains("llama3.3")
            || m.contains("qwen2.5")
            || m.contains("qwq")
            || m.contains("deepseek-r1")
            || m.contains("deepseek-v3")
            || m.contains("mistral-nemo")
            || m.contains("mistral-large")
            || m.contains("codestral")
            || m.contains("command-r")
            || m.contains("hermes")
        {
            131072
        } else if m.contains("mistral") || m.contains("mixtral") || m.contains("qwen2") || m.contains("deepseek") {
            32768
        } else if m.contains("llama3") || m.contains("gemma2") || m.contains("phi3") || m.contains("phi4") || m.contains("starcoder2") {
            8192
        } else {
            4096
        }
    }

    pub fn default_context_for_model(model_name: &str) -> u32 {
        let m = model_name.to_lowercase();
        if m.contains("deepseek") || m.contains("qwen2.5") || m.contains("qwq") || m.contains("command-r") || m.contains("hermes") {
            32768
        } else if m.contains("llama3.1") || m.contains("llama3.2") || m.contains("llama3.3") || m.contains("mistral") || m.contains("mixtral") {
            16384
        } else if m.contains("llama3") || m.contains("gemma2") || m.contains("phi3") {
            8192
        } else {
            4096
        }
    }

    pub fn apply_model_config(
        &self,
        model_name: &str,
        config: &Rc<RefCell<AppConfig>>,
        client: &Rc<RefCell<OllamaClient>>,
        model_max_contexts: &Rc<RefCell<HashMap<String, u32>>>,
    ) {
        let cached_max = model_max_contexts.borrow().get(model_name).copied();
        let max_ctx = cached_max.unwrap_or_else(|| Self::detect_max_context_for_model(model_name));
        self.update_context_slider_range(max_ctx);

        {
            let cfg = config.borrow();
            if let Some(custom) = cfg.get_model_settings(model_name) {
                if let Some(ctx) = custom.num_ctx {
                    let clamped = (ctx.min(max_ctx)) as f64;
                    self.ctx_scale.set_value(clamped);
                    self.ctx_spin.set_value(clamped);
                } else {
                    let default_ctx = (Self::default_context_for_model(model_name).min(max_ctx)) as f64;
                    self.ctx_scale.set_value(default_ctx);
                    self.ctx_spin.set_value(default_ctx);
                }

                if let Some(temp) = custom.temperature {
                    self.temp_scale.set_value(temp);
                }

                if let Some(ref sys) = custom.system_prompt {
                    let cur = self.system_prompt_buffer
                        .text(&self.system_prompt_buffer.start_iter(), &self.system_prompt_buffer.end_iter(), false);
                    if cur.trim().is_empty() {
                        self.system_prompt_buffer.set_text(sys);
                    }
                }
            } else {
                let default_ctx = (Self::default_context_for_model(model_name).min(max_ctx)) as f64;
                self.ctx_scale.set_value(default_ctx);
                self.ctx_spin.set_value(default_ctx);
            }
        }

        if cached_max.is_none() {
            let client_c = client.borrow().clone();
            let model_name_c = model_name.to_string();
            let cache_c = model_max_contexts.clone();
            let scale_c = self.ctx_scale.clone();
            let spin_c = self.ctx_spin.clone();
            let lbl_c = self.ctx_header_label.clone();
            let cfg_c = config.clone();

            glib::MainContext::default().spawn_local(async move {
                if let Ok(show) = client_c.show_model(&model_name_c).await {
                    if let Some(real_max) = show.extract_context_length() {
                        cache_c.borrow_mut().insert(model_name_c.clone(), real_max);
                        Self::apply_context_range_marks(&scale_c, &spin_c, real_max, &lbl_c);

                        let cfg = cfg_c.borrow();
                        if let Some(custom) = cfg.get_model_settings(&model_name_c) {
                            if let Some(ctx) = custom.num_ctx {
                                let clamped = (ctx.min(real_max)) as f64;
                                scale_c.set_value(clamped);
                                spin_c.set_value(clamped);
                            }
                        }
                    }
                }
            });
        }
    }
}
