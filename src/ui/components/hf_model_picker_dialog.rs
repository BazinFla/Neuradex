use crate::api::hub_remote::HfRepoDetails;
use crate::api::types::ModelNameUtils;
use crate::core::hardware::estimator::format_bytes;
use crate::core::hardware::HardwareSnapshot;
use crate::core::hub::{HardwareFitness, HubManager};
use crate::t;
use adw::prelude::*;
use adw::{ActionRow, Clamp, Dialog, HeaderBar, PreferencesGroup, ToolbarView};
use gtk4::{
    Align, Box, Button, Image, Label, ListBox, Orientation, ScrolledWindow, SearchEntry,
};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

pub struct HfModelPickerDialog;

impl HfModelPickerDialog {
    pub fn show(
        parent: Option<&impl IsA<gtk4::Widget>>,
        details: HfRepoDetails,
        snap: &HardwareSnapshot,
        installed_set: &HashSet<String>,
        on_select_tag: impl Fn(String) + 'static,
    ) {
        let dialog = Dialog::builder()
            .title(format!("🤗 {}", details.model_name))
            .content_width(780)
            .content_height(640)
            .build();

        let toolbar_view = ToolbarView::new();
        let header_bar = HeaderBar::new();
        toolbar_view.add_top_bar(&header_bar);

        let main_scroller = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .build();

        let clamp = Clamp::builder()
            .maximum_size(740)
            .tightening_threshold(600)
            .margin_top(16)
            .margin_bottom(24)
            .margin_start(16)
            .margin_end(16)
            .build();

        let root_box = Box::new(Orientation::Vertical, 16);

        // 1. MODEL & HARDWARE METADATA BANNER
        let meta_card = Box::new(Orientation::Vertical, 10);
        meta_card.set_css_classes(&["card"]);
        meta_card.set_margin_bottom(4);

        let meta_inner = Box::new(Orientation::Vertical, 8);
        meta_inner.set_margin_top(14);
        meta_inner.set_margin_bottom(14);
        meta_inner.set_margin_start(16);
        meta_inner.set_margin_end(16);

        // Row 1: Title + Author + Web Link Button
        let title_row = Box::new(Orientation::Horizontal, 12);
        title_row.set_valign(Align::Center);

        let author_icon = Image::from_icon_name("network-workgroup-symbolic");
        author_icon.set_pixel_size(24);
        title_row.append(&author_icon);

        let title_vbox = Box::new(Orientation::Vertical, 2);
        title_vbox.set_hexpand(true);

        let title_label = Label::builder()
            .label(&details.model_name)
            .css_classes(["heading"])
            .halign(Align::Start)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .build();

        let author_label = Label::builder()
            .label(t!("hf_picker.by_author", author = details.author.clone()))
            .css_classes(["caption", "dim-label"])
            .halign(Align::Start)
            .build();

        title_vbox.append(&title_label);
        title_vbox.append(&author_label);
        title_row.append(&title_vbox);

        let page_url = details.page_url.clone();
        let btn_web = Button::builder()
            .label(t!("hf_picker.open_hf"))
            .css_classes(["flat"])
            .tooltip_text(t!("hf_picker.open_hf_tooltip"))
            .valign(Align::Center)
            .build();
        btn_web.connect_clicked(move |_| {
            if !page_url.is_empty() {
                let _ = gtk4::gio::AppInfo::launch_default_for_uri(
                    &page_url,
                    None::<&gtk4::gio::AppLaunchContext>,
                );
            }
        });
        title_row.append(&btn_web);
        meta_inner.append(&title_row);

        // Row 2: Badges (Downloads, Variants, Hardware)
        let badges_row = Box::new(Orientation::Horizontal, 8);
        badges_row.set_valign(Align::Center);

        let count_str = format_download_count(details.downloads);
        let downloads_badge = Label::builder()
            .label(t!("hf_picker.downloads_badge", count = count_str))
            .css_classes(["badge"])
            .tooltip_text(t!("hf_picker.downloads_tooltip"))
            .build();
        badges_row.append(&downloads_badge);

        let versions_badge = Label::builder()
            .label(t!("hf_picker.versions_count", count = details.files.len().to_string()))
            .css_classes(["badge"])
            .build();
        badges_row.append(&versions_badge);

        let spacer = Box::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        badges_row.append(&spacer);

        let (gpu_info, ram_info) = format_hardware_summary(snap);
        let hw_label = Label::builder()
            .label(format!("💻 {}  •  {}", gpu_info, ram_info))
            .css_classes(["caption", "dim-label"])
            .build();
        badges_row.append(&hw_label);

        meta_inner.append(&badges_row);
        meta_card.append(&meta_inner);
        root_box.append(&meta_card);

        // 2. SEARCH / QUICK FILTER
        let search_entry = SearchEntry::builder()
            .placeholder_text(t!("hf_picker.search_placeholder"))
            .margin_bottom(4)
            .build();
        root_box.append(&search_entry);

        // 3. GGUF VARIANTS LIST
        let list_group = PreferencesGroup::builder()
            .title(t!("hf_picker.variants_title"))
            .description(t!("hf_picker.variants_desc"))
            .build();

        let list_box = ListBox::builder()
            .css_classes(["boxed-list"])
            .selection_mode(gtk4::SelectionMode::None)
            .build();

        let on_select_rc: Rc<dyn Fn(String)> = Rc::new(on_select_tag);
        let rows_ref: Rc<RefCell<Vec<(String, ActionRow)>>> = Rc::new(RefCell::new(Vec::new()));

        for file in &details.files {
            let fitness = HubManager::evaluate_fitness(file.size_bytes, snap);
            let is_installed = ModelNameUtils::is_tag_installed(
                installed_set.iter().map(|s| s.as_str()),
                &file.tag,
            );

            let row = ActionRow::builder().build();

            let mut title_str = file.quantization.clone();
            if file.is_recommended {
                title_str.push_str(&format!("  {}", t!("hf_picker.recommended")));
            }
            if is_installed {
                title_str.push_str(&format!("  {}", t!("hf_picker.installed")));
            }
            row.set_title(&title_str);

            let mut sub_parts = Vec::new();
            if !file.description.is_empty() {
                sub_parts.push(file.description.clone());
            }
            if file.shard_count > 1 {
                sub_parts.push(t!("hf_picker.shards_combined", count = file.shard_count.to_string()));
            }
            row.set_subtitle(&sub_parts.join("  •  "));

            let icon_name = if is_installed {
                "emblem-ok-symbolic"
            } else if file.is_recommended {
                "emblem-favorite-symbolic"
            } else {
                "application-x-executable-symbolic"
            };
            let prefix_icon = Image::from_icon_name(icon_name);
            prefix_icon.set_pixel_size(20);
            row.add_prefix(&prefix_icon);

            let right_box = Box::new(Orientation::Horizontal, 8);
            right_box.set_valign(Align::Center);

            let size_label = Label::builder()
                .label(&file.size_formatted)
                .css_classes(["heading", "dim-label"])
                .valign(Align::Center)
                .build();
            right_box.append(&size_label);

            let fitness_badge = Label::builder()
                .label(fitness.badge_label())
                .css_classes(["badge"])
                .tooltip_text(fitness.tooltip())
                .valign(Align::Center)
                .build();
            right_box.append(&fitness_badge);

            let btn_download = Button::builder().valign(Align::Center).build();

            if is_installed {
                btn_download.set_label(&t!("hub.already_installed"));
                btn_download.set_css_classes(&["flat"]);
                btn_download.set_sensitive(false);
                btn_download.set_tooltip_text(Some(&t!("hub.already_installed_tooltip")));
            } else {
                btn_download.set_label(&t!("hub.download_btn"));
                if file.is_recommended || fitness == HardwareFitness::FullGpu {
                    btn_download.set_css_classes(&["suggested-action"]);
                } else {
                    btn_download.set_css_classes(&["flat"]);
                }

                let dialog_close = dialog.clone();
                let on_select = on_select_rc.clone();
                let target_tag = file.tag.clone();

                btn_download.connect_clicked(move |_| {
                    on_select(target_tag.clone());
                    dialog_close.close();
                });
            }

            right_box.append(&btn_download);
            row.add_suffix(&right_box);

            let filter_key = format!(
                "{} {} {} {}",
                file.quantization.to_lowercase(),
                file.filename.to_lowercase(),
                file.description.to_lowercase(),
                if is_installed { "installed" } else { "" }
            );

            rows_ref.borrow_mut().push((filter_key, row.clone()));
            list_box.append(&row);
        }

        let rows_filter = rows_ref.clone();
        search_entry.connect_search_changed(move |entry| {
            let query = entry.text().trim().to_lowercase();
            for (key, row) in rows_filter.borrow().iter() {
                if query.is_empty() || key.contains(&query) {
                    row.set_visible(true);
                } else {
                    row.set_visible(false);
                }
            }
        });

        list_group.add(&list_box);
        root_box.append(&list_group);

        clamp.set_child(Some(&root_box));
        main_scroller.set_child(Some(&clamp));
        toolbar_view.set_content(Some(&main_scroller));
        dialog.set_child(Some(&toolbar_view));

        dialog.present(parent);
    }
}

fn format_download_count(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}k", count as f64 / 1_000.0)
    } else {
        format!("{}", count)
    }
}

fn format_hardware_summary(snap: &HardwareSnapshot) -> (String, String) {
    let gpu_str = if let Some(ref g) = snap.gpu {
        format!("VRAM: {} / {}", format_bytes(g.vram_free), format_bytes(g.vram_total))
    } else {
        "GPU: N/A".to_string()
    };

    let ram_free = snap.cpu.ram_total.saturating_sub(snap.cpu.ram_used);
    let ram_str = format!("RAM: {} / {}", format_bytes(ram_free), format_bytes(snap.cpu.ram_total));

    (gpu_str, ram_str)
}
