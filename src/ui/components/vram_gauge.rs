use crate::core::hardware::estimator::{format_bytes, format_gib};
use crate::core::hardware::GpuMetrics;
use crate::t;
use gtk4::prelude::*;
use gtk4::{Align, Box, Label, LevelBar, Orientation};
use std::cell::RefCell;

struct SingleGpuCard {
    container: Box,
    label_title: Label,
    label_value: Label,
    level_bar: LevelBar,
    label_meta: Label,
}

impl SingleGpuCard {
    fn new(_is_subcard: bool) -> Self {
        let container = Box::new(Orientation::Vertical, 6);
        container.set_css_classes(&["card"]);
        container.set_margin_start(2);
        container.set_margin_end(2);
        container.set_margin_top(2);
        container.set_margin_bottom(2);

        let inner = Box::new(Orientation::Vertical, 6);
        inner.set_margin_start(12);
        inner.set_margin_end(12);
        inner.set_margin_top(10);
        inner.set_margin_bottom(10);
        container.append(&inner);

        let top_box = Box::new(Orientation::Horizontal, 12);
        top_box.set_hexpand(true);

        let label_title = Label::builder()
            .label(t!("instances.vram_dedicated_gpu"))
            .css_classes(["heading"])
            .halign(Align::Start)
            .hexpand(true)
            .build();

        let label_value = Label::builder()
            .label("0.0 / 0.0 GB (0%)")
            .css_classes(["numeric", "heading"])
            .halign(Align::End)
            .build();

        top_box.append(&label_title);
        top_box.append(&label_value);

        let level_bar = LevelBar::builder()
            .min_value(0.0)
            .max_value(1.0)
            .value(0.0)
            .height_request(10)
            .build();

        let label_meta = Label::builder()
            .label(t!("instances.gpu_detecting"))
            .css_classes(["dim-label", "caption"])
            .halign(Align::Start)
            .build();

        inner.append(&top_box);
        inner.append(&level_bar);
        inner.append(&label_meta);

        Self {
            container,
            label_title,
            label_value,
            level_bar,
            label_meta,
        }
    }


    fn update(&self, g: &GpuMetrics, prefix: Option<&str>) {
        let total = g.vram_total;
        let used = g.vram_used;
        let ratio = if total > 0 {
            (used as f64 / total as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let title_text = if let Some(p) = prefix {
            format!("{} : {}", p, g.name)
        } else {
            format!("VRAM : {}", g.name)
        };

        self.label_title.set_label(&title_text);
        self.label_value.set_label(&format!(
            "{} / {} ({:.0}%)",
            format_gib(used),
            format_gib(total),
            ratio * 100.0
        ));
        self.level_bar.set_value(ratio);

        let mut meta_parts = Vec::new();
        if let Some(t) = g.temperature_c {
            meta_parts.push(format!("🌡️ {}°C", t));
        }
        if let Some(u) = g.utilization_percent {
            meta_parts.push(t!("instances.gpu_load", load = u));
        }
        if let Some(p) = g.power_watts {
            meta_parts.push(format!("🔌 {:.0} W", p));
        }

        if meta_parts.is_empty() {
            self.label_meta.set_label(&t!("instances.vram_free", free = format_bytes(g.vram_free)));
        } else {
            self.label_meta.set_label(&meta_parts.join("   •   "));
        }
    }

    fn set_empty(&self) {
        self.label_title.set_label(&t!("instances.vram_dedicated"));
        self.label_value.set_label(&t!("instances.not_detected"));
        self.level_bar.set_value(0.0);
        self.label_meta.set_label(&t!("instances.cpu_mode_only"));
    }
}

pub struct VramGauge {
    container: Box,
    rendered_count: RefCell<usize>,
    subcards: RefCell<Vec<SingleGpuCard>>,
    summary_card: RefCell<Option<SingleGpuCard>>,
}

impl Default for VramGauge {
    fn default() -> Self {
        Self::new()
    }
}

impl VramGauge {
    pub fn new() -> Self {
        let container = Box::new(Orientation::Vertical, 6);
        container.set_hexpand(true);

        let initial_card = SingleGpuCard::new(false);
        container.append(&initial_card.container);

        Self {
            container,
            rendered_count: RefCell::new(1),
            subcards: RefCell::new(vec![initial_card]),
            summary_card: RefCell::new(None),
        }
    }

    pub fn widget(&self) -> &Box {
        &self.container
    }

    /// Updates display for a slice of detected GPUs (and optional combined aggregate)
    pub fn update_gpus(&self, gpus: &[GpuMetrics], aggregate: Option<&GpuMetrics>) {
        let count = gpus.len();

        if *self.rendered_count.borrow() != count {
            // Rebuild UI layout to fit the new topology (0, 1, or N GPUs)
            while let Some(child) = self.container.first_child() {
                self.container.remove(&child);
            }
            self.subcards.borrow_mut().clear();
            *self.summary_card.borrow_mut() = None;

            if count <= 1 {
                let card = SingleGpuCard::new(false);
                self.container.append(&card.container);
                self.subcards.borrow_mut().push(card);
            } else {
                // Multi-GPU layout: Summary header card + individual distinct GPU cards
                let sum_card = SingleGpuCard::new(true);
                self.container.append(&sum_card.container);
                *self.summary_card.borrow_mut() = Some(sum_card);

                for _ in 0..count {
                    let card = SingleGpuCard::new(true);
                    self.container.append(&card.container);
                    self.subcards.borrow_mut().push(card);
                }
            }

            *self.rendered_count.borrow_mut() = count;
        }

        if count == 0 {
            if let Some(card) = self.subcards.borrow().first() {
                card.set_empty();
            }
        } else if count == 1 {
            if let (Some(card), Some(gpu)) = (self.subcards.borrow().first(), gpus.first()) {
                card.update(gpu, None);
            }
        } else {
            // Update Summary Card
            if let Some(ref sum_card) = *self.summary_card.borrow() {
                if let Some(agg) = aggregate {
                    let pool_title = t!("instances.gpu_pool_title", count = count.to_string());
                    sum_card.update(agg, Some(&pool_title));
                }
            }

            // Update Individual GPU Cards
            let cards = self.subcards.borrow();
            for (idx, (card, gpu)) in cards.iter().zip(gpus.iter()).enumerate() {
                let gpu_prefix = format!("GPU {}", idx);
                card.update(gpu, Some(&gpu_prefix));
            }
        }
    }

    /// Legacy single-GPU update method
    pub fn update(&self, gpu: Option<&GpuMetrics>) {
        if let Some(g) = gpu {
            self.update_gpus(std::slice::from_ref(g), Some(g));
        } else {
            self.update_gpus(&[], None);
        }
    }
}

