use crate::api::OllamaClient;
use crate::core::config::{AppConfig, CustomModelSettings};
use crate::core::hardware::estimator::{calculate_memory_estimate, ModelTopology};
use crate::core::hardware::HardwareMonitor;
use crate::core::spawn_async;
use crate::t;
use crate::ui::helpers::bind_slider_and_spin;
use adw::prelude::*;
use adw::{ActionRow, Clamp, ComboRow, Dialog, EntryRow, ExpanderRow, HeaderBar, PreferencesGroup, SwitchRow, ToolbarView, ViewStack, ViewSwitcher, ViewSwitcherPolicy};
use gtk4::{Align, Box, Button, Orientation, ProgressBar, Scale, ScrolledWindow, SpinButton, TextView, WrapMode};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

pub struct ModelSettingsDialog;

struct CustomModelfileParams<'a> {
    base_model: &'a str,
    template: Option<&'a str>,
    ctx: u32,
    temp: f64,
    top_p: f64,
    top_k: u32,
    min_p: f64,
    predict: i32,
    seed: i64,
    threads: u32,
    repeat_penalty: f64,
    repeat_last_n: i32,
    presence_penalty: f64,
    frequency_penalty: f64,
    num_gpu: i32,
    use_mlock: bool,
    system_text: &'a str,
}

fn build_custom_modelfile(p: &CustomModelfileParams) -> String {
    let mut modelfile = format!("FROM {}\n", p.base_model);
    if let Some(tpl) = p.template {
        let trimmed = tpl.trim();
        if !trimmed.is_empty() {
            modelfile.push_str(&format!("TEMPLATE \"\"\"{}\"\"\"\n", trimmed));
        }
    }
    modelfile.push_str(&format!("PARAMETER num_ctx {}\n", p.ctx));
    modelfile.push_str(&format!("PARAMETER temperature {}\n", p.temp));
    modelfile.push_str(&format!("PARAMETER top_p {}\n", p.top_p));
    modelfile.push_str(&format!("PARAMETER top_k {}\n", p.top_k));
    modelfile.push_str(&format!("PARAMETER min_p {}\n", p.min_p));
    modelfile.push_str(&format!("PARAMETER repeat_penalty {}\n", p.repeat_penalty));
    modelfile.push_str(&format!("PARAMETER repeat_last_n {}\n", p.repeat_last_n));
    modelfile.push_str(&format!("PARAMETER presence_penalty {}\n", p.presence_penalty));
    modelfile.push_str(&format!("PARAMETER frequency_penalty {}\n", p.frequency_penalty));
    if p.predict >= 0 {
        modelfile.push_str(&format!("PARAMETER num_predict {}\n", p.predict));
    }
    if p.seed > 0 {
        modelfile.push_str(&format!("PARAMETER seed {}\n", p.seed));
    }
    if p.threads > 0 {
        modelfile.push_str(&format!("PARAMETER num_thread {}\n", p.threads));
    }
    if p.num_gpu >= 0 {
        modelfile.push_str(&format!("PARAMETER num_gpu {}\n", p.num_gpu));
    }
    if p.use_mlock {
        modelfile.push_str("PARAMETER use_mlock true\n");
    }
    let trimmed_sys = p.system_text.trim();
    if !trimmed_sys.is_empty() {
        modelfile.push_str(&format!("SYSTEM \"\"\"{}\"\"\"\n", trimmed_sys));
    }
    modelfile
}

