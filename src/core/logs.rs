use async_channel::Sender;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub struct LogStreamer;

impl LogStreamer {
    /// Starts streaming Ollama logs via journalctl to the provided channel.
    pub async fn stream_logs(tx: Sender<String>, running: Arc<AtomicBool>) {
        // Command: journalctl -u ollama.service -f -n 150 --no-pager
        let mut child = match Command::new("journalctl")
            .args(["-u", "ollama.service", "-f", "-n", "150", "--no-pager"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(crate::t!("logs.journalctl_err", err = e)).await;
                return;
            }
        };

        if let Some(stdout) = child.stdout.take() {
            let mut reader = BufReader::new(stdout).lines();
            while running.load(Ordering::SeqCst) {
                match tokio::time::timeout(std::time::Duration::from_millis(500), reader.next_line()).await {
                    Ok(Ok(Some(line))) => {
                        if tx.send(line).await.is_err() {
                            break;
                        }
                    }
                    Ok(Ok(None)) => break,
                    Ok(Err(_)) => break,
                    Err(_) => {
                        // Short timeout to check if running is still active
                        continue;
                    }
                }
            }
        }

        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

