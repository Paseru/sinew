use std::{env, time::Duration};

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use kuchikiki::{traits::TendrilSink, NodeData, NodeRef};
use reqwest::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    Url,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use claakecode_core::ToolDescriptor;

use crate::store::WebSearchProvider;
use crate::tool_run::ToolRunResult;

const LINKUP_SEARCH_URL: &str = "https://api.linkup.so/v1/search";
const EXA_MCP_URL: &str = "https://mcp.exa.ai/mcp";
const EXA_DEFAULT_NUM_RESULTS: u8 = 8;
const USER_AGENT: &str = "ClaakeCode/0.1";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);
const WEBSEARCH_RESPONSE_LIMIT: usize = 256 * 1024;
const WEBFETCH_RESPONSE_LIMIT: usize = 512 * 1024;
const TOOL_OUTPUT_LIMIT: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct WebSearchTool {
    http: reqwest::Client,
    provider: WebSearchProvider,
    api_key: Option<String>,
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self::with_api_key(None)
    }

    pub fn with_api_key(api_key: Option<String>) -> Self {
        Self::with_settings(WebSearchProvider::LinkUp, api_key)
    }

    pub fn with_settings(provider: WebSearchProvider, api_key: Option<String>) -> Self {
        Self {
            http: web_client(),
            provider,
            api_key: api_key
                .map(|key| key.trim().to_string())
                .filter(|key| !key.is_empty()),
        }
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        match self.provider {
            WebSearchProvider::LinkUp => ToolDescriptor {
                name: "WebSearch".into(),
                description: "Use this for web search, documentation check or fresh information."
                    .into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "q": {
                            "type": "string",
                            "description": "The search question or query."
                        },
                        "depth": {
                            "type": "string",
                            "enum": ["standard", "deep"],
                            "description": "Use `standard` for a simple direct answer, `deep` for complex research with multiple sources."
                        }
                    },
                    "required": ["q", "depth"],
                    "additionalProperties": false
                }),
            },
            WebSearchProvider::Classic => ToolDescriptor {
                name: "WebSearch".into(),
                description: "Search the web using Exa AI. Use this for current information, docs, recent data, or information beyond the model knowledge cutoff.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Websearch query. Use the current year when searching for recent information or current events."
                        },
                        "numResults": {
                            "type": "number",
                            "description": "Number of search results to return. Defaults to 8."
                        },
                        "contextMaxCharacters": {
                            "type": "number",
                            "description": "Maximum characters for context optimized for LLMs. Defaults to Exa's standard value."
                        }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }),
            },
        }
    }

    pub async fn run(&self, input: Value) -> ToolRunResult {
        match self.search(input).await {
            Ok(output) => ToolRunResult::ok(output, Vec::new()),
            Err(err) => ToolRunResult::err(err.to_string(), Vec::new()),
        }
    }

    async fn search(&self, input: Value) -> Result<String> {
        match self.provider {
            WebSearchProvider::LinkUp => self.search_linkup(input).await,
            WebSearchProvider::Classic => self.search_classic(input).await,
        }
    }

    async fn search_linkup(&self, input: Value) -> Result<String> {
        let parsed: WebSearchInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid WebSearch input: {err}"))?;
        let q = parsed.q.trim();
        if q.is_empty() {
            bail!("q is required");
        }
        let api_key = load_linkup_api_key(self.api_key.as_deref())?;

        let response = self
            .http
            .post(LINKUP_SEARCH_URL)
            .header(AUTHORIZATION, format!("Bearer {api_key}"))
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json")
            .json(&json!({
                "q": q,
                "depth": parsed.depth.as_str(),
                "outputType": "sourcedAnswer",
                "includeInlineCitations": "true"
            }))
            .send()
            .await
            .context("Linkup request failed")?;

        let status = response.status();
        let (body, truncated) = collect_response_text(response, WEBSEARCH_RESPONSE_LIMIT).await?;
        if !status.is_success() {
            bail!(
                "Linkup request failed ({status}): {}",
                clip_chars(&body, 2_000).0
            );
        }

        let mut output = match serde_json::from_str::<Value>(&body) {
            Ok(value) => format_linkup_response(&value),
            Err(_) => body,
        };

        if truncated {
            output.push_str("\n\n[Response truncated]");
        }

        Ok(clip_with_notice(output, TOOL_OUTPUT_LIMIT))
    }

    async fn search_classic(&self, input: Value) -> Result<String> {
        let parsed: ExaSearchInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid WebSearch input: {err}"))?;
        let query = parsed.query.trim();
        if query.is_empty() {
            bail!("query is required");
        }

        let body = exa_web_search_body(&parsed, query);
        let response = self
            .http
            .post(EXA_MCP_URL)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/event-stream")
            .json(&body)
            .send()
            .await
            .context("Exa WebSearch request failed")?;
        let status = response.status();
        let (body, truncated) = collect_response_text(response, WEBSEARCH_RESPONSE_LIMIT).await?;
        if !status.is_success() {
            bail!(
                "Exa WebSearch request failed ({status}): {}",
                clip_chars(&body, 2_000).0
            );
        }

        let mut output = parse_exa_web_search_response(&body)?;
        if truncated {
            output.push_str("\n\n[Response truncated]");
        }

        Ok(clip_with_notice(output, TOOL_OUTPUT_LIMIT))
    }
}

