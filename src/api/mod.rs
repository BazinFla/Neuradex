pub mod client;
pub mod error;
pub mod github;
pub mod hf_downloader;
pub mod hub_remote;
pub mod types;

pub use client::OllamaClient;
pub use error::ApiError;
pub use github::{GitHubClient, UpdateCheckResult};
pub use hub_remote::OllamaWebClient;

