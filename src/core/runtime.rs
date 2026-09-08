use async_channel::bounded;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("neuradex-tokio-worker")
            .build()
            .expect("Failed to initialize Tokio runtime")
    })
}

pub fn spawn_async<F, T, CB>(future: F, on_ui: CB)
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
    CB: FnOnce(T) + 'static,
{
    let (sender, receiver) = bounded(1);

    glib::spawn_future_local(async move {
        if let Ok(res) = receiver.recv().await {
            on_ui(res);
        }
    });

    let rt = runtime();
    rt.spawn(async move {
        let result = future.await;
        let _ = sender.send(result).await;
    });
}