fn load_linkup_api_key(configured: Option<&str>) -> Result<String> {
    configured
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
        .or_else(|| {
            env::var("LINKUP_API_KEY")
                .ok()
                .map(|key| key.trim().to_string())
        })
        .filter(|key| !key.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "LinkUp API key is missing. Add it in Settings > Tools before using WebSearch."
            )
        })
}

fn exa_web_search_body(input: &ExaSearchInput, query: &str) -> Value {
    let mut arguments = Map::new();
    arguments.insert("query".into(), json!(query));
    arguments.insert("type".into(), json!("deep"));
    arguments.insert(
        "numResults".into(),
        json!(input.num_results.unwrap_or(EXA_DEFAULT_NUM_RESULTS)),
    );
    arguments.insert("livecrawl".into(), json!("preferred"));
    if let Some(context_max_characters) = input.context_max_characters {
        arguments.insert("contextMaxCharacters".into(), json!(context_max_characters));
    }

    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "web_search_exa",
            "arguments": arguments
        }
    })
}

#[derive(Debug, Clone)]
pub struct WebFetchTool {
    http: reqwest::Client,
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self { http: web_client() }
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "WebFetch".into(),
            description: "Fetch a specific URL, usually a source returned by WebSearch, and return readable text for closer inspection.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch."
                    }
                },
                "required": ["url"],
                "additionalProperties": false
            }),
        }
    }

    pub async fn run(&self, input: Value) -> ToolRunResult {
        match self.fetch(input).await {
            Ok(output) => ToolRunResult::ok(output, Vec::new()),
            Err(err) => ToolRunResult::err(err.to_string(), Vec::new()),
        }
    }

    async fn fetch(&self, input: Value) -> Result<String> {
        let parsed: WebFetchInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid WebFetch input: {err}"))?;
        let url = parse_http_url(&parsed.url)?;

        let response = self
            .http
            .get(url)
            .header(
                ACCEPT,
                "text/html, text/plain, application/json, application/xml, text/xml, */*;q=0.8",
            )
            .send()
            .await
            .context("web fetch failed")?;

        let final_url = response.url().to_string();
        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("unknown")
            .to_string();
        let (body, truncated_bytes) =
            collect_response_text(response, WEBFETCH_RESPONSE_LIMIT).await?;

        if !status.is_success() {
            bail!(
                "fetch failed ({status}) for {final_url}: {}",
                clip_chars(&body, 2_000).0
            );
        }

        let is_html = content_type.to_ascii_lowercase().contains("html")
            || body
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("<!doctype html")
            || body.trim_start().to_ascii_lowercase().starts_with("<html");
        let readable = if is_html {
            html_to_markdown(&body, &final_url)
        } else {
            body.trim().to_string()
        };
        let (content, truncated_chars) = clip_chars(&readable, TOOL_OUTPUT_LIMIT);

        let mut output = format!(
            "url: {final_url}\nstatus: {status}\ncontent-type: {content_type}\n\n{content}"
        );
        if truncated_bytes || truncated_chars {
            output.push_str("\n\n[Content truncated]");
        }

        Ok(output)
    }
}

#[derive(Debug, Deserialize)]
struct WebSearchInput {
    #[serde(alias = "query")]
    q: String,
    depth: SearchDepth,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SearchDepth {
    Standard,
    Deep,
}

impl SearchDepth {
    fn as_str(self) -> &'static str {
        match self {
            SearchDepth::Standard => "standard",
            SearchDepth::Deep => "deep",
        }
    }
}

