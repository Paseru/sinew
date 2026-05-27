mod auth;
mod client;
mod connect;
mod identity;
mod model_info;
mod usage;

pub use auth::api::{
    delete_default_api_auth, load_default_api_auth_status, save_default_api_key, ApiCredential,
    CursorApiAuthStatus,
};
pub use auth::composer::{
    delete_composer_auth, ensure_fresh_composer_token, load_composer_auth_status,
    load_composer_session, sync_composer_auth_from_ide, ComposerSession, CursorComposerAuthStatus,
};
pub use auth::oauth::{create_login_challenge, wait_for_oauth_login, CursorLoginChallenge};
pub use client::{CursorConfig, CursorProvider};
pub use identity::CursorIdeIdentity;
pub use model_info::{capabilities, API_PROVIDER_ID, MODEL_COMPOSER_25, MODEL_COMPOSER_25_FAST, PROVIDER_ID};
pub use usage::{fetch_usage, CursorUsageInfo};
