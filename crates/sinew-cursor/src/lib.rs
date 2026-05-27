mod auth;
mod client;
mod connect;
mod conversation;
mod identity;
mod model_info;
mod sanitize;
mod stream_state;
mod tools;
mod usage;
mod workspace;

#[cfg(test)]
mod tests;

pub use auth::composer::{
    delete_composer_auth, ensure_fresh_composer_token, load_composer_auth_status,
    load_composer_session, sync_composer_auth_from_ide, ComposerSession, CursorComposerAuthStatus,
};
pub use auth::oauth::{create_login_challenge, wait_for_oauth_login, CursorLoginChallenge};
pub use client::{CursorConfig, CursorProvider};
pub use identity::CursorIdeIdentity;
pub use model_info::{capabilities, MODEL_COMPOSER_25, MODEL_COMPOSER_25_FAST, PROVIDER_ID};
pub use sanitize::{sanitize_outbound_json, sanitize_outbound_text};
pub use usage::{fetch_usage, CursorUsageInfo};