#[derive(Debug, Deserialize)]
struct ExaSearchInput {
    #[serde(alias = "q")]
    query: String,
    #[serde(default, rename = "numResults", alias = "num_results")]
    num_results: Option<u8>,
    #[serde(
        default,
        rename = "contextMaxCharacters",
        alias = "context_max_characters"
    )]
    context_max_characters: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct WebFetchInput {
    url: String,
}

fn web_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn parse_http_url(raw: &str) -> Result<Url> {
    let raw = raw.trim();
    if raw.is_empty() {
        bail!("url is required");
    }
    let url = Url::parse(raw).context("invalid url")?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        _ => bail!("url must start with http:// or https://"),
    }
}

async fn collect_response_text(
    response: reqwest::Response,
    byte_limit: usize,
) -> Result<(String, bool)> {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("unable to read response body")?;
        let remaining = byte_limit.saturating_sub(bytes.len());
        if chunk.len() > remaining {
            bytes.extend_from_slice(&chunk[..remaining]);
            truncated = true;
            break;
        }
        bytes.extend_from_slice(&chunk);
        if bytes.len() >= byte_limit {
            truncated = true;
            break;
        }
    }

    Ok((String::from_utf8_lossy(&bytes).into_owned(), truncated))
}

fn format_linkup_response(value: &Value) -> String {
    let mut output = String::new();
    if let Some(answer) = string_field(value, &["answer", "sourcedAnswer", "output"]) {
        output.push_str(answer.trim());
    } else {
        output.push_str(&serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()));
    }

    if let Some(sources) = array_field(value, &["sources", "results", "documents"]) {
        let rendered = sources
            .iter()
            .take(12)
            .enumerate()
            .filter_map(|(index, source)| render_source(index + 1, source))
            .collect::<Vec<_>>();
        if !rendered.is_empty() {
            output.push_str("\n\nSources:\n");
            output.push_str(&rendered.join("\n"));
        }
    }

    output
}

fn parse_exa_web_search_response(body: &str) -> Result<String> {
    if body
        .lines()
        .any(|line| line.trim_start().starts_with("data:"))
    {
        for event in parse_sse_json_events(body)? {
            if let Some(text) = exa_result_text(&event) {
                return Ok(text);
            }
        }
        bail!("No search results found. Please try a different query.");
    }

    let value: Value = serde_json::from_str(body).context("invalid Exa WebSearch response")?;
    exa_result_text(&value)
        .ok_or_else(|| anyhow::anyhow!("No search results found. Please try a different query."))
}

fn exa_result_text(value: &Value) -> Option<String> {
    value
        .get("result")?
        .get("content")?
        .as_array()?
        .iter()
        .find_map(|item| {
            let kind = item.get("type").and_then(Value::as_str).unwrap_or_default();
            if kind == "text" {
                item.get("text")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .map(str::to_string)
            } else {
                None
            }
        })
}

fn parse_sse_json_events(body: &str) -> Result<Vec<Value>> {
    let mut events = Vec::new();
    let mut current = String::new();

    for line in body.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            flush_sse_json_event(&mut current, &mut events)?;
            continue;
        }
        let Some(data) = line.trim_start().strip_prefix("data:") else {
            continue;
        };
        let data = data.trim_start();
        if data == "[DONE]" {
            flush_sse_json_event(&mut current, &mut events)?;
            continue;
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(data);
    }
    flush_sse_json_event(&mut current, &mut events)?;

    if events.is_empty() {
        bail!("Exa WebSearch returned an empty stream");
    }
    Ok(events)
}

fn flush_sse_json_event(current: &mut String, events: &mut Vec<Value>) -> Result<()> {
    let payload = current.trim();
    if !payload.is_empty() {
        events.push(
            serde_json::from_str(payload)
                .with_context(|| format!("invalid Exa WebSearch stream event: {payload}"))?,
        );
    }
    current.clear();
    Ok(())
}

fn string_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| value.get(*key)?.as_str())
}

fn array_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Vec<Value>> {
    keys.iter().find_map(|key| value.get(*key)?.as_array())
}

fn render_source(index: usize, source: &Value) -> Option<String> {
    let title = string_field(source, &["title", "name", "source"])
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let url = string_field(source, &["url", "link"])
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let snippet = string_field(source, &["snippet", "content", "text"])
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if title.is_none() && url.is_none() && snippet.is_none() {
        return None;
    }

    let mut line = format!("[{index}]");
    if let Some(title) = title {
        line.push(' ');
        line.push_str(title);
    }
    if let Some(url) = url {
        if title.is_some() {
            line.push_str(" - ");
        } else {
            line.push(' ');
        }
        line.push_str(url);
    }
    if let Some(snippet) = snippet {
        let snippet = clip_chars(snippet, 300).0;
        line.push_str("\n    ");
        line.push_str(&snippet);
    }

    Some(line)
}

