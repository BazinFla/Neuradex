use gtk4::prelude::*;
use gtk4::{Scale, SpinButton};
use std::cell::RefCell;
use std::rc::Rc;

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
