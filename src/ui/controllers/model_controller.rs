use crate::api::client::OllamaClient;
use crate::api::types::PullProgress;
use crate::core::config::CustomModelSettings;
use crate::core::spawn_async;
use crate::t;
use crate::ui::header::Header;
use crate::ui::helpers::create_copyable_toast;
use crate::ui::views::{HubView, InstancesView};
use adw::ToastOverlay;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;


/// Controller managing asynchronous model actions (VRAM Loading, Unloading, Downloading, Compiling, and Deletion)
pub struct ModelController;

impl ModelController {
    /// Loads or unloads a model into/from VRAM in Ollama
    pub fn load_or_unload_model<F>(
        client: OllamaClient,
        view: Rc<InstancesView>,
        toast_overlay: ToastOverlay,
        model: &str,
        unload: bool,
        custom_settings: Option<CustomModelSettings>,
        on_finished: F,
    ) where
        F: Fn(bool) + 'static,
    {
        let t = toast_overlay.clone();
        let v = view.clone();
        let m = model.to_string();

        let (options, system, keep_alive_val) = if let Some(ref s) = custom_settings {
            let ka = s.keep_alive.clone().unwrap_or_else(|| "30m".to_string());
            (Some(s.to_options_map()), s.system_prompt.clone(), ka)
        } else {
            (None, None, "30m".to_string())
        };

        spawn_async(
            async move {
                let action_res = if unload {
                    client.unload_model(&m).await
                } else {
                    client.load_model_with_options(&m, &keep_alive_val, options, system).await
                };
                (m, action_res, unload)
            },
            move |(m, action_res, was_unload)| {
                v.set_model_loading(&m, false);
                match action_res {
                    Ok(_) => {
                        if was_unload {
                            t.add_toast(create_copyable_toast(&t!("toasts.vram_freed", name = m)));
                        } else {
                            t.add_toast(create_copyable_toast(&t!("toasts.model_loaded", name = m)));
                        }
                        on_finished(true);
                    }
                    Err(e) => {
                        if was_unload {
                            t.add_toast(create_copyable_toast(&t!("toasts.vram_flush_error", err = e)));
                        } else {
                            t.add_toast(create_copyable_toast(&t!("toasts.load_error", err = e)));
                        }
                        on_finished(false);
                    }
                }
            },
        );
    }

    /// Deletes a local model from Ollama
    pub fn delete_model<F>(
        client: OllamaClient,
        toast_overlay: ToastOverlay,
        model: &str,
        on_finished: F,
    ) where
        F: Fn(bool) + 'static,
    {
        let t = toast_overlay.clone();
        let m = model.to_string();

        spawn_async(
            async move {
                let del_res = client.delete_model(&m).await;
                (m, del_res)
            },
            move |(m, res)| match res {
                Ok(_) => {
                    t.add_toast(create_copyable_toast(&t!("toasts.model_deleted", name = m)));
                    on_finished(true);
                }
                Err(e) => {
                    t.add_toast(create_copyable_toast(&t!("toasts.delete_error", err = e)));
                    on_finished(false);
                }
            },
        );
    }