fn clip_with_notice(input: String, max_chars: usize) -> String {
    let (mut clipped, truncated) = clip_chars(&input, max_chars);
    if truncated {
        clipped.push_str("\n\n[Output truncated]");
    }
    clipped
}

fn clip_chars(input: &str, max_chars: usize) -> (String, bool) {
    if input.chars().count() <= max_chars {
        return (input.to_string(), false);
    }
    (input.chars().take(max_chars).collect(), true)
}

fn html_to_markdown(html: &str, base_url: &str) -> String {
    let document = kuchikiki::parse_html().one(html).document_node;
    let root = content_root(&document);
    let base = Url::parse(base_url).ok();
    let mut output = String::new();

    render_children(&root, &mut output, base.as_ref(), 0);
    normalize_markdown(&output)
}

fn content_root(document: &NodeRef) -> NodeRef {
    for selector in ["article", "main", "body"] {
        if let Ok(node) = document.select_first(selector) {
            return node.as_node().clone();
        }
    }
    document.clone()
}

fn render_children(node: &NodeRef, output: &mut String, base: Option<&Url>, list_depth: usize) {
    for child in node.children() {
        render_node(&child, output, base, list_depth);
    }
}

fn render_node(node: &NodeRef, output: &mut String, base: Option<&Url>, list_depth: usize) {
    match node.data() {
        NodeData::Text(text) => push_text(output, &text.borrow()),
        NodeData::Element(element) => {
            let tag = element.name.local.to_string().to_ascii_lowercase();
            if should_skip_tag(&tag) {
                return;
            }

            match tag.as_str() {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let level = tag[1..].parse::<usize>().unwrap_or(2).clamp(1, 6);
                    ensure_blank_line(output);
                    output.push_str(&"#".repeat(level));
                    output.push(' ');
                    render_children(node, output, base, list_depth);
                    ensure_blank_line(output);
                }
                "p" => {
                    ensure_blank_line(output);
                    render_children(node, output, base, list_depth);
                    ensure_blank_line(output);
                }
                "br" => output.push_str("  \n"),
                "hr" => {
                    ensure_blank_line(output);
                    output.push_str("---");
                    ensure_blank_line(output);
                }
                "strong" | "b" => render_wrapped(node, output, base, list_depth, "**", "**"),
                "em" | "i" => render_wrapped(node, output, base, list_depth, "*", "*"),
                "del" | "s" => render_wrapped(node, output, base, list_depth, "~~", "~~"),
                "code" => render_inline_code(node, output),
                "pre" => render_code_block(node, output),
                "a" => render_link(node, output, base),
                "img" => render_image(node, output, base),
                "ul" => render_list(node, output, base, list_depth, false),
                "ol" => render_list(node, output, base, list_depth, true),
                "blockquote" => render_blockquote(node, output, base, list_depth),
                "table" => {
                    if !render_table(node, output) {
                        render_children(node, output, base, list_depth);
                    }
                }
                "div" | "section" | "article" | "main" | "body" | "header" | "footer" => {
                    render_children(node, output, base, list_depth);
                    ensure_blank_line(output);
                }
                "li" => {
                    ensure_line_start(output);
                    output.push_str("- ");
                    render_children(node, output, base, list_depth);
                    output.push('\n');
                }
                _ => render_children(node, output, base, list_depth),
            }
        }
        _ => {}
    }
}

fn should_skip_tag(tag: &str) -> bool {
    matches!(
        tag,
        "script"
            | "style"
            | "noscript"
            | "svg"
            | "canvas"
            | "iframe"
            | "head"
            | "meta"
            | "link"
            | "title"
            | "nav"
            | "aside"
            | "form"
            | "button"
            | "input"
            | "select"
            | "textarea"
    )
}

fn render_wrapped(
    node: &NodeRef,
    output: &mut String,
    base: Option<&Url>,
    list_depth: usize,
    before: &str,
    after: &str,
) {
    output.push_str(before);
    render_children(node, output, base, list_depth);
    trim_trailing_inline_space(output);
    output.push_str(after);
}

