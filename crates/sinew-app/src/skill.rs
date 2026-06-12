use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sinew_core::ToolDescriptor;

use crate::tool_run::ToolRunResult;

const SKILL_TOOL_NAME: &str = "skill";
const SKILL_FILE_NAME: &str = "SKILL.md";

#[derive(Debug, Clone)]
pub struct SkillTool {
    workspace_root: PathBuf,
    settings: SkillSettings,
}

#[derive(Debug, Clone)]
struct SkillEntry {
    name: String,
    path: PathBuf,
}

impl SkillTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            settings: SkillSettings::default(),
        }
    }

    pub fn with_settings(workspace_root: impl Into<PathBuf>, settings: SkillSettings) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            settings: settings.normalized(),
        }
    }

    pub fn descriptor(&self) -> Option<ToolDescriptor> {
        let skills = self.discover();
        if skills.is_empty() {
            return None;
        }

        Some(ToolDescriptor {
            name: SKILL_TOOL_NAME.into(),
            description: format!(
                "Load one skill by name before using it. Available skills:\n{}",
                skills
                    .iter()
                    .map(|skill| format!("- {}", skill.name))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Exact skill name from the available skills list."
                    }
                },
                "required": ["name"],
                "additionalProperties": false
            }),
        })
    }

    pub async fn run(&self, input: Value) -> ToolRunResult {
        match self.load(input) {
            Ok(output) => ToolRunResult::ok(output, Vec::new()),
            Err(err) => ToolRunResult::err(err.to_string(), Vec::new()),
        }
    }

    fn load(&self, input: Value) -> Result<String> {
        let parsed: SkillInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid skill input: {err}"))?;
        let name = parsed.name.trim();
        if name.is_empty() {
            bail!("skill name is required");
        }

        let skills = self.discover();
        let Some(skill) = skills.iter().find(|skill| skill.name == name) else {
            let available = skills
                .iter()
                .map(|skill| skill.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            bail!("unknown skill `{name}`. Available skills: {available}");
        };

        let content = fs::read_to_string(&skill.path)
            .with_context(|| format!("unable to read skill `{name}`"))?;
        Ok(format!(
            "Loaded skill `{name}`:\n\n<skill name=\"{name}\">\n{}\n</skill>",
            content.trim()
        ))
    }

    fn discover(&self) -> Vec<SkillEntry> {
        let mut roots = vec![
            self.workspace_root.join(".agents/skills"),
            self.workspace_root.join(".sinew/skills"),
        ];
        if let Some(base_dirs) = BaseDirs::new() {
            let home = base_dirs.home_dir();
            roots.push(home.join(".agents/skills"));
            roots.push(home.join(".sinew/skills"));
        }

        let mut seen = HashSet::new();
        let mut skills = Vec::new();
        for root in roots {
            for skill in scan_skill_root(&root) {
                if self.settings.is_enabled(&skill.name) && seen.insert(skill.name.clone()) {
                    skills.push(skill);
                }
            }
        }
        skills.sort_by(|left, right| left.name.cmp(&right.name));
        skills
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSettings {
    #[serde(default)]
    pub skills: Vec<SkillConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillConfig {
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl SkillSettings {
    pub fn normalized(mut self) -> Self {
        let mut seen = HashSet::new();
        self.skills = self
            .skills
            .into_iter()
            .filter_map(|mut skill| {
                skill.name = skill.name.trim().to_string();
                if skill.name.is_empty() || !seen.insert(skill.name.clone()) {
                    return None;
                }
                Some(skill)
            })
            .collect();
        self
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        self.skills
            .iter()
            .find(|skill| skill.name == name)
            .map(|skill| skill.enabled)
            .unwrap_or(true)
    }
}

fn scan_skill_root(root: &Path) -> Vec<SkillEntry> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    let mut skills = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let skill_path = path.join(SKILL_FILE_NAME);
        if !skill_path.is_file() {
            continue;
        }

        let fallback_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("skill")
            .to_string();
        let name = fs::read_to_string(&skill_path)
            .ok()
            .and_then(|content| parse_skill_name(&content))
            .unwrap_or(fallback_name);

        skills.push(SkillEntry {
            name,
            path: skill_path,
        });
    }

    skills
}

fn parse_skill_name(content: &str) -> Option<String> {
    parse_frontmatter(content)
        .remove("name")
        .filter(|value| !value.is_empty())
}

fn parse_frontmatter(content: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return fields;
    }
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        fields.insert(
            key.trim().to_string(),
            clean_yaml_string(value.trim()).to_string(),
        );
    }
    fields
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillSource {
    Workspace,
    Global,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledSkill {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub source: SkillSource,
    pub root_label: String,
    pub absolute_path: String,
    pub content: String,
    pub enabled: bool,
}

pub fn list_installed_skills(
    workspace_root: impl AsRef<Path>,
    settings: &SkillSettings,
) -> Vec<InstalledSkill> {
    let workspace_root = workspace_root.as_ref();
    let home_dir = BaseDirs::new().map(|base| base.home_dir().to_path_buf());

    let mut roots: Vec<(SkillSource, PathBuf)> = vec![
        (
            SkillSource::Workspace,
            workspace_root.join(".agents/skills"),
        ),
        (SkillSource::Workspace, workspace_root.join(".sinew/skills")),
    ];
    if let Some(home) = home_dir.as_ref() {
        roots.push((SkillSource::Global, home.join(".agents/skills")));
        roots.push((SkillSource::Global, home.join(".sinew/skills")));
    }

    let mut seen = HashSet::new();
    let mut skills = Vec::new();

    for (source, root) in roots {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        let root_label = format_root_label(&root, workspace_root, home_dir.as_deref());

        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let skill_path = dir.join(SKILL_FILE_NAME);
            if !skill_path.is_file() {
                continue;
            }

            let Ok(content) = fs::read_to_string(&skill_path) else {
                continue;
            };
            let frontmatter = parse_frontmatter(&content);
            let fallback_name = dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("skill")
                .to_string();
            let name = frontmatter
                .get("name")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or(fallback_name);

            if !seen.insert(name.clone()) {
                continue;
            }

            let description = frontmatter
                .get("description")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            let enabled = settings.is_enabled(&name);

            skills.push(InstalledSkill {
                name,
                description,
                source,
                root_label: root_label.clone(),
                absolute_path: display_path_string(&skill_path),
                content,
                enabled,
            });
        }
    }

    skills.sort_by(|left, right| left.name.cmp(&right.name));
    skills
}

/// Create a new empty skill under `~/.agents/skills/<name>/SKILL.md`.
///
/// Picks a unique folder name (`new-skill`, `new-skill-1`, …) so the user can
/// just click "Add" without having to think about naming. Returns the path of
/// the freshly written `SKILL.md` along with the resolved skill name.
pub fn create_installed_skill() -> Result<(String, PathBuf)> {
    let base = BaseDirs::new()
        .map(|base| base.home_dir().to_path_buf())
        .context("unable to locate home directory")?
        .join(".agents/skills");
    fs::create_dir_all(&base)
        .with_context(|| format!("unable to create skills folder {}", base.display()))?;

    let (name, folder) = pick_unique_skill_folder(&base);
    fs::create_dir_all(&folder)
        .with_context(|| format!("unable to create skill folder {}", folder.display()))?;

    let skill_path = folder.join(SKILL_FILE_NAME);
    if skill_path.exists() {
        bail!("skill file already exists at {}", skill_path.display());
    }
    let template = default_skill_template(&name);
    fs::write(&skill_path, template)
        .with_context(|| format!("unable to write {}", skill_path.display()))?;

    Ok((name, skill_path))
}

fn pick_unique_skill_folder(base: &Path) -> (String, PathBuf) {
    const STEM: &str = "new-skill";
    let candidate = base.join(STEM);
    if !candidate.exists() {
        return (STEM.to_string(), candidate);
    }
    let mut index = 1u32;
    loop {
        let name = format!("{STEM}-{index}");
        let candidate = base.join(&name);
        if !candidate.exists() {
            return (name, candidate);
        }
        index = index.saturating_add(1);
        if index == u32::MAX {
            // Extremely unlikely; fall back to a timestamp suffix.
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or_default();
            let name = format!("{STEM}-{stamp}");
            return (name.clone(), base.join(name));
        }
    }
}

fn default_skill_template(name: &str) -> String {
    format!(
        "---\nname: {name}\ndescription: Describe what this skill helps with\n---\n\n# {name}\n\nWrite the instructions the agent should follow when this skill is enabled.\n"
    )
}

fn display_path_string(path: &Path) -> String {
    let raw = path.display().to_string();
    if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = raw.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    raw
}

fn format_root_label(root: &Path, workspace_root: &Path, home_dir: Option<&Path>) -> String {
    if let Ok(rel) = root.strip_prefix(workspace_root) {
        return rel.display().to_string();
    }
    if let Some(home) = home_dir {
        if let Ok(rel) = root.strip_prefix(home) {
            return format!("~/{}", rel.display());
        }
    }
    root.display().to_string()
}

fn clean_yaml_string(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedSkillImport {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSkillsResult {
    pub imported: Vec<String>,
    pub skipped: Vec<SkippedSkillImport>,
}

/// Copy skills from Claude Code or Codex/ChatGPT install locations into
/// `<workspace>/.agents/skills/`, which Sinew scans as workspace skills.
pub fn import_skills_from_provider(
    workspace_root: impl AsRef<Path>,
    provider: &str,
) -> Result<ImportSkillsResult> {
    let workspace_root = workspace_root.as_ref();
    let dest_base = workspace_root.join(".agents/skills");
    fs::create_dir_all(&dest_base)
        .with_context(|| format!("unable to create {}", dest_base.display()))?;

    let home = BaseDirs::new()
        .map(|base| base.home_dir().to_path_buf())
        .context("unable to locate home directory")?;

    let sources = provider_import_roots(provider, workspace_root, &home)?;
    let dest_canon = canonicalize_lossy(&dest_base);

    let mut imported = Vec::new();
    let mut skipped = Vec::new();
    let mut seen_dirs = HashSet::new();

    for source_root in sources {
        if !source_root.is_dir() {
            continue;
        }
        if canonicalize_lossy(&source_root) == dest_canon {
            continue;
        }

        for entry in scan_skill_root(&source_root) {
            let Some(skill_dir) = entry.path.parent() else {
                continue;
            };
            let dir_key = canonicalize_lossy(skill_dir);
            if !seen_dirs.insert(dir_key) {
                continue;
            }

            let folder_name = skill_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(entry.name.as_str());
            if folder_name.starts_with('.') {
                continue;
            }

            let dest_dir = dest_base.join(folder_name);
            if dest_dir.exists() {
                skipped.push(SkippedSkillImport {
                    name: entry.name,
                    reason: format!(
                        "folder already exists at {}",
                        dest_dir.strip_prefix(workspace_root)
                            .unwrap_or(&dest_dir)
                            .display()
                    ),
                });
                continue;
            }

            match copy_dir_recursive(skill_dir, &dest_dir) {
                Ok(()) => imported.push(folder_name.to_string()),
                Err(err) => skipped.push(SkippedSkillImport {
                    name: entry.name,
                    reason: err.to_string(),
                }),
            }
        }
    }

    imported.sort_unstable();
    Ok(ImportSkillsResult { imported, skipped })
}

fn provider_import_roots(
    provider: &str,
    workspace_root: &Path,
    home: &Path,
) -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();
    match provider.trim().to_ascii_lowercase().as_str() {
        "claude" => {
            push_existing_dir(&mut roots, home.join(".claude/skills"));
            push_existing_dir(&mut roots, workspace_root.join(".claude/skills"));
        }
        "codex" | "chatgpt" | "openai" => {
            push_existing_dir(&mut roots, home.join(".agents/skills"));
            if let Ok(codex_home) = std::env::var("CODEX_HOME") {
                let trimmed = codex_home.trim();
                if !trimmed.is_empty() {
                    push_existing_dir(&mut roots, PathBuf::from(trimmed).join("skills"));
                }
            }
            push_existing_dir(&mut roots, home.join(".codex/skills"));
            // workspace `.agents/skills` is the import destination — skip as source
            push_existing_dir(&mut roots, workspace_root.join(".codex/skills"));
        }
        other => bail!("unknown skill import provider `{other}` (expected claude or codex)"),
    }
    Ok(roots)
}

fn push_existing_dir(roots: &mut Vec<PathBuf>, path: PathBuf) {
    if path.is_dir() {
        roots.push(path);
    }
}

fn canonicalize_lossy(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !src.is_dir() {
        bail!("{} is not a directory", src.display());
    }
    fs::create_dir_all(dst)
        .with_context(|| format!("unable to create {}", dst.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("unable to read {}", src.display()))? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to)
                .with_context(|| format!("unable to copy {} to {}", from.display(), to.display()))?;
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct SkillInput {
    name: String,
}
