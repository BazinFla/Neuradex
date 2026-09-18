use adw::prelude::*;
use adw::Toast;
use crate::t;
use gtk4::{Align, Box, Button, Label, Orientation, Scale, SpinButton};
use std::cell::RefCell;
use std::rc::Rc;

/// Creates an Adw Toast with an icon-only Copy button (using `edit-copy-symbolic`)
/// that copies the toast message to the system clipboard without displaying text on the button.
pub fn create_copyable_toast(message: &str) -> Toast {
    let toast = Toast::new("");

    let box_container = Box::new(Orientation::Horizontal, 8);
    box_container.set_valign(Align::Center);

    let label = Label::builder()
        .label(message)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .valign(Align::Center)
        .build();
    box_container.append(&label);

    let btn_copy = Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text(t!("toasts.copy_tooltip"))
        .css_classes(["flat", "circular", "toast-copy-btn"])
        .valign(Align::Center)
        .build();

    let text = message.to_string();
    let btn_weak = btn_copy.downgrade();
    btn_copy.connect_clicked(move |_| {
        if let Some(display) = gtk4::gdk::Display::default() {
            display.clipboard().set_text(&text);
        }
        if let Some(btn) = btn_weak.upgrade() {
            btn.set_icon_name("emblem-ok-symbolic");
            btn.set_tooltip_text(Some(&t!("toasts.copied_tooltip")));
        }
    });

    box_container.append(&btn_copy);

    toast.set_custom_title(Some(&box_container));
    toast
}

/// Smoothly binds a `gtk4::Scale` and a `gtk4::SpinButton` bidirectionally
/// without infinite loops using a local synchronization lock.
pub fn bind_slider_and_spin(scale: &Scale, spin: &SpinButton) {
    let scale_adj = scale.adjustment();
    let spin_adj = spin.adjustment();

    if scale_adj == spin_adj {
        // If both widgets already share the same Adjustment instance, GTK manages synchronization
        return;
    }

    let is_syncing = Rc::new(RefCell::new(false));

    let is_syncing_spin = is_syncing.clone();
    let spin_adj_c = spin_adj.clone();
    scale_adj.connect_value_changed(move |adj| {
        if *is_syncing_spin.borrow() {
            return;
        }
        *is_syncing_spin.borrow_mut() = true;
        spin_adj_c.set_value(adj.value());
        *is_syncing_spin.borrow_mut() = false;
    });

    let is_syncing_scale = is_syncing.clone();
    let scale_adj_c = scale_adj.clone();
    spin_adj.connect_value_changed(move |adj| {
        if *is_syncing_scale.borrow() {
            return;
        }
        *is_syncing_scale.borrow_mut() = true;
        scale_adj_c.set_value(adj.value());
        *is_syncing_scale.borrow_mut() = false;
    });
}