fn render_inline_code(node: &NodeRef, output: &mut String) {
    let text = node.text_contents();
    let text = text.trim();
    if text.is_empty() {
        return;
    }

    let fence = if text.contains('`') { "``" } else { "`" };
    output.push_str(fence);
    output.push_str(text);
    output.push_str(fence);
}

fn render_code_block(node: &NodeRef, output: &mut String) {
    let text = node.text_contents();
    let text = text.trim_matches('\n');
    if text.trim().is_empty() {
        return;
    }

    ensure_blank_line(output);
    output.push_str("```");
    if let Some(language) = code_language(node) {
        output.push_str(&language);
    }
    output.push('\n');
    output.push_str(text);
    output.push_str("\n```");
    ensure_blank_line(output);
}

fn code_language(node: &NodeRef) -> Option<String> {
    let class = attr(node, "class").or_else(|| {
        node.select_first("code")
            .ok()
            .and_then(|code| attr(code.as_node(), "class"))
    })?;

    class.split_whitespace().find_map(|part| {
        part.strip_prefix("language-")
            .or_else(|| part.strip_prefix("lang-"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn render_link(node: &NodeRef, output: &mut String, base: Option<&Url>) {
    let Some(href) = attr(node, "href").map(|value| resolve_url(&value, base)) else {
        render_children(node, output, base, 0);
        return;
    };

    let label = normalize_inline(&node.text_contents());
    let label = if label.is_empty() {
        href.clone()
    } else {
        escape_markdown_text(&label)
    };

    output.push('[');
    output.push_str(&label);
    output.push_str("](");
    output.push_str(&escape_markdown_url(&href));
    output.push(')');
}

fn render_image(node: &NodeRef, output: &mut String, base: Option<&Url>) {
    let Some(src) = attr(node, "src").map(|value| resolve_url(&value, base)) else {
        return;
    };
    let alt = attr(node, "alt").unwrap_or_default();

    output.push_str("![");
    output.push_str(&escape_markdown_text(&alt));
    output.push_str("](");
    output.push_str(&escape_markdown_url(&src));
    output.push(')');
}

fn render_list(
    node: &NodeRef,
    output: &mut String,
    base: Option<&Url>,
    list_depth: usize,
    ordered: bool,
) {
    ensure_blank_line(output);
    let mut index = 1usize;

    for child in node.children() {
        if tag_name(&child).as_deref() != Some("li") {
            continue;
        }

        let mut item = String::new();
        render_children(&child, &mut item, base, list_depth + 1);
        let item = normalize_markdown(&item);
        if item.is_empty() {
            continue;
        }

        let indent = "  ".repeat(list_depth);
        let prefix = if ordered {
            let value = format!("{index}. ");
            index += 1;
            value
        } else {
            "- ".to_string()
        };

        for (line_index, line) in item.lines().enumerate() {
            ensure_line_start(output);
            output.push_str(&indent);
            if line_index == 0 {
                output.push_str(&prefix);
            } else {
                output.push_str(&" ".repeat(prefix.len()));
            }
            output.push_str(line);
            output.push('\n');
        }
    }

    ensure_blank_line(output);
}

fn render_blockquote(node: &NodeRef, output: &mut String, base: Option<&Url>, list_depth: usize) {
    let mut quoted = String::new();
    render_children(node, &mut quoted, base, list_depth);
    let quoted = normalize_markdown(&quoted);
    if quoted.is_empty() {
        return;
    }

    ensure_blank_line(output);
    for line in quoted.lines() {
        output.push_str("> ");
        output.push_str(line);
        output.push('\n');
    }
    ensure_blank_line(output);
}

fn render_table(node: &NodeRef, output: &mut String) -> bool {
    let mut rows = Vec::new();
    collect_table_rows(node, &mut rows);
    rows.retain(|row| !row.is_empty());
    if rows.is_empty() {
        return false;
    }

    let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    if column_count == 0 {
        return false;
    }

    ensure_blank_line(output);
    render_table_row(output, &rows[0], column_count);
    output.push('|');
    for _ in 0..column_count {
        output.push_str(" --- |");
    }
    output.push('\n');
    for row in rows.iter().skip(1) {
        render_table_row(output, row, column_count);
    }
    ensure_blank_line(output);

    true
}

fn collect_table_rows(node: &NodeRef, rows: &mut Vec<Vec<String>>) {
    for child in node.children() {
        match tag_name(&child).as_deref() {
            Some("tr") => {
                let cells = child
                    .children()
                    .filter_map(|cell| match tag_name(&cell).as_deref() {
                        Some("th") | Some("td") => {
                            Some(escape_table_cell(&normalize_inline(&cell.text_contents())))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                rows.push(cells);
            }
            Some("table") => {}
            _ => collect_table_rows(&child, rows),
        }
    }
}

fn render_table_row(output: &mut String, row: &[String], column_count: usize) {
    output.push('|');
    for index in 0..column_count {
        output.push(' ');
        output.push_str(row.get(index).map(String::as_str).unwrap_or(""));
        output.push_str(" |");
    }
    output.push('\n');
}

fn attr(node: &NodeRef, name: &str) -> Option<String> {
    node.as_element()?
        .attributes
        .borrow()
        .get(name)
        .map(ToString::to_string)
}

fn tag_name(node: &NodeRef) -> Option<String> {
    Some(
        node.as_element()?
            .name
            .local
            .to_string()
            .to_ascii_lowercase(),
    )
}

fn resolve_url(raw: &str, base: Option<&Url>) -> String {
    let raw = raw.trim();
    base.and_then(|url| url.join(raw).ok())
        .map(|url| url.to_string())
        .unwrap_or_else(|| raw.to_string())
}

fn push_text(output: &mut String, text: &str) {
    let mut pending_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            pending_space = true;
            continue;
        }

        if pending_space && should_insert_space(output) {
            output.push(' ');
        }
        pending_space = false;

        if matches!(ch, '\\' | '*' | '_' | '[' | ']' | '`') {
            output.push('\\');
        }
        output.push(ch);
    }

    if pending_space && should_insert_space(output) {
        output.push(' ');
    }
}

fn should_insert_space(output: &str) -> bool {
    output
        .chars()
        .last()
        .is_some_and(|ch| !ch.is_whitespace() && ch != '(' && ch != '[')
}

fn ensure_blank_line(output: &mut String) {
    trim_trailing_inline_space(output);
    if output.is_empty() || output.ends_with("\n\n") {
        return;
    }
    if output.ends_with('\n') {
        output.push('\n');
    } else {
        output.push_str("\n\n");
    }
}

fn ensure_line_start(output: &mut String) {
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
}

fn trim_trailing_inline_space(output: &mut String) {
    let trimmed_len = output.trim_end_matches([' ', '\t']).len();
    output.truncate(trimmed_len);
}

fn normalize_inline(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_markdown(text: &str) -> String {
    let mut output = String::new();
    let mut pending_blank = false;

    for raw_line in text.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            pending_blank = true;
            continue;
        }

        if !output.is_empty() {
            if pending_blank {
                output.push_str("\n\n");
            } else {
                output.push('\n');
            }
        }
        output.push_str(line);
        pending_blank = false;
    }

    output.trim().to_string()
}

fn escape_markdown_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(ch, '\\' | '*' | '_' | '[' | ']' | '`') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

fn escape_markdown_url(url: &str) -> String {
    url.replace(')', "%29").replace(' ', "%20")
}

fn escape_table_cell(text: &str) -> String {
    text.replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_to_markdown_removes_noise_and_formats_blocks() {
        let html = r#"<html><head><style>.x{}</style></head><body><h1>Hello</h1><script>alert(1)</script><p>World &amp; <strong>friends</strong></p><a href="/docs">Docs</a></body></html>"#;
        let markdown = html_to_markdown(html, "https://example.com/base/");

        assert!(markdown.contains("# Hello"));
        assert!(markdown.contains("World & **friends**"));
        assert!(markdown.contains("[Docs](https://example.com/docs)"));
        assert!(!markdown.contains("alert"));
    }

    #[test]
    fn html_to_markdown_formats_lists_code_and_tables() {
        let html = r#"
            <body>
              <ul><li>One</li><li>Two</li></ul>
              <pre><code class="language-rust">fn main() {}</code></pre>
              <table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>
            </body>
        "#;
        let markdown = html_to_markdown(html, "https://example.com");

        assert!(markdown.contains("- One"));
        assert!(markdown.contains("```rust\nfn main() {}\n```"));
        assert!(markdown.contains("| A | B |"));
        assert!(markdown.contains("| 1 | 2 |"));
    }

    #[test]
    fn parse_http_url_rejects_other_schemes() {
        assert!(parse_http_url("https://example.com").is_ok());
        assert!(parse_http_url("file:///etc/passwd").is_err());
    }
}
