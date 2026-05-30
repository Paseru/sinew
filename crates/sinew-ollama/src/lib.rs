mod auth;
mod client;
mod model_info;
mod stream;
mod wire;

pub use auth::{
    default_base_url, delete_default_auth, load_default_auth_status, load_default_base_url,
    save_default_base_url, touch_default_auth_validation, OllamaAuthStatus,
};
pub use client::{
    fetch_model_catalog, validate_endpoint, OllamaCatalogModel, OllamaConfig, OllamaProvider,
};
pub use model_info::{capabilities_from_catalog_model, capabilities_from_parts, PROVIDER_ID};
