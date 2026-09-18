use crate::api::types::ModelPs;
use crate::api::OllamaClient;
use crate::core::hardware::estimator::format_bytes;
use crate::core::spawn_async;
use crate::t;
use adw::prelude::*;
use adw::{ActionRow, Clamp, Dialog, HeaderBar, PreferencesGroup, ToolbarView};
use gtk4::{Align, Box, Button, Image, Label, ListBox, Orientation, ProgressBar, ScrolledWindow, Spinner};
use std::cell::RefCell;
use std::rc::Rc;

pub struct RunningModelInfoDialog;

impl RunningModelInfoDialog {
    pub fn show(
        parent: Option<&impl IsA<gtk4::Widget>>,
        ps: ModelPs,
    ) {
        let dialog = Dialog::builder()
            .title(format!("ℹ️ {}", ps.name))
            .content_width(640)
            .content_height(600)
            .build();

        let toolbar_view = ToolbarView::new();
        let header_bar = HeaderBar::new();

        // Copy button in header bar
        let btn_copy = Button::builder()
            .icon_name("edit-copy-symbolic")
            .tooltip_text(t!("instances.info_copy_btn"))
            .valign(Align::Center)
            .build();
        header_bar.pack_end(&btn_copy);

        toolbar_view.add_top_bar(&header_bar);

        let main_scroller = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .build();

        let clamp = Clamp::builder()
            .maximum_size(600)
            .tightening_threshold(500)
            .margin_top(16)
            .margin_bottom(24)
            .margin_start(16)
            .margin_end(16)
            .build();

        let root_box = Box::new(Orientation::Vertical, 16);

        // Calculate VRAM and CPU offload
        let cpu_offload = ps.size.saturating_sub(ps.size_vram);
        let is_full_gpu = cpu_offload == 0;
        let vram_ratio = if ps.size > 0 {
            (ps.size_vram as f64 / ps.size as f64).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let vram_pct = (vram_ratio * 100.0).round() as u64;
        let ram_pct = 100_u64.saturating_sub(vram_pct);

        // 1. ACTIVE MEMORY SESSION
        let mem_group = PreferencesGroup::builder()
            .title(t!("instances.info_memory_group"))
            .description(t!("instances.info_memory_desc"))
            .build();

        let mem_box = ListBox::builder()
            .css_classes(["boxed-list"])
            .selection_mode(gtk4::SelectionMode::None)
            .build();

        // Row: VRAM Allocation
        let vram_row = ActionRow::builder()
            .title(t!("instances.info_vram_alloc"))
            .subtitle(t!("instances.info_vram_subtitle"))
            .build();
        let icon_gpu = Image::from_icon_name("video-display-symbolic");
        icon_gpu.set_pixel_size(18);
        vram_row.add_prefix(&icon_gpu);
        let vram_val = Label::builder()
            .label(format_bytes(ps.size_vram))
            .css_classes(["heading", "accent"])
            .valign(Align::Center)
            .build();
        vram_row.add_suffix(&vram_val);
        mem_box.append(&vram_row);

        // Row: System RAM (CPU)
        let ram_row = ActionRow::builder()
            .title(t!("instances.info_ram_alloc"))
            .subtitle(t!("instances.info_ram_subtitle"))
            .build();
        let icon_cpu = Image::from_icon_name("drive-harddisk-symbolic");
        icon_cpu.set_pixel_size(18);
        ram_row.add_prefix(&icon_cpu);
        let ram_val = Label::builder()
            .label(format_bytes(cpu_offload))
            .css_classes(["heading", "dim-label"])
            .valign(Align::Center)
            .build();
        ram_row.add_suffix(&ram_val);
        mem_box.append(&ram_row);

        // Row: Acceleration ratio & bar
        let accel_row = ActionRow::builder()
            .title(t!("instances.info_accel"))
            .build();
        let accel_label = if is_full_gpu {
            t!("instances.info_accel_full_gpu")
        } else {
            t!("instances.info_accel_hybrid", vram = vram_pct.to_string(), ram = ram_pct.to_string())
        };
        let accel_badge = Label::builder()
            .label(&accel_label)
            .css_classes(if is_full_gpu { vec!["running-alloc-chip"] } else { vec!["running-alloc-chip", "hybrid"] })
            .valign(Align::Center)
            .build();
        accel_row.add_suffix(&accel_badge);
        mem_box.append(&accel_row);

        // Memory ProgressBar
        let vram_bar = ProgressBar::builder()
            .fraction(vram_ratio)
            .css_classes(if is_full_gpu { vec!["running-vram-bar"] } else { vec!["running-vram-bar", "hybrid"] })
            .margin_start(12)
            .margin_end(12)
            .margin_top(4)
            .margin_bottom(8)
            .build();
        mem_box.append(&vram_bar);

        // Row: Active context length in memory
        if let Some(ctx_len) = ps.context_length {
            let ctx_row = ActionRow::builder()
                .title(t!("instances.info_active_ctx"))
                .build();
            let icon_ctx = Image::from_icon_name("format-justify-left-symbolic");
            icon_ctx.set_pixel_size(18);
            ctx_row.add_prefix(&icon_ctx);
            let ctx_badge = Label::builder()
                .label(t!("instances.info_active_ctx_tokens", tokens = ctx_len.to_string()))
                .css_classes(["badge"])
                .valign(Align::Center)
                .build();
            ctx_row.add_suffix(&ctx_badge);
            mem_box.append(&ctx_row);
        }

        // Row: Keep-Alive / Expiration
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

        let expire_row = ActionRow::builder()
            .title(t!("instances.info_keep_alive"))
            .build();
        let icon_timer = Image::from_icon_name("preferences-system-time-symbolic");
        icon_timer.set_pixel_size(18);
        expire_row.add_prefix(&icon_timer);
        let expire_badge = Label::builder()
            .label(&expire_text)
            .css_classes(["running-expire-chip"])
            .valign(Align::Center)
            .build();
        expire_row.add_suffix(&expire_badge);
        mem_box.append(&expire_row);

        mem_group.add(&mem_box);
        root_box.append(&mem_group);

        // 2. IDENTIFICATION & FORMAT
        let format_group = PreferencesGroup::builder()
            .title(t!("instances.info_format_group"))
            .build();

        let format_box = ListBox::builder()
            .css_classes(["boxed-list"])
            .selection_mode(gtk4::SelectionMode::None)
            .build();

        if let Some(ref det) = ps.details {
            if let Some(ref fam) = det.family {
                let row = ActionRow::builder()
                    .title(t!("instances.info_family"))
                    .build();
                let val = Label::builder().label(fam).css_classes(["dim-label"]).build();
                row.add_suffix(&val);
                format_box.append(&row);
            }
            if let Some(ref param) = det.parameter_size {
                let row = ActionRow::builder()
                    .title(t!("instances.info_params_size"))
                    .build();
                let val = Label::builder().label(param).css_classes(["dim-label"]).build();
                row.add_suffix(&val);
                format_box.append(&row);
            }
            if let Some(ref quant) = det.quantization_level {
                let row = ActionRow::builder()
                    .title(t!("instances.info_quant"))
                    .build();
                let val = Label::builder().label(quant).css_classes(["badge"]).build();
                row.add_suffix(&val);
                format_box.append(&row);
            }
        }

        if let Some(ref dig) = ps.digest {
            let row = ActionRow::builder()
                .title(t!("instances.info_digest"))
                .subtitle(if dig.len() > 24 { format!("{}...", &dig[..24]) } else { dig.clone() })
                .build();
            format_box.append(&row);
        }

        format_group.add(&format_box);
        root_box.append(&format_group);

        // 3. GENERATION HYPERPARAMETERS (Async loaded from /api/show)
        let hyperparams_group = PreferencesGroup::builder()
            .title(t!("instances.info_hyperparams_group"))
            .description(t!("instances.info_hyperparams_desc"))
            .build();

        let hyperparams_box = ListBox::builder()
            .css_classes(["boxed-list"])
            .selection_mode(gtk4::SelectionMode::None)
            .build();

        // Loading indicator
        let loading_row = ActionRow::builder()
            .title(t!("instances.info_loading_params"))
            .build();
        let spinner = Spinner::builder().spinning(true).valign(Align::Center).build();
        loading_row.add_suffix(&spinner);
        hyperparams_box.append(&loading_row);
        hyperparams_group.add(&hyperparams_box);
        root_box.append(&hyperparams_group);

        // 4. ARCHITECTURAL SPECIFICATIONS GROUP
        let arch_group = PreferencesGroup::builder()
            .title(t!("instances.info_architecture_group"))
            .build();

        let arch_box = ListBox::builder()
            .css_classes(["boxed-list"])
            .selection_mode(gtk4::SelectionMode::None)
            .build();
        arch_group.add(&arch_box);
        root_box.append(&arch_group);

        clamp.set_child(Some(&root_box));
        main_scroller.set_child(Some(&clamp));
        toolbar_view.set_content(Some(&main_scroller));
        dialog.set_child(Some(&toolbar_view));

        // Shared text buffer for Copy button
        let summary_text: Rc<RefCell<String>> = Rc::new(RefCell::new(format!(
            "# Model: {}\n- VRAM: {}\n- RAM (CPU): {}\n- Acceleration: {}\n- Context: {}\n",
            ps.name,
            format_bytes(ps.size_vram),
            format_bytes(cpu_offload),
            if is_full_gpu { "100% Full GPU" } else { "Hybrid Offload" },
            ps.context_length.map(|c| c.to_string()).unwrap_or_else(|| "N/A".to_string())
        )));

        // Connect copy button
        let dialog_for_copy = dialog.clone();
        let summary_ref = summary_text.clone();
        let btn_copy_ref = btn_copy.clone();
        btn_copy.connect_clicked(move |_| {
            let text = summary_ref.borrow().clone();
            dialog_for_copy.clipboard().set_text(&text);
            btn_copy_ref.set_tooltip_text(Some(&t!("instances.info_copied")));
            btn_copy_ref.set_icon_name("emblem-ok-symbolic");
            let btn_reset = btn_copy_ref.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
                btn_reset.set_icon_name("edit-copy-symbolic");
                btn_reset.set_tooltip_text(Some(&t!("instances.info_copy_btn")));
            });
        });

