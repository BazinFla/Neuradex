pub mod capabilities;
pub mod chat_session;
pub mod config;
pub mod diagnostic;
pub mod hardware;
pub mod hub;
pub mod i18n;
pub mod lifecycle;
pub mod logs;
pub mod runtime;
pub mod secret_store;
pub mod vault;

pub use capabilities::{
    detect_capabilities, detect_for_hub_model, detect_for_model_tag, detect_icons_for_hub_model,
    detect_icons_for_model_tag, format_capability_icons, ModelCapability,
};
pub use chat_session::ChatSession;
pub use config::{ApiProfile, ApiProviderType, AppConfig, CustomModelSettings, DaemonMode};
pub use diagnostic::generate_diagnostic_report;
pub use hub::{HardwareFitness, HubManager, HubModelInfo, HubModelVariant};
pub use i18n::{
    available_languages, detect_system_language, get_effective_language, get_language,
    get_language_def, get_language_flag, get_language_name, set_language, t, t_args,
    LanguageDef, AVAILABLE_LANGUAGES,
};
pub use lifecycle::{DiskSpaceInfo, LifecycleManager, ServiceState};
pub use logs::LogStreamer;
pub use runtime::{runtime, spawn_async};
pub use secret_store::SecretStore;
pub use vault::{ApiVault, SshKeyInfo, ValidationResult};
