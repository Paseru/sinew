use crate::*;

use base64::engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD};
use futures_util::{SinkExt, StreamExt};
use ring::{
    aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM},
    agreement, digest,
    rand::{SecureRandom, SystemRandom},
};
use serde::de::DeserializeOwned;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const REMOTE_SETTINGS_KEY: &str = "remote_settings_v1";
pub(super) const REMOTE_STATUS_EVENT_NAME: &str = "remote-status-changed";
pub(super) const CONVERSATIONS_CHANGED_EVENT_NAME: &str = "conversations-changed";
const DEFAULT_RELAY_WS_URL: &str = "wss://remote.sinew-ide.com/ws";
const PAIRING_CODE_TTL_MS: i64 = 5 * 60 * 1000;
const PAIRING_MAX_ATTEMPTS: u32 = 5;
const PAIRING_LOCK_MS: i64 = 60 * 1000;
const REMOTE_PROTOCOL_VERSION: u32 = 1;
const REMOTE_AEAD_AAD: &[u8] = b"sinew-remote-v1";
const REMOTE_ATTACHMENT_MAX_BYTES: usize = 15 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct RemoteRuntime {
    inner: Arc<Mutex<RemoteRuntimeInner>>,
    relay_tx: Arc<Mutex<Option<mpsc::UnboundedSender<RelayClientFrame>>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteSettings {
    enabled: bool,
    pc_id: String,
    relay_url: String,
    devices: Vec<RemoteDeviceRecord>,
}

impl Default for RemoteSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            pc_id: random_id("pc", 18),
            relay_url: DEFAULT_RELAY_WS_URL.to_string(),
            devices: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteDeviceRecord {
    id: String,
    name: String,
    token_hash: String,
    secret_b64: String,
    paired_at_ms: i64,
    last_seen_at_ms: Option<i64>,
    revoked_at_ms: Option<i64>,
    #[serde(default)]
    push_subscriptions: Vec<RemotePushSubscription>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemotePushSubscription {
    endpoint: String,
    keys: RemotePushKeys,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemotePushKeys {
    p256dh: String,
    auth: String,
}

#[derive(Debug, Clone)]
struct PairingWindow {
    code: String,
    expires_at_ms: i64,
    attempts: u32,
    locked_until_ms: Option<i64>,
}

#[derive(Debug, Clone)]
struct RemoteRuntimeInner {
    settings: RemoteSettings,
    relay_connected: bool,
    connected_devices: HashSet<String>,
    seen_request_ids: HashSet<String>,
    pairing: Option<PairingWindow>,
    current_workspace: Option<String>,
    open_workspaces: HashMap<String, String>,
    connect_generation: u64,
}

impl RemoteRuntime {
    pub(super) fn from_store(store: &AppStore) -> Self {
        let settings = store
            .load_json_setting::<RemoteSettings>(REMOTE_SETTINGS_KEY)
            .ok()
            .flatten()
            .unwrap_or_default()
            .normalized();
        Self {
            inner: Arc::new(Mutex::new(RemoteRuntimeInner {
                settings,
                relay_connected: false,
                connected_devices: HashSet::new(),
                seen_request_ids: HashSet::new(),
                pairing: None,
                current_workspace: None,
                open_workspaces: HashMap::new(),
                connect_generation: 0,
            })),
            relay_tx: Arc::new(Mutex::new(None)),
        }
    }

    async fn status(&self) -> RemoteStatus {
        let inner = self.inner.lock().await;
        status_from_inner(&inner)
    }

    async fn set_enabled(
        &self,
        app: &AppHandle,
        store: &AppStore,
        enabled: bool,
        relay_url: Option<String>,
    ) -> Result<RemoteStatus> {
        let should_start = {
            let mut inner = self.inner.lock().await;
            inner.settings.enabled = enabled;
            if let Some(relay_url) = relay_url
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
            {
                inner.settings.relay_url = normalize_relay_url(&relay_url);
            }
            if !enabled {
                inner.relay_connected = false;
                inner.connected_devices.clear();
                inner.seen_request_ids.clear();
                inner.pairing = None;
                inner.connect_generation = inner.connect_generation.saturating_add(1);
            }
            store.save_json_setting(REMOTE_SETTINGS_KEY, &inner.settings)?;
            enabled
        };
        if !should_start {
            self.relay_tx.lock().await.take();
        } else {
            self.ensure_connection(app.clone());
        }
        self.emit_status(app).await;
        Ok(self.status().await)
    }

    async fn start_pairing(&self, app: &AppHandle, store: &AppStore) -> Result<RemoteStatus> {
        let (pc_id, code, expires_at_ms) = {
            let mut inner = self.inner.lock().await;
            inner.settings.enabled = true;
            let code = generate_pairing_code();
            let expires_at_ms = now_ms().saturating_add(PAIRING_CODE_TTL_MS);
            inner.pairing = Some(PairingWindow {
                code: code.clone(),
                expires_at_ms,
                attempts: 0,
                locked_until_ms: None,
            });
            store.save_json_setting(REMOTE_SETTINGS_KEY, &inner.settings)?;
            (inner.settings.pc_id.clone(), code, expires_at_ms)
        };
        self.ensure_connection(app.clone());
        let _ = self
            .send_relay(RelayClientFrame::PcPairingCode {
                pc_id,
                code,
                expires_at_ms,
            })
            .await;
        self.emit_status(app).await;
        Ok(self.status().await)
    }

    async fn stop_pairing(&self, app: &AppHandle) -> RemoteStatus {
        {
            let mut inner = self.inner.lock().await;
            inner.pairing = None;
        }
        self.emit_status(app).await;
        self.status().await
    }

    async fn revoke_device(
        &self,
        app: &AppHandle,
        store: &AppStore,
        device_id: &str,
    ) -> Result<RemoteStatus> {
        let secret = {
            let mut inner = self.inner.lock().await;
            let Some(device) = inner
                .settings
                .devices
                .iter_mut()
                .find(|device| device.id == device_id)
            else {
                return Err(anyhow::anyhow!("remote device not found"));
            };
            device.revoked_at_ms = Some(now_ms());
            let secret = device.secret_b64.clone();
            inner.connected_devices.remove(device_id);
            store.save_json_setting(REMOTE_SETTINGS_KEY, &inner.settings)?;
            secret
        };

        if let Ok(key) = B64.decode(secret) {
            let _ = self
                .send_encrypted_to_device_with_key(device_id, &key, &RemotePcPayload::DeviceRevoked)
                .await;
        }
        let _ = self
            .send_relay(RelayClientFrame::PcRevokeDevice {
                device_id: device_id.to_string(),
            })
            .await;
        self.emit_status(app).await;
        Ok(self.status().await)
    }

    fn ensure_connection(&self, app: AppHandle) {
        let runtime = self.clone();
        tauri::async_runtime::spawn(async move {
            runtime.connection_loop(app).await;
        });
    }

    async fn connection_loop(self, app: AppHandle) {
        let generation = {
            let mut inner = self.inner.lock().await;
            if !inner.settings.enabled {
                return;
            }
            inner.connect_generation = inner.connect_generation.saturating_add(1);
            inner.connect_generation
        };

        loop {
            let (enabled, relay_url, pc_id, current_generation) = {
                let inner = self.inner.lock().await;
                (
                    inner.settings.enabled,
                    inner.settings.relay_url.clone(),
                    inner.settings.pc_id.clone(),
                    inner.connect_generation,
                )
            };
            if !enabled || current_generation != generation {
                break;
            }

            match connect_async(&relay_url).await {
                Ok((ws, _)) => {
                    let (mut write, mut read) = ws.split();
                    let (tx, mut rx) = mpsc::unbounded_channel::<RelayClientFrame>();
                    {
                        let mut relay_tx = self.relay_tx.lock().await;
                        *relay_tx = Some(tx.clone());
                    }
                    {
                        let mut inner = self.inner.lock().await;
                        inner.relay_connected = true;
                        inner.connected_devices.clear();
                    }
                    self.emit_status(&app).await;

                    let _ = tx.send(RelayClientFrame::PcHello {
                        pc_id: pc_id.clone(),
                        protocol_version: REMOTE_PROTOCOL_VERSION,
                    });
                    if let Some((code, expires_at_ms)) = self.current_pairing_code().await {
                        let _ = tx.send(RelayClientFrame::PcPairingCode {
                            pc_id: pc_id.clone(),
                            code,
                            expires_at_ms,
                        });
                    }

                    let writer = tauri::async_runtime::spawn(async move {
                        while let Some(frame) = rx.recv().await {
                            let Ok(text) = serde_json::to_string(&frame) else {
                                continue;
                            };
                            if write.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                    });

                    loop {
                        let still_current = self.connection_is_current(generation).await;
                        if !still_current {
                            break;
                        }
                        let next_message = tokio::select! {
                            message = read.next() => message,
                            _ = tokio::time::sleep(Duration::from_millis(250)) => continue,
                        };
                        if !self.connection_is_current(generation).await {
                            break;
                        }
                        match next_message {
                            Some(Ok(Message::Text(text))) => {
                                match serde_json::from_str::<RelayServerFrame>(&text) {
                                    Ok(frame) => self.handle_relay_frame(&app, frame).await,
                                    Err(err) => {
                                        tracing::warn!(error = %err, "bad remote relay frame")
                                    }
                                }
                            }
                            Some(Ok(Message::Binary(bytes))) => {
                                match serde_json::from_slice::<RelayServerFrame>(&bytes) {
                                    Ok(frame) => self.handle_relay_frame(&app, frame).await,
                                    Err(err) => {
                                        tracing::warn!(error = %err, "bad binary remote relay frame")
                                    }
                                }
                            }
                            Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {}
                            Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                            _ => {}
                        }
                    }
                    writer.abort();
                }
                Err(err) => {
                    tracing::warn!(error = %err, relay_url = %relay_url, "remote relay connect failed");
                }
            }

            let (still_current, should_reconnect) = {
                let mut inner = self.inner.lock().await;
                if inner.connect_generation != generation {
                    (false, false)
                } else {
                    inner.relay_connected = false;
                    inner.connected_devices.clear();
                    (true, inner.settings.enabled)
                }
            };
            if !still_current {
                break;
            }
            {
                let mut relay_tx = self.relay_tx.lock().await;
                relay_tx.take();
            }
            self.emit_status(&app).await;
            if !should_reconnect {
                break;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    async fn connection_is_current(&self, generation: u64) -> bool {
        let inner = self.inner.lock().await;
        inner.settings.enabled && inner.connect_generation == generation
    }

    async fn handle_relay_frame(&self, app: &AppHandle, frame: RelayServerFrame) {
        match frame {
            RelayServerFrame::PcRegistered { pc_id } => {
                tracing::debug!(pc_id = %pc_id, "remote PC registered with relay");
                let mut inner = self.inner.lock().await;
                inner.relay_connected = true;
                drop(inner);
                self.emit_status(app).await;
            }
            RelayServerFrame::PairingRequest {
                phone_conn_id,
                code,
                device_name,
                phone_public_key,
            } => {
                if let Err(err) = self
                    .handle_pairing_request(app, phone_conn_id, code, device_name, phone_public_key)
                    .await
                {
                    tracing::warn!(error = %err, "remote pairing request failed");
                }
            }
            RelayServerFrame::PhoneCipher {
                device_id,
                envelope,
            } => {
                if let Err(err) = self.handle_phone_cipher(app, &device_id, envelope).await {
                    tracing::warn!(error = %err, device_id = %device_id, "remote command failed");
                    let _ = self
                        .send_error_to_device(&device_id, None, err.to_string())
                        .await;
                }
            }
            RelayServerFrame::PhoneConnected { device_id } => {
                let mut inner = self.inner.lock().await;
                inner.connected_devices.insert(device_id);
                drop(inner);
                self.emit_status(app).await;
            }
            RelayServerFrame::PhoneDisconnected { device_id } => {
                let mut inner = self.inner.lock().await;
                inner.connected_devices.remove(&device_id);
                drop(inner);
                self.emit_status(app).await;
            }
            RelayServerFrame::Error { message } => {
                tracing::warn!(message = %message, "remote relay error");
            }
        }
    }

    async fn current_pairing_code(&self) -> Option<(String, i64)> {
        let mut inner = self.inner.lock().await;
        expire_pairing_if_needed(&mut inner);
        inner
            .pairing
            .as_ref()
            .map(|pairing| (pairing.code.clone(), pairing.expires_at_ms))
    }

    async fn handle_pairing_request(
        &self,
        app: &AppHandle,
        phone_conn_id: String,
        code: String,
        device_name: String,
        phone_public_key_b64: String,
    ) -> Result<()> {
        let pc_id = {
            let mut inner = self.inner.lock().await;
            expire_pairing_if_needed(&mut inner);
            let now = now_ms();
            let Some(pairing) = inner.pairing.as_mut() else {
                return self
                    .reject_pairing(phone_conn_id, "pairing is not open")
                    .await;
            };
            if pairing.locked_until_ms.is_some_and(|until| until > now) {
                return self
                    .reject_pairing(phone_conn_id, "pairing is temporarily locked")
                    .await;
            }
            if pairing.code != code || pairing.expires_at_ms <= now {
                pairing.attempts = pairing.attempts.saturating_add(1);
                if pairing.attempts >= PAIRING_MAX_ATTEMPTS {
                    pairing.locked_until_ms = Some(now.saturating_add(PAIRING_LOCK_MS));
                }
                drop(inner);
                self.emit_status(app).await;
                return self
                    .reject_pairing(phone_conn_id, "invalid pairing code")
                    .await;
            }
            inner.settings.pc_id.clone()
        };

        let phone_public_key = B64
            .decode(phone_public_key_b64.as_bytes())
            .context("invalid phone public key")?;
        let rng = SystemRandom::new();
        let pc_private = agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, &rng)
            .map_err(|_| anyhow::anyhow!("unable to generate pairing key"))?;
        let pc_public_key = pc_private
            .compute_public_key()
            .map_err(|_| anyhow::anyhow!("unable to compute pairing public key"))?;
        let pc_public_key_bytes = pc_public_key.as_ref().to_vec();
        let peer =
            agreement::UnparsedPublicKey::new(&agreement::ECDH_P256, phone_public_key.as_slice());
        let pairing_key = agreement::agree_ephemeral(pc_private, &peer, |shared| {
            derive_pairing_key(shared, &pc_public_key_bytes, &phone_public_key, &code)
        })
        .map_err(|_| anyhow::anyhow!("unable to establish pairing key"))?;

        let mut secret = [0u8; 32];
        rng.fill(&mut secret)
            .map_err(|_| anyhow::anyhow!("unable to generate device secret"))?;
        let device_id = random_id("dev", 18);
        let device_token = random_id("tok", 32);
        let paired_at_ms = now_ms();
        let name = sanitize_device_name(&device_name);
        let grant = RemotePairingGrant {
            pc_id: pc_id.clone(),
            relay_url: public_relay_url_for_ws(&self.relay_url().await),
            device_id: device_id.clone(),
            device_name: name.clone(),
            device_token: device_token.clone(),
            device_secret: B64.encode(secret),
        };
        let encrypted = encrypt_json(&pairing_key, &grant)?;

        {
            let app_state = app.state::<DesktopState>();
            let mut inner = self.inner.lock().await;
            inner
                .settings
                .devices
                .retain(|device| device.id != device_id);
            inner.settings.devices.push(RemoteDeviceRecord {
                id: device_id,
                name,
                token_hash: token_hash(&device_token),
                secret_b64: grant.device_secret.clone(),
                paired_at_ms,
                last_seen_at_ms: None,
                revoked_at_ms: None,
                push_subscriptions: Vec::new(),
            });
            app_state
                .store
                .save_json_setting(REMOTE_SETTINGS_KEY, &inner.settings)?;
        }

        self.send_relay(RelayClientFrame::PcPairResponse {
            phone_conn_id,
            accepted: true,
            pc_public_key: Some(B64.encode(pc_public_key_bytes)),
            encrypted: Some(encrypted),
            error: None,
        })
        .await?;
        self.emit_status(app).await;
        Ok(())
    }

    async fn reject_pairing(&self, phone_conn_id: String, error: &str) -> Result<()> {
        self.send_relay(RelayClientFrame::PcPairResponse {
            phone_conn_id,
            accepted: false,
            pc_public_key: None,
            encrypted: None,
            error: Some(error.to_string()),
        })
        .await
    }

    async fn relay_url(&self) -> String {
        let inner = self.inner.lock().await;
        inner.settings.relay_url.clone()
    }

    async fn handle_phone_cipher(
        &self,
        app: &AppHandle,
        device_id: &str,
        envelope: CipherEnvelope,
    ) -> Result<()> {
        let (device, key) = self.device_for_command(device_id).await?;
        let command_envelope: RemotePhoneEnvelope = decrypt_json(&key, envelope)?;
        if token_hash(&command_envelope.token) != device.token_hash {
            return Err(anyhow::anyhow!("remote device token rejected"));
        }
        self.touch_device_seen(app, device_id).await?;
        let request_id = command_envelope.request_id.clone();
        self.remember_request_id(device_id, &request_id).await?;
        let result = self
            .execute_phone_command(app, device_id, command_envelope)
            .await;
        match result {
            Ok(data) => {
                self.send_response_to_device(device_id, request_id, true, Some(data), None)
                    .await?;
            }
            Err(err) => {
                self.send_response_to_device(
                    device_id,
                    request_id,
                    false,
                    None,
                    Some(err.to_string()),
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn remember_request_id(&self, device_id: &str, request_id: &str) -> Result<()> {
        let key = format!("{device_id}:{request_id}");
        let mut inner = self.inner.lock().await;
        if inner.seen_request_ids.contains(&key) {
            return Err(anyhow::anyhow!("duplicate remote request rejected"));
        }
        if inner.seen_request_ids.len() > 4_000 {
            inner.seen_request_ids.clear();
        }
        inner.seen_request_ids.insert(key);
        Ok(())
    }

    async fn device_for_command(&self, device_id: &str) -> Result<(RemoteDeviceRecord, Vec<u8>)> {
        let inner = self.inner.lock().await;
        if !inner.settings.enabled {
            return Err(anyhow::anyhow!("remote access is disabled"));
        }
        let Some(device) = inner
            .settings
            .devices
            .iter()
            .find(|device| device.id == device_id)
            .cloned()
        else {
            return Err(anyhow::anyhow!("remote device is not paired"));
        };
        if device.revoked_at_ms.is_some() {
            return Err(anyhow::anyhow!("remote device was revoked"));
        }
        let key = B64
            .decode(device.secret_b64.as_bytes())
            .context("stored remote device key is invalid")?;
        Ok((device, key))
    }

    async fn touch_device_seen(&self, app: &AppHandle, device_id: &str) -> Result<()> {
        let app_state = app.state::<DesktopState>();
        let mut inner = self.inner.lock().await;
        if let Some(device) = inner
            .settings
            .devices
            .iter_mut()
            .find(|device| device.id == device_id)
        {
            device.last_seen_at_ms = Some(now_ms());
            app_state
                .store
                .save_json_setting(REMOTE_SETTINGS_KEY, &inner.settings)?;
        }
        Ok(())
    }

    async fn execute_phone_command(
        &self,
        app: &AppHandle,
        device_id: &str,
        envelope: RemotePhoneEnvelope,
    ) -> Result<Value> {
        let state = app.state::<DesktopState>();
        let (current_workspace, mut open_workspaces) = self.workspace_view().await;
        let workspace_path = match envelope.workspace.as_deref() {
            Some(requested) if !requested.is_empty() => {
                if open_workspaces.iter().any(|id| id == requested)
                    || current_workspace.as_deref() == Some(requested)
                {
                    requested.to_string()
                } else {
                    return Err(anyhow::anyhow!(
                        "this workspace is no longer open on the PC"
                    ));
                }
            }
            _ => current_workspace
                .clone()
                .or_else(|| open_workspaces.first().cloned())
                .ok_or_else(|| anyhow::anyhow!("no workspace is open on this PC"))?,
        };
        if open_workspaces.is_empty() {
            open_workspaces.push(workspace_path.clone());
        }

        match envelope.command {
            RemotePhoneCommand::Bootstrap => {
                let bootstrap = remote_bootstrap(&state, &workspace_path)?;
                let active_turns = turns::list_active_turns(state)
                    .await
                    .map_err(|err| anyhow::anyhow!(err))?;
                let workspaces: Vec<Value> = open_workspaces
                    .iter()
                    .map(|id| {
                        json!({
                            "path": id,
                            "name": workspace_display_name(id),
                        })
                    })
                    .collect();
                Ok(json!({
                    "workspacePath": workspace_path,
                    "workspaces": workspaces,
                    "bootstrap": bootstrap,
                    "activeTurns": active_turns,
                }))
            }
            RemotePhoneCommand::ListConversations => {
                let conversations = conversations::list_conversations(
                    state,
                    WorkspaceInput {
                        workspace_path: workspace_path.clone(),
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err))?;
                Ok(serde_json::to_value(conversations)?)
            }
            RemotePhoneCommand::CreateConversation => {
                let bootstrap = conversations::create_conversation(
                    state,
                    WorkspaceInput {
                        workspace_path: workspace_path.clone(),
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err))?;
                emit_conversations_changed(app, &workspace_path);
                Ok(serde_json::to_value(bootstrap)?)
            }
            RemotePhoneCommand::LoadConversation { conversation_id } => {
                let conversation = conversations::load_conversation(
                    state,
                    ConversationInput {
                        workspace_path: workspace_path.clone(),
                        conversation_id,
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err))?;
                Ok(serde_json::to_value(conversation)?)
            }
            RemotePhoneCommand::DeleteConversation { conversation_id } => {
                let bootstrap = conversations::delete_conversation(
                    state,
                    ConversationInput {
                        workspace_path: workspace_path.clone(),
                        conversation_id,
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err))?;
                emit_conversations_changed(app, &workspace_path);
                Ok(serde_json::to_value(bootstrap)?)
            }
            RemotePhoneCommand::SetConversationMode {
                conversation_id,
                mode,
            } => {
                let conversation = conversations::set_conversation_mode(
                    state,
                    ConversationModeInput {
                        workspace_path: workspace_path.clone(),
                        conversation_id,
                        mode,
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err))?;
                Ok(serde_json::to_value(conversation)?)
            }
            RemotePhoneCommand::ListModels => {
                let providers = providers::list_configured_model_providers(state)
                    .map_err(|err| anyhow::anyhow!(err))?;
                let openrouter_models = if providers.iter().any(|p| p == "openrouter") {
                    providers::list_openrouter_models(app.state::<DesktopState>())
                        .map_err(|err| anyhow::anyhow!(err))?
                } else {
                    Vec::new()
                };
                Ok(json!({
                    "providers": providers,
                    "openrouterModels": openrouter_models,
                }))
            }
            RemotePhoneCommand::SetConversationModel {
                conversation_id,
                mode,
                model,
                thinking,
            } => {
                let settings = conversations::set_conversation_model_preference(
                    state,
                    ConversationModelPreferenceInput {
                        workspace_path: workspace_path.clone(),
                        conversation_id,
                        mode,
                        model,
                        thinking,
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err))?;
                Ok(serde_json::to_value(settings)?)
            }
            RemotePhoneCommand::SendMessage {
                conversation_id,
                text,
                attachments,
                model,
                thinking,
                mode,
                service_tier,
                plan_control,
                message_visibility,
            } => {
                let attachments = materialize_remote_attachments(&attachments).await?;
                turns::send_message(
                    app.clone(),
                    state,
                    SendMessageInput {
                        workspace_path: workspace_path.clone(),
                        conversation_id,
                        text,
                        attachments,
                        model,
                        thinking,
                        mode,
                        service_tier,
                        plan_control,
                        message_visibility,
                        rewrite_from_history_index: None,
                        revert_workspace_changes: false,
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err))?;
                Ok(json!({ "accepted": true }))
            }
            RemotePhoneCommand::CompactConversation {
                conversation_id,
                model,
                thinking,
                service_tier,
                instruction,
            } => {
                turns::compact_conversation(
                    app.clone(),
                    state,
                    CompactConversationInput {
                        workspace_path: workspace_path.clone(),
                        conversation_id,
                        model,
                        thinking,
                        service_tier,
                        instruction,
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err))?;
                Ok(json!({ "compacted": true }))
            }
            RemotePhoneCommand::AnswerQuestion {
                conversation_id,
                tool_call_id,
                answers,
                stop_questions,
            } => {
                let ok = turns::answer_question(
                    state,
                    AnswerQuestionInput {
                        workspace_path: workspace_path.clone(),
                        conversation_id,
                        tool_call_id,
                        answers,
                        stop_questions,
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err))?;
                Ok(json!({ "accepted": ok }))
            }
            RemotePhoneCommand::RejectQuestion {
                conversation_id,
                tool_call_id,
            } => {
                let ok = turns::reject_question(
                    state,
                    RejectQuestionInput {
                        workspace_path: workspace_path.clone(),
                        conversation_id,
                        tool_call_id,
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err))?;
                Ok(json!({ "accepted": ok }))
            }
            RemotePhoneCommand::ReplayActiveTurnEvents {
                conversation_id,
                after_sequence,
            } => {
                let replay = turns::replay_active_turn_events(
                    state,
                    ActiveTurnReplayInput {
                        workspace_path: workspace_path.clone(),
                        conversation_id,
                        after_sequence,
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err))?;
                Ok(serde_json::to_value(replay)?)
            }
            RemotePhoneCommand::SubscribePush { subscription } => {
                self.save_push_subscription(app, device_id, subscription)
                    .await?;
                Ok(json!({ "subscribed": true }))
            }
            RemotePhoneCommand::UnsubscribePush { endpoint } => {
                self.remove_push_subscription(app, device_id, &endpoint)
                    .await?;
                Ok(json!({ "subscribed": false }))
            }
            RemotePhoneCommand::Ping => Ok(json!({ "pong": true, "nowMs": now_ms() })),
        }
    }

    async fn save_push_subscription(
        &self,
        app: &AppHandle,
        device_id: &str,
        subscription: RemotePushSubscription,
    ) -> Result<()> {
        let state = app.state::<DesktopState>();
        let mut inner = self.inner.lock().await;
        let Some(device) = inner
            .settings
            .devices
            .iter_mut()
            .find(|device| device.id == device_id)
        else {
            return Err(anyhow::anyhow!("remote device is not paired"));
        };
        device
            .push_subscriptions
            .retain(|existing| existing.endpoint != subscription.endpoint);
        device.push_subscriptions.push(subscription);
        state
            .store
            .save_json_setting(REMOTE_SETTINGS_KEY, &inner.settings)?;
        Ok(())
    }

    async fn remove_push_subscription(
        &self,
        app: &AppHandle,
        device_id: &str,
        endpoint: &str,
    ) -> Result<()> {
        let state = app.state::<DesktopState>();
        let mut inner = self.inner.lock().await;
        if let Some(device) = inner
            .settings
            .devices
            .iter_mut()
            .find(|device| device.id == device_id)
        {
            device
                .push_subscriptions
                .retain(|subscription| subscription.endpoint != endpoint);
            state
                .store
                .save_json_setting(REMOTE_SETTINGS_KEY, &inner.settings)?;
        }
        Ok(())
    }

    async fn send_response_to_device(
        &self,
        device_id: &str,
        request_id: String,
        ok: bool,
        data: Option<Value>,
        error: Option<String>,
    ) -> Result<()> {
        self.send_encrypted_to_device(
            device_id,
            &RemotePcPayload::Response {
                request_id,
                ok,
                data,
                error,
            },
        )
        .await
    }

    async fn send_error_to_device(
        &self,
        device_id: &str,
        request_id: Option<String>,
        error: String,
    ) -> Result<()> {
        self.send_encrypted_to_device(
            device_id,
            &RemotePcPayload::Response {
                request_id: request_id.unwrap_or_else(|| "unknown".to_string()),
                ok: false,
                data: None,
                error: Some(error),
            },
        )
        .await
    }

    async fn send_encrypted_to_device<T: Serialize>(
        &self,
        device_id: &str,
        payload: &T,
    ) -> Result<()> {
        let (_device, key) = self.device_for_command(device_id).await?;
        self.send_encrypted_to_device_with_key(device_id, &key, payload)
            .await
    }

    async fn send_encrypted_to_device_with_key<T: Serialize>(
        &self,
        device_id: &str,
        key: &[u8],
        payload: &T,
    ) -> Result<()> {
        let envelope = encrypt_json(key, payload)?;
        self.send_relay(RelayClientFrame::PcCipher {
            device_id: device_id.to_string(),
            envelope,
        })
        .await
    }

    async fn broadcast_payload<T: Serialize>(&self, payload: &T) -> Result<()> {
        let devices = {
            let inner = self.inner.lock().await;
            inner
                .settings
                .devices
                .iter()
                .filter(|device| device.revoked_at_ms.is_none())
                .map(|device| (device.id.clone(), device.secret_b64.clone()))
                .collect::<Vec<_>>()
        };
        for (device_id, secret) in devices {
            let Ok(key) = B64.decode(secret.as_bytes()) else {
                continue;
            };
            let _ = self
                .send_encrypted_to_device_with_key(&device_id, &key, payload)
                .await;
        }
        Ok(())
    }

    async fn notify_turn_finished(&self, conversation_id: &str) {
        let subscriptions = {
            let inner = self.inner.lock().await;
            inner
                .settings
                .devices
                .iter()
                .filter(|device| device.revoked_at_ms.is_none())
                .flat_map(|device| device.push_subscriptions.iter().cloned())
                .collect::<Vec<_>>()
        };
        for subscription in subscriptions {
            let _ = self
                .send_relay(RelayClientFrame::PcPush {
                    subscription,
                    payload: RemotePushPayload {
                        title: "Sinew".to_string(),
                        body: "Response ready".to_string(),
                        conversation_id: conversation_id.to_string(),
                    },
                })
                .await;
        }
    }

    async fn send_relay(&self, frame: RelayClientFrame) -> Result<()> {
        let tx = self.relay_tx.lock().await.clone();
        let Some(tx) = tx else {
            return Err(anyhow::anyhow!("remote relay is offline"));
        };
        tx.send(frame)
            .map_err(|_| anyhow::anyhow!("remote relay connection is closed"))
    }

    async fn emit_status(&self, app: &AppHandle) {
        let status = self.status().await;
        let _ = app.emit(REMOTE_STATUS_EVENT_NAME, status);
    }

    pub(super) async fn set_window_workspace(&self, window_label: String, workspace_id: String) {
        let mut inner = self.inner.lock().await;
        inner
            .open_workspaces
            .insert(window_label, workspace_id.clone());
        inner.current_workspace = Some(workspace_id);
    }

    pub(super) async fn remove_window_workspace(&self, window_label: &str) {
        let mut inner = self.inner.lock().await;
        inner.open_workspaces.remove(window_label);
        let current_still_open = inner
            .current_workspace
            .as_ref()
            .is_some_and(|current| inner.open_workspaces.values().any(|id| id == current));
        if !current_still_open {
            inner.current_workspace = inner.open_workspaces.values().next().cloned();
        }
    }

    pub(super) async fn focus_window_workspace(&self, window_label: &str) {
        let mut inner = self.inner.lock().await;
        if let Some(workspace) = inner.open_workspaces.get(window_label).cloned() {
            inner.current_workspace = Some(workspace);
        }
    }

    async fn workspace_view(&self) -> (Option<String>, Vec<String>) {
        let inner = self.inner.lock().await;
        let mut list: Vec<String> = inner.open_workspaces.values().cloned().collect();
        list.sort();
        list.dedup();
        (inner.current_workspace.clone(), list)
    }
}

impl RemoteSettings {
    fn normalized(mut self) -> Self {
        if self.pc_id.trim().is_empty() {
            self.pc_id = random_id("pc", 18);
        }
        self.relay_url = normalize_relay_url(&self.relay_url);
        self.devices.retain(|device| {
            !device.id.trim().is_empty()
                && !device.secret_b64.trim().is_empty()
                && !device.token_hash.trim().is_empty()
        });
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemoteStatus {
    enabled: bool,
    relay_url: String,
    pc_id: String,
    relay_connected: bool,
    reachable: bool,
    pairing: Option<RemotePairingStatus>,
    devices: Vec<RemoteDeviceView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemotePairingStatus {
    code: String,
    expires_at_ms: i64,
    qr_url: String,
    attempts_remaining: u32,
    locked_until_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemoteDeviceView {
    id: String,
    name: String,
    paired_at_ms: i64,
    last_seen_at_ms: Option<i64>,
    revoked_at_ms: Option<i64>,
    connected: bool,
    push_enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemoteSetEnabledInput {
    enabled: bool,
    relay_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemoteDeviceInput {
    device_id: String,
}

#[tauri::command]
pub(super) async fn remote_get_status(
    state: State<'_, DesktopState>,
) -> std::result::Result<RemoteStatus, String> {
    Ok(state.remote.status().await)
}

#[tauri::command]
pub(super) async fn remote_set_enabled(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: RemoteSetEnabledInput,
) -> std::result::Result<RemoteStatus, String> {
    state
        .remote
        .set_enabled(&app, &state.store, input.enabled, input.relay_url)
        .await
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn remote_start_pairing(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> std::result::Result<RemoteStatus, String> {
    state
        .remote
        .start_pairing(&app, &state.store)
        .await
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn remote_stop_pairing(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> std::result::Result<RemoteStatus, String> {
    Ok(state.remote.stop_pairing(&app).await)
}

#[tauri::command]
pub(super) async fn remote_revoke_device(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: RemoteDeviceInput,
) -> std::result::Result<RemoteStatus, String> {
    state
        .remote
        .revoke_device(&app, &state.store, &input.device_id)
        .await
        .map_err(error_to_string)
}

pub(super) fn start_remote_if_enabled(app: &AppHandle) {
    let Some(state) = app.try_state::<DesktopState>() else {
        return;
    };
    let runtime = state.remote.clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if runtime.status().await.enabled {
            runtime.ensure_connection(app);
        }
    });
}

pub(super) fn update_current_workspace(
    app: &AppHandle,
    window_label: String,
    workspace_id: String,
) {
    let Some(state) = app.try_state::<DesktopState>() else {
        return;
    };
    let runtime = state.remote.clone();
    tauri::async_runtime::spawn(async move {
        runtime
            .set_window_workspace(window_label, workspace_id)
            .await;
    });
}

pub(super) fn remove_window_workspace(app: &AppHandle, window_label: String) {
    let Some(state) = app.try_state::<DesktopState>() else {
        return;
    };
    let runtime = state.remote.clone();
    tauri::async_runtime::spawn(async move {
        runtime.remove_window_workspace(&window_label).await;
    });
}

pub(super) fn focus_window_workspace(app: &AppHandle, window_label: String) {
    let Some(state) = app.try_state::<DesktopState>() else {
        return;
    };
    let runtime = state.remote.clone();
    tauri::async_runtime::spawn(async move {
        runtime.focus_window_workspace(&window_label).await;
    });
}

fn emit_conversations_changed(app: &AppHandle, workspace_id: &str) {
    let _ = app.emit(
        CONVERSATIONS_CHANGED_EVENT_NAME,
        json!({ "workspaceId": workspace_id }),
    );
}

fn workspace_display_name(workspace_id: &str) -> String {
    Path::new(workspace_id)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| workspace_id.to_string())
}

pub(super) fn forward_agent_event(
    app: &AppHandle,
    workspace_id: &str,
    conversation_id: &str,
    sequence: Option<u64>,
    event: &AgentEvent,
) {
    let Some(state) = app.try_state::<DesktopState>() else {
        return;
    };
    let runtime = state.remote.clone();
    let workspace_id = workspace_id.to_string();
    let conversation_id = conversation_id.to_string();
    let event = event.clone();
    tauri::async_runtime::spawn(async move {
        let payload = RemotePcPayload::AgentEvent {
            workspace_id,
            conversation_id: conversation_id.clone(),
            sequence,
            event: event.clone(),
        };
        let _ = runtime.broadcast_payload(&payload).await;
        if matches!(event, AgentEvent::TurnFinished { .. }) {
            runtime.notify_turn_finished(&conversation_id).await;
        }
    });
}

pub(super) fn forward_active_turns(app: &AppHandle, active_turns: Vec<ActiveTurnSummary>) {
    let Some(state) = app.try_state::<DesktopState>() else {
        return;
    };
    let runtime = state.remote.clone();
    tauri::async_runtime::spawn(async move {
        let _ = runtime
            .broadcast_payload(&RemotePcPayload::ActiveTurnsChanged { active_turns })
            .await;
    });
}

fn status_from_inner(inner: &RemoteRuntimeInner) -> RemoteStatus {
    let now = now_ms();
    let pairing = inner.pairing.as_ref().and_then(|pairing| {
        (pairing.expires_at_ms > now).then(|| RemotePairingStatus {
            code: pairing.code.clone(),
            expires_at_ms: pairing.expires_at_ms,
            qr_url: format!(
                "{}?code={}",
                public_relay_url_for_ws(&inner.settings.relay_url),
                pairing.code
            ),
            attempts_remaining: PAIRING_MAX_ATTEMPTS.saturating_sub(pairing.attempts),
            locked_until_ms: pairing.locked_until_ms.filter(|until| *until > now),
        })
    });
    RemoteStatus {
        enabled: inner.settings.enabled,
        relay_url: inner.settings.relay_url.clone(),
        pc_id: inner.settings.pc_id.clone(),
        relay_connected: inner.relay_connected,
        reachable: inner.settings.enabled && inner.relay_connected,
        pairing,
        devices: inner
            .settings
            .devices
            .iter()
            .map(|device| RemoteDeviceView {
                id: device.id.clone(),
                name: device.name.clone(),
                paired_at_ms: device.paired_at_ms,
                last_seen_at_ms: device.last_seen_at_ms,
                revoked_at_ms: device.revoked_at_ms,
                connected: inner.connected_devices.contains(&device.id),
                push_enabled: !device.push_subscriptions.is_empty(),
            })
            .collect(),
    }
}

fn expire_pairing_if_needed(inner: &mut RemoteRuntimeInner) {
    let now = now_ms();
    if inner
        .pairing
        .as_ref()
        .is_some_and(|pairing| pairing.expires_at_ms <= now)
    {
        inner.pairing = None;
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RelayClientFrame {
    PcHello {
        pc_id: String,
        protocol_version: u32,
    },
    PcPairingCode {
        pc_id: String,
        code: String,
        expires_at_ms: i64,
    },
    PcPairResponse {
        phone_conn_id: String,
        accepted: bool,
        pc_public_key: Option<String>,
        encrypted: Option<CipherEnvelope>,
        error: Option<String>,
    },
    PcCipher {
        device_id: String,
        envelope: CipherEnvelope,
    },
    PcPush {
        subscription: RemotePushSubscription,
        payload: RemotePushPayload,
    },
    PcRevokeDevice {
        device_id: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RelayServerFrame {
    PcRegistered {
        pc_id: String,
    },
    PairingRequest {
        phone_conn_id: String,
        code: String,
        device_name: String,
        phone_public_key: String,
    },
    PhoneCipher {
        device_id: String,
        envelope: CipherEnvelope,
    },
    PhoneConnected {
        device_id: String,
    },
    PhoneDisconnected {
        device_id: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CipherEnvelope {
    nonce: String,
    ciphertext: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemotePairingGrant {
    pc_id: String,
    relay_url: String,
    device_id: String,
    device_name: String,
    device_token: String,
    device_secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemotePushPayload {
    title: String,
    body: String,
    conversation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemotePhoneEnvelope {
    request_id: String,
    token: String,
    #[serde(default)]
    workspace: Option<String>,
    command: RemotePhoneCommand,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RemotePhoneCommand {
    Bootstrap,
    ListConversations,
    CreateConversation,
    LoadConversation {
        conversation_id: String,
    },
    DeleteConversation {
        conversation_id: String,
    },
    SetConversationMode {
        conversation_id: String,
        mode: AgentModeInput,
    },
    ListModels,
    SetConversationModel {
        conversation_id: String,
        mode: AgentModeInput,
        model: Option<ModelInput>,
        thinking: Option<ThinkingLevelInput>,
    },
    SendMessage {
        conversation_id: String,
        text: String,
        #[serde(default)]
        attachments: Vec<RemoteAttachmentInput>,
        model: Option<ModelInput>,
        thinking: Option<ThinkingLevelInput>,
        mode: Option<AgentModeInput>,
        service_tier: Option<ServiceTierInput>,
        plan_control: Option<PlanControlInput>,
        message_visibility: Option<MessageVisibilityInput>,
    },
    CompactConversation {
        conversation_id: String,
        model: Option<ModelInput>,
        thinking: Option<ThinkingLevelInput>,
        service_tier: Option<ServiceTierInput>,
        instruction: Option<String>,
    },
    AnswerQuestion {
        conversation_id: String,
        tool_call_id: String,
        #[serde(default)]
        answers: Vec<Vec<String>>,
        #[serde(default)]
        stop_questions: bool,
    },
    RejectQuestion {
        conversation_id: String,
        tool_call_id: String,
    },
    ReplayActiveTurnEvents {
        conversation_id: String,
        after_sequence: Option<u64>,
    },
    SubscribePush {
        subscription: RemotePushSubscription,
    },
    UnsubscribePush {
        endpoint: String,
    },
    Ping,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteAttachmentInput {
    path: Option<String>,
    name: Option<String>,
    media_type: Option<String>,
    data: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RemotePcPayload {
    Response {
        request_id: String,
        ok: bool,
        data: Option<Value>,
        error: Option<String>,
    },
    AgentEvent {
        workspace_id: String,
        conversation_id: String,
        sequence: Option<u64>,
        event: AgentEvent,
    },
    ActiveTurnsChanged {
        active_turns: Vec<ActiveTurnSummary>,
    },
    DeviceRevoked,
}

fn remote_bootstrap(state: &DesktopState, workspace_path: &str) -> Result<WorkspaceBootstrap> {
    let workspace_root = normalize_workspace_root(workspace_path)?;
    let mut bootstrap = state.store.bootstrap_workspace(
        &workspace_root,
        &state.default_model,
        &state.system_prompt,
    )?;
    let workspace_id = workspace_root.display().to_string();
    let active_conversation_id = state.active_turn_details.lock().ok().and_then(|active| {
        active
            .values()
            .filter(|record| record.workspace_id == workspace_id)
            .max_by_key(|record| record.started_at_ms)
            .map(|record| record.conversation_id.clone())
    });
    if let Some(conversation_id) = active_conversation_id {
        if let Some(active_conversation) = state
            .store
            .load_conversation(&workspace_id, &conversation_id)?
        {
            bootstrap.active_conversation = active_conversation;
        }
    }
    Ok(bootstrap)
}

async fn materialize_remote_attachments(
    attachments: &[RemoteAttachmentInput],
) -> Result<Vec<AttachmentInput>> {
    let mut out = Vec::new();
    for attachment in attachments.iter().take(8) {
        if let Some(path) = attachment
            .path
            .as_ref()
            .filter(|path| !path.trim().is_empty())
        {
            out.push(AttachmentInput {
                path: path.clone(),
                name: attachment.name.clone(),
            });
            continue;
        }
        let Some(data) = attachment.data.as_ref() else {
            continue;
        };
        let raw = data
            .split_once(',')
            .map(|(_, data)| data)
            .unwrap_or(data.as_str())
            .trim();
        let bytes = B64.decode(raw).context("invalid attachment data")?;
        if bytes.is_empty() {
            continue;
        }
        if bytes.len() > REMOTE_ATTACHMENT_MAX_BYTES {
            return Err(anyhow::anyhow!("attachment is too large"));
        }
        let name =
            remote_attachment_name(attachment.name.as_deref(), attachment.media_type.as_deref());
        let path = write_remote_attachment(&name, &bytes).await?;
        out.push(AttachmentInput {
            path: path.display().to_string(),
            name: Some(name),
        });
    }
    Ok(out)
}

async fn write_remote_attachment(name: &str, bytes: &[u8]) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("sinew-remote-attachments");
    tokio::fs::create_dir_all(&dir).await?;
    let stem = Path::new(name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("remote-attachment");
    let ext = Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("bin");
    let file_name = format!(
        "{}-{}-{}.{}",
        safe_temp_file_stem(stem),
        std::process::id(),
        now_ms(),
        ext
    );
    let path = dir.join(file_name);
    tokio::fs::write(&path, bytes).await?;
    Ok(path)
}

fn remote_attachment_name(name: Option<&str>, media_type: Option<&str>) -> String {
    let raw = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("remote-attachment");
    let path = Path::new(raw);
    if path.extension().is_some() {
        return path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("remote-attachment")
            .to_string();
    }
    let extension = media_type
        .and_then(|media_type| clipboard_image_type(media_type, None).map(|(_, ext)| ext))
        .unwrap_or("bin");
    format!("{}.{}", safe_temp_file_stem(raw), extension)
}

fn encrypt_json<T: Serialize>(key_bytes: &[u8], value: &T) -> Result<CipherEnvelope> {
    if key_bytes.len() != 32 {
        return Err(anyhow::anyhow!("remote encryption key has invalid length"));
    }
    let rng = SystemRandom::new();
    let mut nonce = [0u8; 12];
    rng.fill(&mut nonce)
        .map_err(|_| anyhow::anyhow!("unable to generate remote nonce"))?;
    let unbound = UnboundKey::new(&AES_256_GCM, key_bytes)
        .map_err(|_| anyhow::anyhow!("unable to initialize remote encryption key"))?;
    let key = LessSafeKey::new(unbound);
    let mut plaintext = serde_json::to_vec(value)?;
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::from(REMOTE_AEAD_AAD),
        &mut plaintext,
    )
    .map_err(|_| anyhow::anyhow!("unable to encrypt remote payload"))?;
    Ok(CipherEnvelope {
        nonce: B64.encode(nonce),
        ciphertext: B64.encode(plaintext),
    })
}

fn decrypt_json<T: DeserializeOwned>(key_bytes: &[u8], envelope: CipherEnvelope) -> Result<T> {
    if key_bytes.len() != 32 {
        return Err(anyhow::anyhow!("remote decryption key has invalid length"));
    }
    let nonce_bytes = B64.decode(envelope.nonce.as_bytes())?;
    let nonce: [u8; 12] = nonce_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("remote nonce has invalid length"))?;
    let mut ciphertext = B64.decode(envelope.ciphertext.as_bytes())?;
    let unbound = UnboundKey::new(&AES_256_GCM, key_bytes)
        .map_err(|_| anyhow::anyhow!("unable to initialize remote decryption key"))?;
    let key = LessSafeKey::new(unbound);
    let plaintext = key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(REMOTE_AEAD_AAD),
            &mut ciphertext,
        )
        .map_err(|_| anyhow::anyhow!("remote payload authentication failed"))?;
    Ok(serde_json::from_slice(plaintext)?)
}

fn derive_pairing_key(
    shared: &[u8],
    pc_public_key: &[u8],
    phone_public_key: &[u8],
    code: &str,
) -> [u8; 32] {
    let mut ctx = digest::Context::new(&digest::SHA256);
    ctx.update(b"sinew-remote-pairing-v1");
    ctx.update(shared);
    ctx.update(pc_public_key);
    ctx.update(phone_public_key);
    ctx.update(code.as_bytes());
    let digest = ctx.finish();
    let mut key = [0u8; 32];
    key.copy_from_slice(digest.as_ref());
    key
}

fn token_hash(token: &str) -> String {
    B64.encode(digest::digest(&digest::SHA256, token.as_bytes()).as_ref())
}

fn random_id(prefix: &str, byte_len: usize) -> String {
    let rng = SystemRandom::new();
    let mut bytes = vec![0u8; byte_len];
    if rng.fill(&mut bytes).is_err() {
        let fallback = now_ms().to_string();
        return format!("{prefix}_{fallback}");
    }
    format!("{prefix}_{}", URL_SAFE_NO_PAD.encode(bytes))
}

fn generate_pairing_code() -> String {
    let rng = SystemRandom::new();
    let mut bytes = [0u8; 4];
    if rng.fill(&mut bytes).is_err() {
        return format!("{:06}", now_ms().rem_euclid(1_000_000));
    }
    let value = u32::from_le_bytes(bytes) % 1_000_000;
    format!("{value:06}")
}

fn sanitize_device_name(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "Phone".to_string()
    } else {
        trimmed.chars().take(80).collect()
    }
}

fn normalize_relay_url(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return DEFAULT_RELAY_WS_URL.to_string();
    }
    if trimmed.starts_with("ws://") || trimmed.starts_with("wss://") {
        return trimmed.to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("https://") {
        return format!("wss://{}/ws", rest.trim_end_matches('/'));
    }
    if let Some(rest) = trimmed.strip_prefix("http://") {
        return format!("ws://{}/ws", rest.trim_end_matches('/'));
    }
    format!("wss://{}/ws", trimmed.trim_end_matches('/'))
}

fn public_relay_url_for_ws(ws_url: &str) -> String {
    let mut url = ws_url.to_string();
    if let Some(rest) = url.strip_prefix("wss://") {
        url = format!("https://{rest}");
    } else if let Some(rest) = url.strip_prefix("ws://") {
        url = format!("http://{rest}");
    }
    if let Some(stripped) = url.strip_suffix("/ws") {
        stripped.to_string()
    } else {
        url.trim_end_matches('/').to_string()
    }
}
