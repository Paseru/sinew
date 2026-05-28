mod auth;
mod client;
mod model_info;
mod stream;
mod wire;

pub use auth::{
    delete_default_auth, load_default_api_key, load_default_auth_status, save_default_api_key,
    touch_default_auth_validation, Credential, DeepSeekAuthStatus,
};
pub use client::{DeepSeekConfig, DeepSeekProvider, validate_api_key};
pub use model_info::{
    capabilities, PROVIDER_ID, DEEPSEEK_CHAT_MODEL, DEEPSEEK_REASONER_MODEL,
    DEEPSEEK_V4_FLASH_MODEL, DEEPSEEK_V4_PRO_MODEL,
};
