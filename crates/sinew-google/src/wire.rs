use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeAssistGenerateRequest {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub request: VertexGenerateContentRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_type: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VertexGenerateContentRequest {
    pub contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<Content>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Content {
    pub role: String,
    pub parts: Vec<Part>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Part {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thought: Option<bool>,
        #[serde(rename = "thoughtSignature")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thought_signature: Option<String>,
    },
    InlineData {
        #[serde(rename = "inlineData")]
        inline_data: InlineData,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: FunctionCall,
        #[serde(rename = "thoughtSignature")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thought_signature: Option<String>,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: FunctionResponse,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InlineData {
    pub mime_type: String,
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionCall {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub args: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionResponse {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub response: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub function_declarations: Vec<FunctionDeclaration>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct CountTokensRequest {
    pub request: VertexCountTokensRequest,
}

#[derive(Debug, Serialize)]
pub struct VertexCountTokensRequest {
    pub model: String,
    pub contents: Vec<Content>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountTokensResponse {
    #[serde(default)]
    pub total_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeAssistResponse {
    #[serde(default)]
    pub response: Option<GenerateContentResponse>,
    #[serde(default)]
    pub trace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentResponse {
    #[serde(default)]
    pub candidates: Vec<Candidate>,
    #[serde(default)]
    pub usage_metadata: Option<UsageMetadata>,
    #[serde(default)]
    pub model_version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    #[serde(default)]
    pub content: Option<ResponseContent>,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResponseContent {
    #[serde(default)]
    pub parts: Vec<ResponsePart>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponsePart {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub thought: Option<Value>,
    #[serde(default)]
    pub thought_signature: Option<String>,
    #[serde(default)]
    pub function_call: Option<ResponseFunctionCall>,
}

#[derive(Debug, Deserialize)]
pub struct ResponseFunctionCall {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub args: Option<Value>,
}

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    #[serde(default)]
    pub prompt_token_count: u32,
    #[serde(default)]
    pub candidates_token_count: u32,
    #[serde(default)]
    pub thoughts_token_count: u32,
    #[serde(default)]
    pub total_token_count: u32,
    #[serde(default)]
    pub cached_content_token_count: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadCodeAssistRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloudaicompanion_project: Option<String>,
    pub metadata: ClientMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<&'static str>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CloudProjectRef {
    Id(String),
    Object(CloudProject),
}

impl CloudProjectRef {
    pub fn into_id(self) -> Option<String> {
        match self {
            Self::Id(id) => Some(id),
            Self::Object(project) => project.id,
        }
        .filter(|id| !id.trim().is_empty())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientMetadata {
    pub ide_type: &'static str,
    pub platform: &'static str,
    pub plugin_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duet_project: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadCodeAssistResponse {
    #[serde(default)]
    pub current_tier: Option<GeminiUserTier>,
    #[serde(default)]
    pub allowed_tiers: Vec<GeminiUserTier>,
    #[serde(default)]
    pub ineligible_tiers: Vec<IneligibleTier>,
    #[serde(default)]
    pub cloudaicompanion_project: Option<CloudProjectRef>,
    #[serde(default)]
    pub paid_tier: Option<GeminiUserTier>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiUserTier {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub is_default: Option<bool>,
    #[serde(default)]
    pub has_onboarded_previously: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IneligibleTier {
    #[serde(default)]
    pub reason_message: Option<String>,
    #[serde(default)]
    pub tier_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardUserRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloudaicompanion_project: Option<String>,
    pub metadata: ClientMetadata,
}

#[derive(Debug, Deserialize)]
pub struct LongRunningOperationResponse {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub response: Option<OnboardUserResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardUserResponse {
    #[serde(default)]
    pub cloudaicompanion_project: Option<CloudProject>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CloudProject {
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ApiErrorEnvelope {
    #[serde(default)]
    pub error: ApiErrorBody,
}

#[derive(Debug, Default, Deserialize)]
pub struct ApiErrorBody {
    #[serde(default)]
    pub code: Option<i64>,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub status: String,
}