        // Async fetch /api/show
        let model_name_req = ps.name.clone();
        let hyperparams_box_ref = hyperparams_box.clone();
        let arch_box_ref = arch_box.clone();
        let arch_group_ref = arch_group.clone();
        let summary_for_async = summary_text.clone();

        spawn_async(
            async move {
                let host = crate::core::config::AppConfig::load().ollama_host;
                let client = OllamaClient::new(host);
                client.show_model(&model_name_req).await
            },
            move |res| {
                // Clear the loading row
                while let Some(child) = hyperparams_box_ref.first_child() {
                    hyperparams_box_ref.remove(&child);
                }

                if let Ok(show) = res {
                    let mut has_params = false;
                    let mut summary_extra = String::from("\n## Hyperparameters & Specs:\n");

                    // Parse parameters string
                    if let Some(ref params) = show.parameters {
                        for line in params.lines() {
                            let trimmed = line.trim();
                            if trimmed.is_empty() { continue; }
                            let parts: Vec<&str> = trimmed.split_whitespace().collect();
                            if parts.len() >= 2 {
                                let key = parts[0];
                                let val = parts[1..].join(" ");

                                let row = ActionRow::builder()
                                    .title(format_param_name(key))
                                    .subtitle(format!("PARAMETER {}", key))
                                    .build();
                                let val_label = Label::builder()
                                    .label(&val)
                                    .css_classes(["heading", "accent"])
                                    .valign(Align::Center)
                                    .build();
                                row.add_suffix(&val_label);
                                hyperparams_box_ref.append(&row);
                                summary_extra.push_str(&format!("- {}: {}\n", key, val));
                                has_params = true;
                            }
                        }
                    }

                    if !has_params {
                        let row = ActionRow::builder()
                            .title(t!("instances.info_no_custom_params"))
                            .subtitle("temperature: 0.8  •  top_p: 0.9  •  top_k: 40")
                            .build();
                        hyperparams_box_ref.append(&row);
                    }

                    // Populate Architectural specifications
                    let mut has_arch = false;
                    if let Some(ctx) = show.extract_context_length() {
                        let row = ActionRow::builder()
                            .title(t!("instances.info_native_ctx"))
                            .build();
                        let badge = Label::builder()
                            .label(format!("{} tokens", format_number(ctx)))
                            .css_classes(["badge"])
                            .valign(Align::Center)
                            .build();
                        row.add_suffix(&badge);
                        arch_box_ref.append(&row);
                        summary_extra.push_str(&format!("- Native Context: {} tokens\n", ctx));
                        has_arch = true;
                    }

                    if let Some(layers) = show.extract_layer_count() {
                        let row = ActionRow::builder()
                            .title(t!("instances.info_layers"))
                            .build();
                        let badge = Label::builder()
                            .label(layers.to_string())
                            .css_classes(["dim-label"])
                            .valign(Align::Center)
                            .build();
                        row.add_suffix(&badge);
                        arch_box_ref.append(&row);
                        summary_extra.push_str(&format!("- Layers: {}\n", layers));
                        has_arch = true;
                    }

                    if let Some(heads) = show.extract_head_count() {
                        let row = ActionRow::builder()
                            .title(t!("instances.info_heads"))
                            .build();
                        let text = if let Some(kv) = show.extract_kv_heads() {
                            t!("instances.info_heads_detail", heads = heads.to_string(), kv = kv.to_string())
                        } else {
                            format!("{} heads", heads)
                        };
                        let badge = Label::builder()
                            .label(&text)
                            .css_classes(["dim-label"])
                            .valign(Align::Center)
                            .build();
                        row.add_suffix(&badge);
                        arch_box_ref.append(&row);
                        summary_extra.push_str(&format!("- Heads: {}\n", text));
                        has_arch = true;
                    }

                    if let Some(emb) = show.extract_embedding_length() {
                        let row = ActionRow::builder()
                            .title(t!("instances.info_embedding_dim"))
                            .build();
                        let badge = Label::builder()
                            .label(emb.to_string())
                            .css_classes(["dim-label"])
                            .valign(Align::Center)
                            .build();
                        row.add_suffix(&badge);
                        arch_box_ref.append(&row);
                        summary_extra.push_str(&format!("- Embedding Dim: {}\n", emb));
                        has_arch = true;
                    }

                    if let Some(ref info) = show.model_info {
                        for (k, v) in info {
                            if k.ends_with(".vocab_size") || k == "vocab_size" {
                                if let Some(vocab) = v.as_u64() {
                                    let row = ActionRow::builder()
                                        .title(t!("instances.info_vocab_size"))
                                        .build();
                                    let badge = Label::builder()
                                        .label(format_number(vocab as u32))
                                        .css_classes(["dim-label"])
                                        .valign(Align::Center)
                                        .build();
                                    row.add_suffix(&badge);
                                    arch_box_ref.append(&row);
                                    summary_extra.push_str(&format!("- Vocab Size: {}\n", vocab));
                                    has_arch = true;
                                    break;
                                }
                            }
                        }
                    }

                    if !has_arch {
                        arch_group_ref.set_visible(false);
                    }

                    summary_for_async.borrow_mut().push_str(&summary_extra);
                } else {
                    let row = ActionRow::builder()
                        .title(t!("instances.info_no_custom_params"))
                        .build();
                    hyperparams_box_ref.append(&row);
                    arch_group_ref.set_visible(false);
                }
            }
        );