impl ModelSettingsDialog {
    pub fn show(
        parent: Option<&impl IsA<gtk4::Widget>>,
        model_name: &str,
        model_size_bytes: u64,
        client: &OllamaClient,
        config: &Rc<RefCell<AppConfig>>,
        hw_monitor: &Rc<RefCell<HardwareMonitor>>,
        on_variant_created: impl Fn(String, String) + 'static,
    ) {
        let dialog = Dialog::builder()
            .title(t!("model_settings.title", model = model_name))
            .content_width(780)
            .content_height(720)
            .build();

        let toolbar_view = ToolbarView::new();
        let header_bar = HeaderBar::new();
        toolbar_view.add_top_bar(&header_bar);

        let view_stack = ViewStack::new();
        let view_switcher = ViewSwitcher::builder()
            .stack(&view_stack)
            .policy(ViewSwitcherPolicy::Wide)
            .build();
        header_bar.set_title_widget(Some(&view_switcher));

        let model_str = model_name.to_string();
        let client_clone = client.clone();
        let config_clone = config.clone();
        let on_variant_created_rc: Rc<dyn Fn(String, String)> = Rc::new(on_variant_created);
        let template_holder: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

        let initial_bytes = if model_size_bytes > 0 {
            model_size_bytes
        } else {
            4 * 1024 * 1024 * 1024
        };

        let default_kv_heads = if initial_bytes < 2_500_000_000 {
            2
        } else if initial_bytes < 4_000_000_000 {
            4
        } else {
            8
        };

        let model_est_state = Rc::new(RefCell::new(ModelTopology {
            total_layers: 32,
            base_model_bytes: initial_bytes,
            context_length: 32768,
            kv_heads: default_kv_heads,
            head_dim: 128,
            sliding_window: None,
        }));

        let existing_settings = config.borrow().get_model_settings(model_name).cloned().unwrap_or_default();

        // STICKY PREDICTION PANEL
        let clamp_sticky = Clamp::builder()
            .maximum_size(720)
            .tightening_threshold(500)
            .margin_top(10)
            .margin_bottom(4)
            .margin_start(16)
            .margin_end(16)
            .build();

        let sticky_group = PreferencesGroup::builder()
            .title(t!("model_settings.sticky_title"))
            .description(t!("model_settings.sticky_desc"))
            .build();

        let row_est_vram = ActionRow::builder()
            .title(t!("model_settings.est_vram"))
            .subtitle(t!("model_settings.est_calculating"))
            .build();
        let prog_est_vram = ProgressBar::builder()
            .valign(Align::Center)
            .width_request(140)
            .show_text(false)
            .build();
        row_est_vram.add_suffix(&prog_est_vram);
        sticky_group.add(&row_est_vram);

        let row_est_ram = ActionRow::builder()
            .title(t!("model_settings.est_ram"))
            .subtitle(t!("model_settings.est_calculating"))
            .build();
        let prog_est_ram = ProgressBar::builder()
            .valign(Align::Center)
            .width_request(140)
            .show_text(false)
            .build();
        row_est_ram.add_suffix(&prog_est_ram);
        sticky_group.add(&row_est_ram);

        let row_est_advice = ActionRow::builder()
            .title(t!("model_settings.est_advice"))
            .subtitle(t!("model_settings.est_calculating"))
            .build();
        sticky_group.add(&row_est_advice);

        clamp_sticky.set_child(Some(&sticky_group));

        // TAB 1: INFERENCE
        let group_params = PreferencesGroup::builder()
            .title(t!("model_settings.group_params_title"))
            .description(t!("model_settings.group_params_desc"))
            .build();

        // 1. Context Window (num_ctx)
        let adj_ctx = gtk4::Adjustment::new(
            existing_settings.num_ctx.unwrap_or(4096) as f64,
            512.0,
            131072.0,
            512.0,
            4096.0,
            0.0,
        );
        let scale_ctx = Scale::builder()
            .adjustment(&adj_ctx)
            .hexpand(true)
            .width_request(160)
            .valign(Align::Center)
            .round_digits(0)
            .build();
        let spin_ctx = SpinButton::builder()
            .adjustment(&adj_ctx)
            .valign(Align::Center)
            .numeric(true)
            .build();
        let row_ctx = ActionRow::builder()
            .title(t!("model_settings.row_ctx_title"))
            .subtitle(t!("model_settings.row_ctx_sub"))
            .build();
        let box_ctx = Box::new(Orientation::Horizontal, 8);
        box_ctx.append(&scale_ctx);
        box_ctx.append(&spin_ctx);
        row_ctx.add_suffix(&box_ctx);
        group_params.add(&row_ctx);

        // 2. GPU Layer Offloading (num_gpu)
        let adj_gpu = gtk4::Adjustment::new(
            existing_settings.num_gpu.unwrap_or(-1) as f64,
            -1.0,
            128.0,
            1.0,
            4.0,
            0.0,
        );
        let scale_gpu = Scale::builder()
            .adjustment(&adj_gpu)
            .hexpand(true)
            .width_request(160)
            .valign(Align::Center)
            .round_digits(0)
            .build();
        let spin_gpu = SpinButton::builder()
            .adjustment(&adj_gpu)
            .valign(Align::Center)
            .numeric(true)
            .build();
        let row_gpu = ActionRow::builder()
            .title(t!("model_settings.row_gpu_title"))
            .subtitle(t!("model_settings.row_gpu_sub"))
            .build();
        let box_gpu = Box::new(Orientation::Horizontal, 8);
        box_gpu.append(&scale_gpu);
        box_gpu.append(&spin_gpu);
        row_gpu.add_suffix(&box_gpu);
        group_params.add(&row_gpu);

        // 3. Temperature
        let adj_temp = gtk4::Adjustment::new(
            existing_settings.temperature.unwrap_or(0.7),
            0.0,
            2.0,
            0.05,
            0.1,
            0.0,
        );
        let scale_temp = Scale::builder()
            .adjustment(&adj_temp)
            .hexpand(true)
            .width_request(160)
            .valign(Align::Center)
            .round_digits(2)
            .build();
        let spin_temp = SpinButton::builder()
            .adjustment(&adj_temp)
            .valign(Align::Center)
            .digits(2)
            .numeric(true)
            .build();
        let row_temp = ActionRow::builder()
            .title(t!("model_settings.row_temp_title"))
            .subtitle(t!("model_settings.row_temp_sub"))
            .build();
        let box_temp = Box::new(Orientation::Horizontal, 8);
        box_temp.append(&scale_temp);
        box_temp.append(&spin_temp);
        row_temp.add_suffix(&box_temp);
        group_params.add(&row_temp);

        // 4. Top-P
        let adj_top_p = gtk4::Adjustment::new(
            existing_settings.top_p.unwrap_or(0.9),
            0.0,
            1.0,
            0.05,
            0.1,
            0.0,
        );
        let scale_top_p = Scale::builder()
            .adjustment(&adj_top_p)
            .hexpand(true)
            .width_request(160)
            .valign(Align::Center)
            .round_digits(2)
            .build();
        let spin_top_p = SpinButton::builder()
            .adjustment(&adj_top_p)
            .valign(Align::Center)
            .digits(2)
            .numeric(true)
            .build();
        let row_top_p = ActionRow::builder()
            .title(t!("model_settings.row_top_p_title"))
            .subtitle(t!("model_settings.row_top_p_sub"))
            .build();
        let box_top_p = Box::new(Orientation::Horizontal, 8);
        box_top_p.append(&scale_top_p);
        box_top_p.append(&spin_top_p);
        row_top_p.add_suffix(&box_top_p);
        group_params.add(&row_top_p);

        // 5. Top-K
        let adj_top_k = gtk4::Adjustment::new(
            existing_settings.top_k.unwrap_or(40) as f64,
            1.0,
            200.0,
            1.0,
            10.0,
            0.0,
        );
        let scale_top_k = Scale::builder()
            .adjustment(&adj_top_k)
            .hexpand(true)
            .width_request(160)
            .valign(Align::Center)
            .round_digits(0)
            .build();
        let spin_top_k = SpinButton::builder()
            .adjustment(&adj_top_k)
            .valign(Align::Center)
            .numeric(true)
            .build();
        let row_top_k = ActionRow::builder()
            .title(t!("model_settings.row_top_k_title"))
            .subtitle(t!("model_settings.row_top_k_sub"))
            .build();
        let box_top_k = Box::new(Orientation::Horizontal, 8);
        box_top_k.append(&scale_top_k);
        box_top_k.append(&spin_top_k);
        row_top_k.add_suffix(&box_top_k);
        group_params.add(&row_top_k);

        // 6. Repeat Penalty
        let adj_pen = gtk4::Adjustment::new(
            existing_settings.repeat_penalty.unwrap_or(1.1),
            1.0,
            2.0,
            0.05,
            0.1,
            0.0,
        );
        let scale_penalty = Scale::builder()
            .adjustment(&adj_pen)
            .hexpand(true)
            .width_request(160)
            .valign(Align::Center)
            .round_digits(2)
            .build();
        let spin_penalty = SpinButton::builder()
            .adjustment(&adj_pen)
            .valign(Align::Center)
            .digits(2)
            .numeric(true)
            .build();
        let row_pen = ActionRow::builder()
            .title(t!("model_settings.row_penalty_title"))
            .subtitle(t!("model_settings.row_penalty_sub"))
            .build();
        let box_pen = Box::new(Orientation::Horizontal, 8);
        box_pen.append(&scale_penalty);
        box_pen.append(&spin_penalty);
        row_pen.add_suffix(&box_pen);
        group_params.add(&row_pen);

        bind_slider_and_spin(&scale_ctx, &spin_ctx);
        bind_slider_and_spin(&scale_gpu, &spin_gpu);
        bind_slider_and_spin(&scale_temp, &spin_temp);
        bind_slider_and_spin(&scale_top_p, &spin_top_p);
        bind_slider_and_spin(&scale_top_k, &spin_top_k);
        bind_slider_and_spin(&scale_penalty, &spin_penalty);

        // Advanced Sampling Options
        let group_adv = PreferencesGroup::builder()
            .title(t!("model_settings.group_adv_title"))
            .description(t!("model_settings.group_adv_desc"))
            .build();

        let exp_adv = ExpanderRow::builder()
            .title(t!("model_settings.exp_adv_title"))
            .subtitle(t!("model_settings.exp_adv_sub"))
            .build();

        // 1. Min-P
        let adj_min_p = gtk4::Adjustment::new(
            existing_settings.min_p.unwrap_or(0.0),
            0.0,
            1.0,
            0.01,
            0.05,
            0.0,
        );
        let scale_min_p = Scale::builder()
            .adjustment(&adj_min_p)
            .hexpand(true)
            .width_request(160)
            .valign(Align::Center)
            .round_digits(2)
            .build();
        let spin_min_p = SpinButton::builder()
            .adjustment(&adj_min_p)
            .valign(Align::Center)
            .digits(2)
            .numeric(true)
            .build();
        let row_min_p = ActionRow::builder()
            .title(t!("model_settings.row_min_p_title"))
            .subtitle(t!("model_settings.row_min_p_sub"))
            .build();
        let box_min_p = Box::new(Orientation::Horizontal, 8);
        box_min_p.append(&scale_min_p);
        box_min_p.append(&spin_min_p);
        row_min_p.add_suffix(&box_min_p);
        exp_adv.add_row(&row_min_p);
        bind_slider_and_spin(&scale_min_p, &spin_min_p);

        // 2. Predict tokens limit
        let adj_predict = gtk4::Adjustment::new(
            existing_settings.num_predict.unwrap_or(-1) as f64,
            -1.0,
            131072.0,
            128.0,
            512.0,
            0.0,
        );
        let spin_predict = SpinButton::builder()
            .adjustment(&adj_predict)
            .valign(Align::Center)
            .numeric(true)
            .build();
        let row_predict = ActionRow::builder()
            .title(t!("model_settings.row_predict_title"))
            .subtitle(t!("model_settings.row_predict_sub"))
            .build();
        row_predict.add_suffix(&spin_predict);
        exp_adv.add_row(&row_predict);

        // 3. Random Seed
        let adj_seed = gtk4::Adjustment::new(
            existing_settings.seed.unwrap_or(0) as f64,
            0.0,
            2147483647.0,
            1.0,
            10.0,
            0.0,
        );
        let spin_seed = SpinButton::builder()
            .adjustment(&adj_seed)
            .valign(Align::Center)
            .numeric(true)
            .build();
        let row_seed = ActionRow::builder()
            .title(t!("model_settings.row_seed_title"))
            .subtitle(t!("model_settings.row_seed_sub"))
            .build();
        row_seed.add_suffix(&spin_seed);
        exp_adv.add_row(&row_seed);

        // 4. CPU Threads
        let adj_threads = gtk4::Adjustment::new(
            existing_settings.num_thread.unwrap_or(0) as f64,
            0.0,
            128.0,
            1.0,
            4.0,
            0.0,
        );
        let spin_threads = SpinButton::builder()
            .adjustment(&adj_threads)
            .valign(Align::Center)
            .numeric(true)
            .build();
        let row_threads = ActionRow::builder()
            .title(t!("model_settings.row_threads_title"))
            .subtitle(t!("model_settings.row_threads_sub"))
            .build();
        row_threads.add_suffix(&spin_threads);
        exp_adv.add_row(&row_threads);

        // 5. Presence Penalty
        let adj_presence = gtk4::Adjustment::new(
            existing_settings.presence_penalty.unwrap_or(0.0),
            0.0,
            2.0,
            0.05,
            0.1,
            0.0,
        );
        let scale_presence = Scale::builder()
            .adjustment(&adj_presence)
            .hexpand(true)
            .width_request(160)
            .valign(Align::Center)
            .round_digits(2)
            .build();
        let spin_presence = SpinButton::builder()
            .adjustment(&adj_presence)
            .valign(Align::Center)
            .digits(2)
            .numeric(true)
            .build();
        let row_presence = ActionRow::builder()
            .title(t!("model_settings.row_presence_title"))
            .subtitle(t!("model_settings.row_presence_sub"))
            .build();
        let box_presence = Box::new(Orientation::Horizontal, 8);
        box_presence.append(&scale_presence);
        box_presence.append(&spin_presence);
        row_presence.add_suffix(&box_presence);
        exp_adv.add_row(&row_presence);
        bind_slider_and_spin(&scale_presence, &spin_presence);

        // 6. Frequency Penalty
        let adj_freq = gtk4::Adjustment::new(
            existing_settings.frequency_penalty.unwrap_or(0.0),
            0.0,
            2.0,
            0.05,
            0.1,
            0.0,
        );
        let scale_freq = Scale::builder()
            .adjustment(&adj_freq)
            .hexpand(true)
            .width_request(160)
            .valign(Align::Center)
            .round_digits(2)
            .build();
        let spin_freq = SpinButton::builder()
            .adjustment(&adj_freq)
            .valign(Align::Center)
            .digits(2)
            .numeric(true)
            .build();
        let row_freq = ActionRow::builder()
            .title(t!("model_settings.row_freq_title"))
            .subtitle(t!("model_settings.row_freq_sub"))
            .build();
        let box_freq = Box::new(Orientation::Horizontal, 8);
        box_freq.append(&scale_freq);
        box_freq.append(&spin_freq);
        row_freq.add_suffix(&box_freq);
        exp_adv.add_row(&row_freq);
        bind_slider_and_spin(&scale_freq, &spin_freq);

        // 7. Repeat Last N
        let adj_last_n = gtk4::Adjustment::new(
            existing_settings.repeat_last_n.unwrap_or(64) as f64,
            -1.0,
            2048.0,
            16.0,
            64.0,
            0.0,
        );
        let spin_last_n = SpinButton::builder()
            .adjustment(&adj_last_n)
            .valign(Align::Center)
            .numeric(true)
            .build();
        let row_last_n = ActionRow::builder()
            .title(t!("model_settings.row_last_n_title"))
            .subtitle(t!("model_settings.row_last_n_sub"))
            .build();
        row_last_n.add_suffix(&spin_last_n);
        exp_adv.add_row(&row_last_n);

        // 8. RAM Locking (mlock)
        let switch_mlock = SwitchRow::builder()
            .title(t!("model_settings.row_mlock_title"))
            .subtitle(t!("model_settings.row_mlock_sub"))
            .active(existing_settings.use_mlock.unwrap_or(false))
            .build();
        exp_adv.add_row(&switch_mlock);

        // 9. Keep-alive
        let keep_alive_list = gtk4::StringList::new(&[
            "5 minutes (Ollama Default)",
            "15 minutes (Chat Recommended)",
            "30 minutes",
            "1 hour",
            "24 hours",
            "Infinite (-1, always resident)",
            "Immediate (0s, flush VRAM)",
        ]);
        let keep_alive_values = ["5m", "15m", "30m", "1h", "24h", "-1", "0s"];

        let initial_ka = existing_settings.keep_alive.as_deref().unwrap_or("5m");
        let initial_ka_idx = keep_alive_values.iter().position(|&v| v == initial_ka).unwrap_or(0) as u32;

        let combo_keep_alive = ComboRow::builder()
            .title(t!("model_settings.row_keep_alive_title"))
            .subtitle(t!("model_settings.row_keep_alive_sub"))
            .model(&keep_alive_list)
            .selected(initial_ka_idx)
            .build();
        exp_adv.add_row(&combo_keep_alive);

        group_adv.add(&exp_adv);

        // Save & Reset Buttons
        let group_save = PreferencesGroup::new();
        let box_save_buttons = Box::new(Orientation::Horizontal, 12);
        box_save_buttons.set_halign(Align::Center);
        box_save_buttons.set_margin_top(8);
        box_save_buttons.set_margin_bottom(8);

        let btn_save = Button::builder()
            .label(t!("model_settings.btn_save"))
            .css_classes(["suggested-action", "pill"])
            .tooltip_text(t!("model_settings.btn_save_tooltip"))
            .build();

        let btn_reset = Button::builder()
            .label(t!("model_settings.btn_reset"))
            .css_classes(["destructive-action", "pill"])
            .tooltip_text(t!("model_settings.btn_reset_tooltip"))
            .build();

        box_save_buttons.append(&btn_save);
        box_save_buttons.append(&btn_reset);
        group_save.add(&box_save_buttons);

        let scroll_params = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .build();
        let clamp_params = Clamp::builder()
            .maximum_size(720)
            .tightening_threshold(500)
            .margin_top(8)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();
        let box_params = Box::new(Orientation::Vertical, 16);
        box_params.append(&group_params);
        box_params.append(&group_adv);
        box_params.append(&group_save);
        clamp_params.set_child(Some(&box_params));
        scroll_params.set_child(Some(&clamp_params));

        let box_inference_tab = Box::new(Orientation::Vertical, 0);
        box_inference_tab.append(&clamp_sticky);
        box_inference_tab.append(&scroll_params);

        view_stack
            .add_titled(&box_inference_tab, Some("params"), &t!("model_settings.tab_params"))
            .set_icon_name(Some("preferences-system-symbolic"));

        // TAB 2: PROMPT & TEMPLATE
        let group_sys = PreferencesGroup::builder()
            .title(t!("model_settings.group_sys_title"))
            .description(t!("model_settings.group_sys_desc"))
            .build();

        let text_view_sys = TextView::builder()
            .wrap_mode(WrapMode::Word)
            .height_request(140)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        if let Some(ref sys_str) = existing_settings.system_prompt {
            text_view_sys.buffer().set_text(sys_str);
        }

        let scroll_sys = ScrolledWindow::builder()
            .height_request(140)
            .child(&text_view_sys)
            .css_classes(["card"])
            .build();
        group_sys.add(&scroll_sys);

        let group_tpl = PreferencesGroup::builder()
            .title(t!("model_settings.group_tpl_title"))
            .description(t!("model_settings.group_tpl_desc"))
            .build();

        let text_view_tpl = TextView::builder()
            .wrap_mode(WrapMode::Word)
            .editable(false)
            .monospace(true)
            .height_request(160)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        let scroll_tpl = ScrolledWindow::builder()
            .height_request(160)
            .child(&text_view_tpl)
            .css_classes(["card"])
            .build();
        group_tpl.add(&scroll_tpl);

        let scroll_prompt = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .build();
        let clamp_prompt = Clamp::builder()
            .maximum_size(720)
            .tightening_threshold(500)
            .margin_top(12)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();
        let box_prompt = Box::new(Orientation::Vertical, 16);
        box_prompt.append(&group_sys);
        box_prompt.append(&group_tpl);
        clamp_prompt.set_child(Some(&box_prompt));
        scroll_prompt.set_child(Some(&clamp_prompt));

        view_stack
            .add_titled(&scroll_prompt, Some("prompt"), &t!("model_settings.tab_prompt"))
            .set_icon_name(Some("text-x-generic-symbolic"));

        // TAB 3: MODELFILE & ARCHITECTURE
        let group_meta = PreferencesGroup::builder()
            .title(t!("model_settings.group_meta_title"))
            .description(t!("model_settings.group_meta_desc"))
            .build();

        let row_family = ActionRow::builder().title(t!("model_settings.row_family_title")).subtitle("...").build();
        let row_quant = ActionRow::builder().title(t!("model_settings.row_quant_title")).subtitle("...").build();
        let row_params_size = ActionRow::builder().title(t!("model_settings.row_params_size_title")).subtitle("...").build();
        group_meta.add(&row_family);
        group_meta.add(&row_quant);
        group_meta.add(&row_params_size);

        let group_mf = PreferencesGroup::builder()
            .title(t!("model_settings.group_mf_title"))
            .description(t!("model_settings.group_mf_desc"))
            .build();

        let text_view_mf = TextView::builder()
            .wrap_mode(WrapMode::Word)
            .editable(false)
            .monospace(true)
            .height_request(220)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        let scroll_mf = ScrolledWindow::builder()
            .height_request(220)
            .child(&text_view_mf)
            .css_classes(["card"])
            .build();
        group_mf.add(&scroll_mf);

        let group_lic = PreferencesGroup::builder()
            .title(t!("model_settings.group_lic_title"))
            .build();
        let text_view_lic = TextView::builder()
            .wrap_mode(WrapMode::Word)
            .editable(false)
            .monospace(true)
            .height_request(100)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        let scroll_lic = ScrolledWindow::builder()
            .height_request(100)
            .child(&text_view_lic)
            .css_classes(["card"])
            .build();
        group_lic.add(&scroll_lic);

        let scroll_modelfile = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .build();
        let clamp_modelfile = Clamp::builder()
            .maximum_size(720)
            .tightening_threshold(500)
            .margin_top(12)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();
        let box_modelfile = Box::new(Orientation::Vertical, 16);
        box_modelfile.append(&group_meta);
        box_modelfile.append(&group_mf);
        box_modelfile.append(&group_lic);
        clamp_modelfile.set_child(Some(&box_modelfile));
        scroll_modelfile.set_child(Some(&clamp_modelfile));

        view_stack
            .add_titled(&scroll_modelfile, Some("modelfile"), &t!("model_settings.tab_modelfile"))
            .set_icon_name(Some("edit-find-symbolic"));

        // TAB 4: CREATE VARIANT
        let group_create = PreferencesGroup::builder()
            .title(t!("model_settings.group_create_title"))
            .description(t!("model_settings.group_create_desc"))
            .build();

        let entry_new_name = EntryRow::builder()
            .title(t!("model_settings.entry_new_name_title"))
            .build();
        group_create.add(&entry_new_name);

        let btn_create_variant = Button::builder()
            .label(t!("model_settings.btn_create_variant"))
            .css_classes(["suggested-action", "pill"])
            .halign(Align::Center)
            .margin_top(12)
            .margin_bottom(8)
            .build();
        group_create.add(&btn_create_variant);

        let scroll_create = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .build();
        let clamp_create = Clamp::builder()
            .maximum_size(720)
            .tightening_threshold(500)
            .margin_top(12)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();
        clamp_create.set_child(Some(&group_create));
        scroll_create.set_child(Some(&clamp_create));

        view_stack
            .add_titled(&scroll_create, Some("create"), &t!("model_settings.tab_create"))
            .set_icon_name(Some("document-new-symbolic"));

        toolbar_view.set_content(Some(&view_stack));
        dialog.set_child(Some(&toolbar_view));

        // Real-time Memory Estimation Calculation
        let update_prediction = {
            let hw_m = hw_monitor.clone();
            let est_st = model_est_state.clone();
            let adj_ctx_p = adj_ctx.clone();
            let adj_gpu_p = adj_gpu.clone();
            let r_vram = row_est_vram.clone();
            let p_vram = prog_est_vram.clone();
            let r_ram = row_est_ram.clone();
            let p_ram = prog_est_ram.clone();
            let r_adv = row_est_advice.clone();

            Rc::new(move || {
                let snap = hw_m.borrow_mut().snapshot();
                let st = *est_st.borrow();
                let ctx = adj_ctx_p.value() as u32;
                let num_gpu = adj_gpu_p.value() as i32;

                let est = calculate_memory_estimate(&st, ctx, num_gpu, &snap);

                p_vram.set_fraction(est.vram_fraction);
                p_ram.set_fraction(est.ram_fraction);

                let est_vram_gb = est.total_vram_bytes as f64 / 1_073_741_824.0;
                let vram_total_gb = est.vram_total_available_bytes as f64 / 1_073_741_824.0;
                let vram_weights_gb = est.gpu_weights_bytes as f64 / 1_073_741_824.0;
                let vram_kv_gb = est.gpu_kv_bytes as f64 / 1_073_741_824.0;

                if est.vram_total_available_bytes > 0 {
                    r_vram.set_subtitle(&t!(
                        "model_settings.est_vram_format",
                        used = format!("{:.2}", est_vram_gb),
                        total = format!("{:.2}", vram_total_gb),
                        pct = format!("{:.0}", est.vram_fraction * 100.0),
                        weights = format!("{:.2}", vram_weights_gb),
                        kv = format!("{:.2}", vram_kv_gb)
                    ));
                } else {
                    r_vram.set_subtitle(&t!("model_settings.est_cpu_mode"));
                }

                let est_ram_gb = est.total_ram_bytes as f64 / 1_073_741_824.0;
                let ram_weights_gb = est.cpu_weights_bytes as f64 / 1_073_741_824.0;
                let ram_kv_gb = est.cpu_kv_bytes as f64 / 1_073_741_824.0;
                r_ram.set_subtitle(&t!(
                    "model_settings.est_ram_format",
                    used = format!("{:.2}", est_ram_gb),
                    weights = format!("{:.2}", ram_weights_gb),
                    kv = format!("{:.2}", ram_kv_gb)
                ));

                r_adv.set_subtitle(&est.advice_message);
            })
        };

        let up_ctx = update_prediction.clone();
        adj_ctx.connect_value_changed(move |_| {
            up_ctx();
        });

        let up_gpu = update_prediction.clone();
        adj_gpu.connect_value_changed(move |_| {
            up_gpu();
        });

        // Async model metadata load (/api/show)
        let model_show_name_req = model_str.clone();
        let tv_tpl = text_view_tpl.clone();
        let tv_mf = text_view_mf.clone();
        let tv_lic = text_view_lic.clone();
        let tv_sys = text_view_sys.clone();
        let rf = row_family.clone();
        let rq = row_quant.clone();
        let rp = row_params_size.clone();
        let row_gpu_clone = row_gpu.clone();
        let adj_gpu_clone = adj_gpu.clone();
        let row_ctx_clone = row_ctx.clone();
        let adj_ctx_clone = adj_ctx.clone();
        let est_state_clone = model_est_state.clone();
        let up_async = update_prediction.clone();
        let tpl_holder_async = template_holder.clone();
        let cfg_snap = config_clone.clone();
        let m_name_snap = model_str.clone();

        spawn_async(
            async move {
                let res = client_clone.show_model(&model_show_name_req).await;
                res
            },
            move |res| {
                if let Ok(show) = res {
                    if let Some(ref d) = show.details {
                        if let Some(ref fam) = d.family {
                            rf.set_subtitle(fam);
                        }
                        if let Some(ref q) = d.quantization_level {
                            rq.set_subtitle(q);
                        }
                        if let Some(ref ps) = d.parameter_size {
                            rp.set_subtitle(ps);
                        }
                    }
                    if let Some(ref tpl) = show.template {
                        *tpl_holder_async.borrow_mut() = Some(tpl.clone());
                        tv_tpl.buffer().set_text(tpl);
                    }
                    if let Some(ref mf) = show.modelfile {
                        tv_mf.buffer().set_text(mf);
                    }
                    if let Some(ref lic) = show.license {
                        tv_lic.buffer().set_text(lic);
                    }
                    if tv_sys.buffer().text(&tv_sys.buffer().start_iter(), &tv_sys.buffer().end_iter(), false).is_empty() {
                        if let Some(ref sys) = show.system {
                            tv_sys.buffer().set_text(sys);
                        }
                    }

                    {
                        let mut cfg_guard = cfg_snap.borrow_mut();
                        if cfg_guard.get_original_modelfile(&m_name_snap).is_none() {
                            if let Some(ref mf) = show.modelfile {
                                cfg_guard.set_original_modelfile(m_name_snap.clone(), mf.clone());
                            }
                        }
                        if cfg_guard.get_original_template(&m_name_snap).is_none() {
                            if let Some(ref tpl) = show.template {
                                cfg_guard.set_original_template(m_name_snap.clone(), tpl.clone());
                            }
                        }
                        let _ = cfg_guard.save();
                    }

                    let detected_layers = show.extract_layer_count();
                    let detected_ctx = show.extract_context_length();
                    let detected_kv_heads = show.extract_kv_heads();
                    let detected_head_dim = show.extract_head_dim();
                    let detected_sw = show.extract_sliding_window();

                    if let Some(layers) = detected_layers {
                        adj_gpu_clone.set_upper(layers as f64);
                        row_gpu_clone.set_subtitle(&format!(
                            "Architecture: {} layers total | -1 = Auto offload",
                            layers
                        ));
                        est_state_clone.borrow_mut().total_layers = layers;
                    }

                    if let Some(ctx_len) = detected_ctx {
                        adj_ctx_clone.set_upper(ctx_len as f64);
                        row_ctx_clone.set_subtitle(&format!(
                            "Native context: {} tokens | Current: {}",
                            ctx_len,
                            adj_ctx_clone.value() as u32
                        ));
                        est_state_clone.borrow_mut().context_length = ctx_len;
                    }

                    if let Some(kv_h) = detected_kv_heads {
                        est_state_clone.borrow_mut().kv_heads = kv_h;
                    }
                    if let Some(h_dim) = detected_head_dim {
                        est_state_clone.borrow_mut().head_dim = h_dim;
                    }
                    if let Some(sw) = detected_sw {
                        est_state_clone.borrow_mut().sliding_window = Some(sw);
                    }

                    up_async();
                }
            },
        );

        let cfg_for_save = config_clone.clone();
        let model_for_save = model_str.clone();
        let adj_ctx_s = adj_ctx.clone();
        let adj_temp_s = adj_temp.clone();
        let adj_tp_s = adj_top_p.clone();
        let adj_tk_s = adj_top_k.clone();
        let adj_min_p_s = adj_min_p.clone();
        let adj_predict_s = adj_predict.clone();
        let adj_seed_s = adj_seed.clone();
        let adj_threads_s = adj_threads.clone();
        let adj_pen_s = adj_pen.clone();
        let adj_last_n_s = adj_last_n.clone();
        let adj_presence_s = adj_presence.clone();
        let adj_freq_s = adj_freq.clone();
        let adj_gpu_s = adj_gpu.clone();
        let switch_mlock_s = switch_mlock.clone();
        let combo_ka_s = combo_keep_alive.clone();
        let tv_sys_save = text_view_sys.clone();
        let dialog_save = dialog.clone();
        let on_save_cb = on_variant_created_rc.clone();
        let tpl_save = template_holder.clone();

        btn_save.connect_clicked(move |_| {
            let start = tv_sys_save.buffer().start_iter();
            let end = tv_sys_save.buffer().end_iter();
            let sys_text = tv_sys_save.buffer().text(&start, &end, false).trim().to_string();

            let min_p_val = adj_min_p_s.value();
            let pred_val = adj_predict_s.value() as i32;
            let seed_val = adj_seed_s.value() as i64;
            let th_val = adj_threads_s.value() as u32;
            let pres_val = adj_presence_s.value();
            let freq_val = adj_freq_s.value();
            let rln_val = adj_last_n_s.value() as i32;
            let mlock_val = switch_mlock_s.is_active();
            let ka_idx = combo_ka_s.selected() as usize;
            let ka_values = ["5m", "15m", "30m", "1h", "24h", "-1", "0s"];
            let ka_str = ka_values.get(ka_idx).copied().unwrap_or("5m");

            let settings = CustomModelSettings {
                num_ctx: Some(adj_ctx_s.value() as u32),
                temperature: Some(adj_temp_s.value()),
                top_p: Some(adj_tp_s.value()),
                top_k: Some(adj_tk_s.value() as u32),
                min_p: if min_p_val > 0.0 { Some(min_p_val) } else { None },
                seed: if seed_val > 0 { Some(seed_val) } else { None },
                num_predict: if pred_val >= 0 { Some(pred_val) } else { None },
                num_thread: if th_val > 0 { Some(th_val) } else { None },
                repeat_penalty: Some(adj_pen_s.value()),
                repeat_last_n: if rln_val != 64 { Some(rln_val) } else { None },
                presence_penalty: if pres_val > 0.0 { Some(pres_val) } else { None },
                frequency_penalty: if freq_val > 0.0 { Some(freq_val) } else { None },
                num_gpu: Some(adj_gpu_s.value() as i32),
                use_mlock: if mlock_val { Some(true) } else { None },
                main_gpu: None,
                keep_alive: Some(ka_str.to_string()),
                system_prompt: if sys_text.is_empty() { None } else { Some(sys_text.clone()) },
            };

            cfg_for_save.borrow_mut().set_model_settings(model_for_save.clone(), settings);
            let _ = cfg_for_save.borrow().save();

            let tpl_opt = tpl_save.borrow().clone().or_else(|| {
                cfg_for_save.borrow().get_original_template(&model_for_save).map(|s| s.to_string())
            });

            let params = CustomModelfileParams {
                base_model: &model_for_save,
                template: tpl_opt.as_deref(),
                ctx: adj_ctx_s.value() as u32,
                temp: adj_temp_s.value(),
                top_p: adj_tp_s.value(),
                top_k: adj_tk_s.value() as u32,
                min_p: min_p_val,
                predict: pred_val,
                seed: seed_val,
                threads: th_val,
                repeat_penalty: adj_pen_s.value(),
                repeat_last_n: rln_val,
                presence_penalty: pres_val,
                frequency_penalty: freq_val,
                num_gpu: adj_gpu_s.value() as i32,
                use_mlock: mlock_val,
                system_text: &sys_text,
            };
            let modelfile = build_custom_modelfile(&params);

            on_save_cb(model_for_save.clone(), modelfile);
            dialog_save.close();
        });

        let cfg_for_reset = config_clone.clone();
        let model_for_reset = model_str.clone();
        let on_reset_cb = on_variant_created_rc.clone();
        let dialog_reset = dialog.clone();

        btn_reset.connect_clicked(move |_| {
            let mut cfg = cfg_for_reset.borrow_mut();
            let modelfile_to_restore = if let Some(orig_mf) = cfg.get_original_modelfile(&model_for_reset) {
                orig_mf.to_string()
            } else {
                format!("FROM {}\n", model_for_reset)
            };

            cfg.remove_model_settings(&model_for_reset);
            let _ = cfg.save();
            drop(cfg);

            on_reset_cb(model_for_reset.clone(), modelfile_to_restore);
            dialog_reset.close();
        });

        let base_model = model_str.clone();
        let dialog_create = dialog.clone();
        let entry_name = entry_new_name.clone();
        let adj_ctx_c = adj_ctx.clone();
        let adj_temp_c = adj_temp.clone();
        let adj_tp_c = adj_top_p.clone();
        let adj_tk_c = adj_top_k.clone();
        let adj_min_p_c = adj_min_p.clone();
        let adj_predict_c = adj_predict.clone();
        let adj_seed_c = adj_seed.clone();
        let adj_threads_c = adj_threads.clone();
        let adj_pen_c = adj_pen.clone();
        let adj_last_n_c = adj_last_n.clone();
        let adj_presence_c = adj_presence.clone();
        let adj_freq_c = adj_freq.clone();
        let adj_gpu_c = adj_gpu.clone();
        let switch_mlock_c = switch_mlock.clone();
        let tv_sys_c = text_view_sys.clone();
        let on_variant_cb = on_variant_created_rc.clone();
        let tpl_variant = template_holder.clone();

        btn_create_variant.connect_clicked(move |_| {
            let new_name = entry_name.text().trim().to_string();
            if new_name.is_empty() {
                return;
            }

            let start = tv_sys_c.buffer().start_iter();
            let end = tv_sys_c.buffer().end_iter();
            let sys_text = tv_sys_c.buffer().text(&start, &end, false).trim().to_string();

            let tpl_opt = tpl_variant.borrow().clone();
            let params = CustomModelfileParams {
                base_model: &base_model,
                template: tpl_opt.as_deref(),
                ctx: adj_ctx_c.value() as u32,
                temp: adj_temp_c.value(),
                top_p: adj_tp_c.value(),
                top_k: adj_tk_c.value() as u32,
                min_p: adj_min_p_c.value(),
                predict: adj_predict_c.value() as i32,
                seed: adj_seed_c.value() as i64,
                threads: adj_threads_c.value() as u32,
                repeat_penalty: adj_pen_c.value(),
                repeat_last_n: adj_last_n_c.value() as i32,
                presence_penalty: adj_presence_c.value(),
                frequency_penalty: adj_freq_c.value(),
                num_gpu: adj_gpu_c.value() as i32,
                use_mlock: switch_mlock_c.is_active(),
                system_text: &sys_text,
            };
            let modelfile = build_custom_modelfile(&params);

            on_variant_cb(new_name, modelfile);
            dialog_create.close();
        });

        let is_open = Rc::new(std::cell::Cell::new(true));
        let is_open_timer = is_open.clone();
        let up_tick = update_prediction.clone();

        up_tick();

        glib::timeout_add_local(Duration::from_millis(1000), move || {
            if !is_open_timer.get() {
                return glib::ControlFlow::Break;
            }
            up_tick();
            glib::ControlFlow::Continue
        });

        let is_open_close = is_open.clone();
        dialog.connect_closed(move |_| {
            is_open_close.set(false);
        });

        dialog.present(parent);
    }
}