    /// Downloads a model from the Ollama Hub with progress streaming, percentage calculation, and cancellation
    pub fn pull_model<F>(
        client: OllamaClient,
        toast_overlay: ToastOverlay,
        hub_view: Rc<HubView>,
        header: Rc<Header>,
        active_cancellers: Rc<RefCell<HashMap<String, Arc<AtomicBool>>>>,
        tag: &str,
        on_finished: F,
    ) where
        F: Fn(bool) + 'static,
    {
        let t = toast_overlay.clone();
        let h = hub_view.clone();
        let tag_str = tag.to_string();

        t.add_toast(create_copyable_toast(&t!("toasts.download_starting", name = tag)));
        h.set_pull_progress(&tag_str, true, &t!("toasts.connecting_checking"), None);

        let cancelled = Arc::new(AtomicBool::new(false));
        active_cancellers.borrow_mut().insert(tag_str.clone(), cancelled.clone());

        let cancel_for_header = cancelled.clone();
        header.update_download_progress(&tag_str, &t!("toasts.connecting"), None, None, move |_t| {
            cancel_for_header.store(true, Ordering::SeqCst);
        });

        let (tx, rx) = async_channel::unbounded::<PullProgress>();

        let is_finished = std::rc::Rc::new(std::cell::Cell::new(false));
        let is_finished_timer = is_finished.clone();

        // GTK timer to consume download progress
        let h_progress = h.clone();
        let hdr_progress = header.clone();
        let tag_progress = tag_str.clone();
        let cancel_for_timer = cancelled.clone();
        glib::timeout_add_local(Duration::from_millis(150), move || {
            if is_finished_timer.get() {
                return glib::ControlFlow::Break;
            }
            let mut keep_polling = true;
            while let Ok(progress) = rx.try_recv() {
                let status_msg = if let (Some(c), Some(tot)) = (progress.completed, progress.total) {
                    let pct = if tot > 0 { (c as f64 / tot as f64) * 100.0 } else { 0.0 };
                    format!("{} ({:.1}%)", progress.status, pct)
                } else {
                    progress.status.clone()
                };

                let frac = progress.fraction();
                h_progress.set_pull_progress(&tag_progress, true, &status_msg, frac);

                let cancel_cb = cancel_for_timer.clone();
                hdr_progress.update_download_progress(
                    &tag_progress,
                    &progress.status,
                    progress.completed,
                    progress.total,
                    move |_| {
                        cancel_cb.store(true, Ordering::SeqCst);
                    },
                );

                if progress.status == "success" {
                    keep_polling = false;
                    is_finished_timer.set(true);
                }
            }
            if keep_polling && !is_finished_timer.get() {
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Break
            }
        });

        let tag_async = tag_str.clone();
        let orig_tag = tag_str.clone();
        let cancel_for_stream = cancelled.clone();
        let is_finished_spawn = is_finished.clone();
        let active_cancellers_cb = active_cancellers.clone();
        let hdr_finish = header.clone();

        spawn_async(
            async move {
                let res = client.pull_model_stream(&tag_async, move |p| {
                    if cancel_for_stream.load(Ordering::SeqCst) {
                        return false;
                    }
                    let _ = tx.send_blocking(p);
                    true
                }).await;
                (tag_async, res)
            },
            move |(model_name, res)| {
                is_finished_spawn.set(true);
                active_cancellers_cb.borrow_mut().remove(&orig_tag);
                active_cancellers_cb.borrow_mut().remove(&model_name);

                match res {
                    Ok(_) => {
                        h.set_pull_progress(&orig_tag, false, &t!("toasts.download_success", name = orig_tag.clone()), Some(1.0));
                        h.set_pull_progress(&model_name, false, &t!("toasts.download_success", name = model_name.clone()), Some(1.0));
                        hdr_finish.mark_download_finished(&orig_tag, &t!("toasts.download_success", name = orig_tag.clone()), true);
                        hdr_finish.mark_download_finished(&model_name, &t!("toasts.download_success", name = model_name.clone()), true);
                        t.add_toast(create_copyable_toast(&t!("toasts.download_success", name = model_name)));
                        on_finished(true);
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        let is_cancelled = err_msg.contains("cancelled") || err_msg.to_lowercase().contains("cancel");
                        let label = if is_cancelled {
                            t!("toasts.download_cancelled")
                        } else {
                            format!("❌ {}", e)
                        };
                        h.set_pull_progress(&orig_tag, false, &label, None);
                        h.set_pull_progress(&model_name, false, &label, None);
                        hdr_finish.mark_download_finished(&orig_tag, &label, false);
                        hdr_finish.mark_download_finished(&model_name, &label, false);
                        t.add_toast(create_copyable_toast(&format!("❌ {}", e)));
                        on_finished(false);
                    }
                }
            },
        );
    }

    /// Compiles and creates a new model / variant from a Modelfile
    pub fn create_custom_variant<F>(
        client: OllamaClient,
        toast_overlay: ToastOverlay,
        name: &str,
        modelfile: &str,
        on_finished: F,
    ) where
        F: Fn(bool) + 'static,
    {
        let t = toast_overlay.clone();
        let name_str = name.to_string();
        let modelfile_str = modelfile.to_string();

        t.add_toast(create_copyable_toast(&t!("toasts.compiling_variant", name = name)));

        let name_async = name_str.clone();
        spawn_async(
            async move {
                let res = client.create_model_stream(&name_async, &modelfile_str, |_| true).await;
                (name_async, res)
            },
            move |(model_name, res)| {
                match res {
                    Ok(_) => {
                        t.add_toast(create_copyable_toast(&t!("toasts.variant_created", name = model_name)));
                        on_finished(true);
                    }
                    Err(e) => {
                        t.add_toast(create_copyable_toast(&t!("toasts.compile_failed", err = e)));
                        on_finished(false);
                    }
                }
            },
        );
    }
}