        dialog.present(parent);
    }
}

fn format_param_name(key: &str) -> String {
    match key {
        "num_ctx" => "Context Window (num_ctx)".to_string(),
        "temperature" => "Temperature (temperature)".to_string(),
        "top_p" => "Top-P Sampling (top_p)".to_string(),
        "top_k" => "Top-K Sampling (top_k)".to_string(),
        "min_p" => "Min-P Sampling (min_p)".to_string(),
        "repeat_penalty" => "Repeat Penalty (repeat_penalty)".to_string(),
        "repeat_last_n" => "Repeat Last N (repeat_last_n)".to_string(),
        "presence_penalty" => "Presence Penalty (presence_penalty)".to_string(),
        "frequency_penalty" => "Frequency Penalty (frequency_penalty)".to_string(),
        "num_predict" => "Max Predict Tokens (num_predict)".to_string(),
        "seed" => "Random Seed (seed)".to_string(),
        "num_thread" | "threads" => "CPU Threads (num_thread)".to_string(),
        "num_gpu" => "GPU Layers (num_gpu)".to_string(),
        "stop" => "Stop Sequence (stop)".to_string(),
        other => other.to_string(),
    }
}

fn format_number(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    let mut count = 0;
    for c in s.chars().rev() {
        if count > 0 && count % 3 == 0 {
            out.push(',');
        }
        out.push(c);
        count += 1;
    }
    out.chars().rev().collect()
}
