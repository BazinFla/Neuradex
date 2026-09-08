use crate::core::logs::LogStreamer;
use crate::core::spawn_async;
use crate::t;
use adw::prelude::*;
use adw::{Clamp, PreferencesGroup};
use gtk4::{
    Box, Button, DropDown, Orientation, PolicyType, ScrolledWindow, SearchEntry,
    StringList, TextBuffer, TextTag, TextTagTable, TextView, ToggleButton, WrapMode,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct LogsView {
    container: ScrolledWindow,
    text_view: TextView,
    buffer: TextBuffer,
    search_entry: SearchEntry,
    btn_pause: ToggleButton,
    btn_clear: Button,
    btn_copy: Button,
    filter_dropdown: DropDown,
    is_paused: Rc<RefCell<bool>>,
    autoscroll: Rc<RefCell<bool>>,
    all_lines: Rc<RefCell<Vec<String>>>,
    stream_running: Arc<AtomicBool>,
}

impl Default for LogsView {
    fn default() -> Self {
        Self::new()
    }
}

impl LogsView {
    pub fn new() -> Self {
        let container = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .build();

        let root_box = Box::new(Orientation::Vertical, 0);

        // Toolbar / Filtering
        let toolbar_box = Box::new(Orientation::Horizontal, 8);
        toolbar_box.set_margin_start(16);
        toolbar_box.set_margin_end(16);
        toolbar_box.set_margin_top(12);
        toolbar_box.set_margin_bottom(12);

        // Search entry
        let search_entry = SearchEntry::builder()
            .placeholder_text(t!("logs.search_placeholder"))
            .hexpand(true)
            .can_focus(true)
            .focus_on_click(true)
            .build();
        toolbar_box.append(&search_entry);

        // Level dropdown
        let filter_levels_vec = [t!("logs.filters.all"),
            t!("logs.filters.gin"),
            t!("logs.filters.warn"),
            t!("logs.filters.error")];
        let filter_levels_refs: Vec<&str> = filter_levels_vec.iter().map(|s| s.as_str()).collect();
        let filter_levels = StringList::new(&filter_levels_refs);
        let filter_dropdown = DropDown::builder()
            .model(&filter_levels)
            .selected(0)
            .build();
        toolbar_box.append(&filter_dropdown);

        // Pause / Resume button
        let btn_pause = ToggleButton::builder()
            .icon_name("media-playback-pause-symbolic")
            .tooltip_text(t!("logs.pause_tooltip"))
            .build();
        toolbar_box.append(&btn_pause);

        // Clear button
        let btn_clear = Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text(t!("logs.clear_tooltip"))
            .build();
        toolbar_box.append(&btn_clear);

        // Copy button
        let btn_copy = Button::builder()
            .icon_name("edit-copy-symbolic")
            .tooltip_text(t!("logs.copy_tooltip"))
            .build();
        toolbar_box.append(&btn_copy);

        root_box.append(&toolbar_box);

        // Console text view
        let tag_table = TextTagTable::new();

        let tag_error = TextTag::builder()
            .name("error")
            .foreground("#ff5555")
            .weight(700)
            .build();
        let tag_warn = TextTag::builder()
            .name("warn")
            .foreground("#f1fa8c")
            .weight(700)
            .build();
        let tag_gin = TextTag::builder()
            .name("gin")
            .foreground("#8be9fd")
            .build();
        let tag_success = TextTag::builder()
            .name("success")
            .foreground("#50fa7b")
            .build();
        let tag_dim = TextTag::builder()
            .name("dim")
            .foreground("#6272a4")
            .build();

        tag_table.add(&tag_error);
        tag_table.add(&tag_warn);
        tag_table.add(&tag_gin);
        tag_table.add(&tag_success);
        tag_table.add(&tag_dim);

        let buffer = TextBuffer::new(Some(&tag_table));

        let text_view = TextView::builder()
            .buffer(&buffer)
            .editable(false)
            .cursor_visible(false)
            .monospace(true)
            .wrap_mode(WrapMode::Word)
            .vexpand(true)
            .hexpand(true)
            .top_margin(12)
            .bottom_margin(12)
            .left_margin(16)
            .right_margin(16)
            .css_classes(["console-view"])
            .build();

        let scroll_console = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Automatic)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .child(&text_view)
            .build();

        let clamp = Clamp::builder()
            .maximum_size(1400)
            .tightening_threshold(1100)
            .child(&scroll_console)
            .vexpand(true)
            .hexpand(true)
            .margin_start(16)
            .margin_end(16)
            .margin_bottom(16)
            .build();

        let pref_group = PreferencesGroup::builder()
            .title(t!("logs.title"))
            .description(t!("logs.description"))
            .build();
        pref_group.add(&clamp);

        root_box.append(&pref_group);
        container.set_child(Some(&root_box));

        let is_paused = Rc::new(RefCell::new(false));
        let autoscroll = Rc::new(RefCell::new(true));
        let all_lines = Rc::new(RefCell::new(Vec::new()));
        let stream_running = Arc::new(AtomicBool::new(true));

        let logs_view = Self {
            container,
            text_view,
            buffer,
            search_entry,
            btn_pause,
            btn_clear,
            btn_copy,
            filter_dropdown,
            is_paused,
            autoscroll,
            all_lines,
            stream_running,
        };

        logs_view.setup_events(&scroll_console);
        logs_view.start_log_stream();

        logs_view
    }

    pub fn widget(&self) -> &ScrolledWindow {
        &self.container
    }

    fn setup_events(&self, _scroll_console: &ScrolledWindow) {
        // Pause / Resume
        let paused_flag = self.is_paused.clone();
        let btn_pause_clone = self.btn_pause.clone();
        self.btn_pause.connect_toggled(move |btn| {
            let active = btn.is_active();
            *paused_flag.borrow_mut() = active;
            if active {
                btn_pause_clone.set_icon_name("media-playback-start-symbolic");
                btn_pause_clone.set_tooltip_text(Some(&t!("logs.resume_tooltip")));
            } else {
                btn_pause_clone.set_icon_name("media-playback-pause-symbolic");
                btn_pause_clone.set_tooltip_text(Some(&t!("logs.pause_tooltip")));
            }
        });

        // Clear
        let lines_clear = self.all_lines.clone();
        let buf_clear = self.buffer.clone();
        self.btn_clear.connect_clicked(move |_| {
            lines_clear.borrow_mut().clear();
            buf_clear.set_text("");
        });

        // Copy
        let buf_copy = self.buffer.clone();
        self.btn_copy.connect_clicked(move |_| {
            let start = buf_copy.start_iter();
            let end = buf_copy.end_iter();
            let text = buf_copy.text(&start, &end, false);
            if let Some(display) = gtk4::gdk::Display::default() {
                display.clipboard().set_text(&text);
            }
        });

        // Search & Filtering
        let view_lines = self.all_lines.clone();
        let view_buf = self.buffer.clone();
        let view_search = self.search_entry.clone();
        let view_filter = self.filter_dropdown.clone();

        let reapply = move || {
            let query = view_search.text().to_lowercase();
            let filter_idx = view_filter.selected();
            view_buf.set_text("");

            for line in view_lines.borrow().iter() {
                if Self::matches_filter(line, &query, filter_idx) {
                    Self::append_styled_line(&view_buf, line);
                }
            }
        };

        let reapply_search = reapply.clone();
        self.search_entry.connect_search_changed(move |_| {
            reapply_search();
        });

        let reapply_filter = reapply.clone();
        self.filter_dropdown.connect_selected_notify(move |_| {
            reapply_filter();
        });
    }

    fn matches_filter(line: &str, query: &str, filter_idx: u32) -> bool {
        if !query.is_empty() && !line.to_lowercase().contains(query) {
            return false;
        }

        match filter_idx {
            1 => line.contains("[GIN]"),
            2 => line.contains("error") || line.contains("WARN") || line.contains("FAIL") || line.contains("Error"),
            3 => line.contains("GPU") || line.contains("ggml") || line.contains("llama") || line.contains("runner") || line.contains("model"),
            _ => true,
        }
    }

    fn append_styled_line(buffer: &TextBuffer, line: &str) {
        let mut end_iter = buffer.end_iter();
        let tag_name = if line.contains("error") || line.contains("Error") || line.contains("FAIL") {
            Some("error")
        } else if line.contains("warn") || line.contains("WARN") {
            Some("warn")
        } else if line.contains("[GIN]") {
            Some("gin")
        } else if line.contains("200") && line.contains("[GIN]") {
            Some("success")
        } else {
            None
        };

        let text_with_nl = format!("{}\n", line);
        if let Some(tag) = tag_name {
            buffer.insert_with_tags_by_name(&mut end_iter, &text_with_nl, &[tag]);
        } else {
            buffer.insert(&mut end_iter, &text_with_nl);
        }
    }

    fn start_log_stream(&self) {
        let (tx, rx) = async_channel::bounded::<String>(200);
        let running = self.stream_running.clone();

        spawn_async(
            async move {
                LogStreamer::stream_logs(tx, running).await;
            },
            |_| {},
        );

        let buffer = self.buffer.clone();
        let text_view = self.text_view.clone();
        let all_lines = self.all_lines.clone();
        let is_paused = self.is_paused.clone();
        let autoscroll = self.autoscroll.clone();
        let search_ent = self.search_entry.clone();
        let filter_drop = self.filter_dropdown.clone();

        glib::spawn_future_local(async move {
            while let Ok(line) = rx.recv().await {
                all_lines.borrow_mut().push(line.clone());
                if all_lines.borrow().len() > 1500 {
                    all_lines.borrow_mut().remove(0);
                }

                if !*is_paused.borrow() {
                    let query = search_ent.text().to_lowercase();
                    let filter_idx = filter_drop.selected();
                    if Self::matches_filter(&line, &query, filter_idx) {
                        Self::append_styled_line(&buffer, &line);

                        if *autoscroll.borrow() {
                            let end_iter = buffer.end_iter();
                            let mark = buffer.create_mark(None, &end_iter, false);
                            text_view.scroll_to_mark(&mark, 0.0, true, 0.0, 1.0);
                        }
                    }
                }
            }
        });
    }
}

impl Drop for LogsView {
    fn drop(&mut self) {
        self.stream_running.store(false, Ordering::SeqCst);
    }
}
