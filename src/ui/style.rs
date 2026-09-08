use gtk4::gdk::Display;
use gtk4::CssProvider;

pub fn load_custom_styles() {
    let provider = CssProvider::new();
    let css = r#"
        /* ==========================================================
           NeuraDex - Design System & Custom Libadwaita Styles
           ========================================================== */

        /* HeaderBar Brand & Badges */
        .brand-chip {
            padding: 2px 4px;
            background: transparent;
            color: @window_fg_color;
            font-weight: 800;
            font-size: 14px;
            letter-spacing: 0.3px;
            margin-left: 2px;
        }

        .daemon-badge {
            padding: 3px 10px;
            border-radius: 14px;
            font-weight: 700;
            font-size: 12px;
            transition: all 200ms ease;
            border: 1px solid transparent;
        }

        .daemon-badge.connected {
            background: alpha(#2ec27e, 0.15);
            color: #26a269;
            border-color: alpha(#2ec27e, 0.3);
        }

        .daemon-badge.disconnected {
            background: alpha(#e01b24, 0.15);
            color: #c01c28;
            border-color: alpha(#e01b24, 0.3);
        }

        .daemon-badge.connecting {
            background: alpha(#e5a50a, 0.15);
            color: #c67800;
            border-color: alpha(#e5a50a, 0.3);
        }

        /* Profile DropDown Styling */
        .profile-dropdown-pill {
            border-radius: 12px;
            font-weight: 600;
            font-size: 12px;
            padding: 2px 4px;
            background: alpha(@window_fg_color, 0.05);
            border: 1px solid alpha(@window_fg_color, 0.08);
            transition: all 150ms ease;
        }

        .profile-dropdown-pill:hover {
            background: alpha(@accent_color, 0.12);
            border-color: alpha(@accent_color, 0.25);
        }

        /* API Key Profile Test Button States */
        button.key-test-btn {
            border-radius: 8px;
            transition: all 180ms ease;
        }

        button.key-test-btn.default {
            color: @window_fg_color;
        }

        button.key-test-btn.valid {
            color: #26a269;
            background: alpha(#2ec27e, 0.18);
        }

        button.key-test-btn.valid:hover {
            background: alpha(#2ec27e, 0.28);
        }

        button.key-test-btn.invalid {
            color: #e01b24;
            background: alpha(#e01b24, 0.18);
        }

        button.key-test-btn.invalid:hover {
            background: alpha(#e01b24, 0.28);
        }

        /* Download Popover & Task Cards */
        .download-popover-card {
            background: alpha(@window_fg_color, 0.04);
            border: 1px solid alpha(@window_fg_color, 0.08);
            border-radius: 12px;
            padding: 10px 12px;
            margin-bottom: 6px;
            transition: all 180ms ease;
        }

        .download-popover-card:hover {
            background: alpha(@window_fg_color, 0.07);
            border-color: alpha(@accent_color, 0.25);
        }

        .download-popover-card.finished {
            background: alpha(#2ec27e, 0.06);
            border-color: alpha(#2ec27e, 0.20);
        }

        .download-popover-card.failed {
            background: alpha(#e01b24, 0.06);
            border-color: alpha(#e01b24, 0.20);
        }

        .download-metrics-label {
            font-family: monospace, sans-serif;
            font-size: 11px;
            font-weight: 600;
            color: @accent_color;
        }

        .download-metrics-label.finished {
            color: #26a269;
        }

        .download-metrics-label.failed {
            color: #e01b24;
        }

        .download-percent-pill {
            font-family: monospace;
            font-size: 11px;
            font-weight: 700;
            padding: 2px 7px;
            border-radius: 8px;
            background: alpha(@accent_color, 0.15);
            color: @accent_color;
        }

        .download-percent-pill.finished {
            background: alpha(#2ec27e, 0.18);
            color: #26a269;
        }

        .download-percent-pill.failed {
            background: alpha(#e01b24, 0.18);
            color: #c01c28;
        }

        .download-progress-bar progress,
        .download-progress-bar trough {
            min-height: 6px;
            border-radius: 3px;
        }

        .download-progress-bar progress {
            background-color: @accent_bg_color;
        }

        /* Running / Memory Resident Model Cards */
        .running-model-card {
            background: alpha(#2ec27e, 0.05);
            border: 1px solid alpha(#2ec27e, 0.25);
            border-left: 4px solid #2ec27e;
            border-radius: 12px;
            padding: 12px 14px;
            margin-bottom: 6px;
            transition: all 180ms ease;
        }

        .running-model-card:hover {
            background: alpha(#2ec27e, 0.08);
            border-color: alpha(#2ec27e, 0.40);
        }

        .running-model-card.hybrid {
            background: alpha(#e5a50a, 0.05);
            border-color: alpha(#e5a50a, 0.25);
            border-left-color: #e5a50a;
        }

        .running-model-card.hybrid:hover {
            background: alpha(#e5a50a, 0.08);
            border-color: alpha(#e5a50a, 0.40);
        }

        .running-model-card.loading {
            background: alpha(@window_fg_color, 0.035);
            border: 1px dashed alpha(@accent_color, 0.45);
            border-left: 4px solid @accent_color;
            opacity: 0.88;
            transition: all 200ms ease;
        }

        .running-model-card.loading:hover {
            background: alpha(@window_fg_color, 0.06);
            border-color: @accent_color;
        }

        .running-alloc-chip {
            font-family: monospace, sans-serif;
            font-size: 11px;
            font-weight: 700;
            padding: 3px 8px;
            border-radius: 8px;
            background: alpha(#2ec27e, 0.18);
            color: #26a269;
        }

        .running-alloc-chip.hybrid {
            background: alpha(#e5a50a, 0.18);
            color: #c67c00;
        }

        .running-alloc-chip.loading {
            background: alpha(@accent_color, 0.16);
            color: @accent_color;
        }

        .running-expire-chip {
            font-size: 11px;
            font-weight: 500;
            padding: 2px 7px;
            border-radius: 6px;
            background: alpha(@window_fg_color, 0.06);
            color: alpha(@window_fg_color, 0.75);
        }

        .running-vram-bar progress,
        .running-vram-bar trough {
            min-height: 5px;
            border-radius: 3px;
        }

        .running-vram-bar progress {
            background-color: #2ec27e;
        }

        .running-vram-bar.hybrid progress {
            background-color: #e5a50a;
        }

        .running-vram-bar.loading progress {
            background-color: @accent_bg_color;
        }


        /* Empty State Running Memory Card */
        .running-empty-state-card {
            background: alpha(@window_fg_color, 0.02);
            border: 1px dashed alpha(@window_fg_color, 0.14);
            border-radius: 12px;
            padding: 22px 16px;
            margin: 6px 2px;
            transition: all 180ms ease;
        }

        .running-empty-state-card:hover {
            background: alpha(@window_fg_color, 0.035);
            border-color: alpha(@window_fg_color, 0.22);
        }

        .empty-state-icon-badge {
            background: alpha(@accent_color, 0.10);
            color: @accent_color;
            border-radius: 50%;
            padding: 10px;
            margin-bottom: 6px;
        }

        .empty-state-tip-pill {
            font-size: 11px;
            font-weight: 500;
            color: alpha(@window_fg_color, 0.65);
            background: alpha(@window_fg_color, 0.04);
            padding: 4px 10px;
            border-radius: 12px;
        }

        /* Cards & Metric Badges */
        .metric-value {
            font-family: monospace, sans-serif;
            font-weight: 700;
            letter-spacing: -0.3px;
        }

        /* Category Filter Pills & FlowBox */
        flowbox.category-filter-box {
            background: transparent;
        }

        flowbox.category-filter-box flowboxchild {
            padding: 0;
            margin: 0;
            background: transparent;
            border-radius: 9999px;
        }

        flowbox.category-filter-box flowboxchild:selected,
        flowbox.category-filter-box flowboxchild:focus {
            background: transparent;
            outline: none;
            box-shadow: none;
        }

        .category-pill,
        button.category-pill,
        togglebutton.category-pill,
        .pill-button {
            border-radius: 9999px;
            padding: 6px 14px;
            font-size: 12.5px;
            font-weight: 600;
            margin: 0px;
            transition: all 180ms cubic-bezier(0.25, 1, 0.5, 1);
            background: alpha(@window_fg_color, 0.05);
            color: alpha(@window_fg_color, 0.85);
            border: 1px solid alpha(@window_fg_color, 0.10);
            box-shadow: 0 1px 2px alpha(#000, 0.04);
        }

        .category-pill:hover,
        button.category-pill:hover,
        togglebutton.category-pill:hover,
        .pill-button:hover {
            background: alpha(@accent_color, 0.12);
            color: @accent_color;
            border-color: alpha(@accent_color, 0.30);
            box-shadow: 0 2px 4px alpha(#000, 0.06);
        }

        .category-pill:checked,
        .category-pill:active,
        button.category-pill:checked,
        togglebutton.category-pill:checked,
        .pill-button:checked {
            background: @accent_bg_color;
            color: @accent_fg_color;
            border-color: @accent_bg_color;
            font-weight: 700;
            box-shadow: 0 2px 6px alpha(@accent_color, 0.35);
        }


        /* Catalog Model List (Clean Transparent Wrapper) */
        list.catalog-model-list {
            background: transparent;
            border: none;
            box-shadow: none;
        }

        list.catalog-model-list > row {
            background: transparent;
            padding: 0;
            margin: 0;
            border: none;
            outline: none;
            box-shadow: none;
        }

        /* Hub Model Card Title Link */

        .hub-model-title-link {
            padding: 0;
            margin: 0;
            background: transparent;
            border: none;
            box-shadow: none;
            min-height: 0;
            min-width: 0;
        }

        .hub-model-title-link label {
            color: @window_fg_color;
            font-weight: 700;
            font-size: 15px;
            transition: color 150ms ease, text-decoration 150ms ease;
        }

        .hub-model-title-link:hover label {
            color: @accent_color;
            text-decoration: underline;
        }

        /* Console & Logs View */
        .console-view {
            background-color: #1e1e2e;
            color: #cdd6f4;
            font-family: "JetBrains Mono", "Fira Code", "Source Code Pro", monospace;
            font-size: 12px;
            line-height: 1.45;
            border-radius: 12px;
        }

        /* ==========================================================
           Chat & Playground View Styles
           ========================================================== */
        .chat-user-bubble {
            background-color: @accent_bg_color;
            color: @accent_fg_color;
            border-radius: 16px 16px 4px 16px;
            padding: 12px 16px;
            font-size: 14px;
            line-height: 1.5;
            box-shadow: 0 2px 6px alpha(#000, 0.08);
        }

        .chat-assistant-bubble {
            background-color: alpha(@window_fg_color, 0.06);
            border: 1px solid alpha(@window_fg_color, 0.08);
            border-radius: 16px 16px 16px 4px;
            padding: 14px 18px;
            font-size: 14px;
            line-height: 1.55;
            box-shadow: 0 1px 4px alpha(#000, 0.04);
        }

        .chat-role-label {
            font-weight: 700;
            font-size: 12px;
            letter-spacing: 0.3px;
        }

        .chat-metric-chip {
            padding: 3px 8px;
            border-radius: 8px;
            font-family: "JetBrains Mono", "Fira Code", monospace;
            font-size: 11px;
            font-weight: 600;
            background: alpha(@window_fg_color, 0.07);
            border: 1px solid alpha(@window_fg_color, 0.08);
        }

        .chat-metric-chip.speed {
            color: #26a269;
            background: alpha(#2ec27e, 0.12);
            border-color: alpha(#2ec27e, 0.25);
        }

        .chat-metric-chip.ttft {
            color: #3584e4;
            background: alpha(#3584e4, 0.12);
            border-color: alpha(#3584e4, 0.25);
        }

        .chat-metric-chip.vram {
            color: #9141ac;
            background: alpha(#9141ac, 0.12);
            border-color: alpha(#9141ac, 0.25);
        }

        .chat-input-bar {
            background-color: alpha(@window_fg_color, 0.05);
            border: 1px solid alpha(@window_fg_color, 0.14);
            border-radius: 24px;
            padding: 4px 6px 4px 12px;
            min-height: 48px;
            box-shadow: 0 1px 3px alpha(#000, 0.04);
            transition: all 180ms ease;
        }

        .chat-input-bar:focus-within {
            border-color: @accent_color;
            box-shadow: 0 0 0 2px alpha(@accent_color, 0.25), 0 2px 6px alpha(#000, 0.06);
            background-color: alpha(@window_fg_color, 0.08);
        }

        .chat-input-bar scrolledwindow,
        .chat-input-bar scrolledwindow viewport {
            background: transparent;
            background-color: transparent;
            border: none;
            box-shadow: none;
            outline: none;
        }

        .chat-input-bar textview {
            background: transparent;
            background-color: transparent;
            border: none;
            box-shadow: none;
            outline: none;
        }

        .chat-input-bar textview text {
            background: transparent;
            background-color: transparent;
            color: @window_fg_color;
            font-size: 14px;
            line-height: 1.45;
            padding: 0;
            margin: 0;
        }

        .chat-input-bar textview:focus {
            box-shadow: none;
            outline: none;
        }

        .chat-action-btn {
            min-width: 36px;
            min-height: 36px;
            border-radius: 18px;
            padding: 0;
            margin: 2px 2px;
        }

        .chat-suggestion-chip {
            border-radius: 12px;
            padding: 8px 14px;
            font-size: 13px;
            font-weight: 500;
            background: alpha(@window_fg_color, 0.04);
            border: 1px solid alpha(@window_fg_color, 0.08);
            transition: all 150ms ease;
        }

        .chat-suggestion-chip:hover {
            background: alpha(@accent_color, 0.12);
            border-color: alpha(@accent_color, 0.3);
        }

        .chat-sidebar {
            background-color: alpha(@window_fg_color, 0.02);
            border-right: 1px solid alpha(@window_fg_color, 0.08);
            min-width: 240px;
        }

        .chat-history-row {
            padding: 6px 8px;
            border-radius: 10px;
            margin: 2px 4px;
            transition: all 120ms ease;
        }

        .chat-history-row:hover {
            background-color: alpha(@window_fg_color, 0.06);
        }

        .chat-history-row:selected,
        .chat-history-row.active {
            background-color: alpha(@accent_color, 0.15);
        }

        .chat-delete-btn {
            opacity: 0.5;
            padding: 4px;
            min-width: 28px;
            min-height: 28px;
            border-radius: 14px;
            transition: all 120ms ease;
        }

        .chat-delete-btn:hover {
            opacity: 1.0;
            color: #e01b24;
            background-color: alpha(#e01b24, 0.15);
        }

        /* ==========================================================
           Custom Dropdown & Popover Styling (Highlight selected row instead of checkmark)
           ========================================================== */
        dropdown popover listview row image.checkmark,
        dropdown popover listview row check,
        dropdown popover listview row checkbutton,
        popover.menu check,
        popover.menu image.checkmark {
            opacity: 0;
            min-width: 0;
            min-height: 0;
        }

        dropdown popover listview row:selected,
        dropdown popover listview row.selected,
        dropdown popover listview row:checked {
            background-color: alpha(@accent_color, 0.15);
            color: @accent_color;
            border-radius: 8px;
            font-weight: 600;
        }

        dropdown popover listview row:selected label,
        dropdown popover listview row.selected label,
        dropdown popover listview row:checked label {
            color: @accent_color;
        }

        dropdown popover listview row:selected image,
        dropdown popover listview row.selected image,
        dropdown popover listview row:checked image {
            color: @accent_color;
        }

        dropdown popover listview row:selected:hover,
        dropdown popover listview row.selected:hover,
        dropdown popover listview row:checked:hover {
            background-color: alpha(@accent_color, 0.22);
            color: @accent_color;
        }

        dropdown popover listview row:hover {
            background-color: alpha(@accent_color, 0.08);
            border-radius: 8px;
        }

        /* ==========================================================
           Action Buttons Contrast Styling (Light & Dark Theme Safety)
           ========================================================== */
        button.suggested-action.flat {
            color: @accent_color;
            background-color: alpha(@accent_color, 0.12);
            border: 1px solid alpha(@accent_color, 0.25);
        }

        button.suggested-action.flat:hover {
            background-color: alpha(@accent_color, 0.22);
            border-color: alpha(@accent_color, 0.40);
            color: @accent_color;
        }

        button.suggested-action.flat:active {
            background-color: alpha(@accent_color, 0.32);
            border-color: alpha(@accent_color, 0.50);
            color: @accent_color;
        }

        button.destructive-action.flat {
            color: @destructive_color;
            background-color: alpha(@destructive_color, 0.12);
            border: 1px solid alpha(@destructive_color, 0.25);
        }

        button.destructive-action.flat:hover {
            background-color: alpha(@destructive_color, 0.22);
            border-color: alpha(@destructive_color, 0.40);
            color: @destructive_color;
        }

        button.destructive-action.flat:active {
            background-color: alpha(@destructive_color, 0.32);
            border-color: alpha(@destructive_color, 0.50);
            color: @destructive_color;
        }

        /* Model Card Inline Delete Confirmation Button */
        button.model-delete-confirm-btn {
            padding: 2px 10px;
            min-height: 32px;
            border-radius: 8px;
            background-color: alpha(@destructive_color, 0.16);
            border: 1px solid alpha(@destructive_color, 0.40);
            color: @destructive_color;
            transition: all 160ms ease-in-out;
        }

        button.model-delete-confirm-btn:hover {
            background-color: alpha(@destructive_color, 0.28);
            border-color: alpha(@destructive_color, 0.60);
            color: @destructive_color;
        }

        button.model-delete-confirm-btn:active {
            background-color: alpha(@destructive_color, 0.45);
            border-color: @destructive_color;
            color: #ffffff;
        }

        .model-delete-confirm-top {
            font-size: 10px;
            font-weight: 500;
            line-height: 1.1;
            opacity: 0.90;
        }

        .model-delete-confirm-bottom {
            font-size: 12.5px;
            font-weight: 800;
            line-height: 1.1;
        }
    "#;

    provider.load_from_string(css);

    if let Some(display) = Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
