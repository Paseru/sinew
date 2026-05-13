use super::*;

impl TeamTool {
    pub fn descriptors(&self) -> Vec<ToolDescriptor> {
        if self.current_agent.is_some() {
            Self::agent_descriptors_static()
        } else {
            Self::descriptors_static()
        }
    }

    pub fn descriptors_static() -> Vec<ToolDescriptor> {
        vec![
            ToolDescriptor {
                name: TEAM_RUN_TOOL.into(),
                description: "Launch an agent team. Use this for swarms/agent teams.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "objective": {
                            "type": "string",
                            "description": "Full user objective for a new team. Required unless relaunching an existing teammate with agent."
                        },
                        "agent": {
                            "type": "string",
                            "description": "Teammate name to relaunch in the active team. When set, provide only agent."
                        },
                        "agent_names": {
                            "type": "array",
                            "minItems": 2,
                            "maxItems": 8,
                            "items": { "type": "string" },
                            "description": "Required teammate names when starting a new team."
                        },
                        "agent_profiles": {
                            "type": "array",
                            "description": "Optional profile assignments for teammates. Use this to make a team member inherit a configured sub-agent profile. Each agent must match agent_names; each profile is a configured sub-agent id or name.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "agent": {
                                        "type": "string",
                                        "description": "Teammate name from agent_names."
                                    },
                                    "profile": {
                                        "type": "string",
                                        "description": "Configured sub-agent id or name to use for that teammate."
                                    }
                                },
                                "required": ["agent", "profile"],
                                "additionalProperties": false
                            }
                        },
                        "agent_prompts": {
                            "description": "Optional teammate-specific launch prompts from the main agent. Each agent must match agent_names. These prompts are delivered only to that teammate alongside the shared objective.",
                            "oneOf": [
                                {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "agent": {
                                                "type": "string",
                                                "description": "Teammate name from agent_names."
                                            },
                                            "prompt": {
                                                "type": "string",
                                                "description": "Specific launch prompt for this teammate."
                                            }
                                        },
                                        "required": ["agent", "prompt"],
                                        "additionalProperties": false
                                    }
                                },
                                {
                                    "type": "object",
                                    "additionalProperties": { "type": "string" },
                                    "description": "Map from teammate name to that teammate's launch prompt."
                                }
                            ]
                        },
                        "tasks": {
                            "type": "array",
                            "description": "Optional initial shared task board for a new team. Prefer parallel, autonomous workstreams owned by different teammates. Avoid a fully sequential chain. Use blockedBy only for real dependency constraints (scaffold etc..).",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "subject": {
                                        "type": "string",
                                        "description": "Short task title."
                                    },
                                    "description": {
                                        "type": "string",
                                        "description": "Detailed task instructions."
                                    },
                                    "owner": {
                                        "type": "string",
                                        "description": "Optional teammate name that should own this task."
                                    },
                                    "blockedBy": {
                                        "type": "array",
                                        "items": {
                                            "oneOf": [
                                                { "type": "integer", "minimum": 1 },
                                                { "type": "string" }
                                            ]
                                        },
                                        "description": "Initial task IDs this task waits on. IDs are assigned by task order starting at 1."
                                    },
                                    "blocked_by": {
                                        "type": "array",
                                        "items": {
                                            "oneOf": [
                                                { "type": "integer", "minimum": 1 },
                                                { "type": "string" }
                                            ]
                                        },
                                        "description": "Alias for blockedBy."
                                    }
                                },
                                "required": ["subject"],
                                "additionalProperties": false
                            }
                        }
                    },
                    "additionalProperties": false
                }),
            },
            ToolDescriptor {
                name: TEAM_STATUS_TOOL.into(),
                description: "Inspect the active agent team, teammates, queued messages, tasks, and latest summaries.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            },
            ToolDescriptor {
                name: TEAM_STOP_TOOL.into(),
                description: "Stop one teammate or stop an entire agent team.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "agent": {
                            "type": "string",
                            "description": "Optional teammate name to stop. Omit to stop every teammate in the team."
                        }
                    },
                    "additionalProperties": false
                }),
            },
        ]
    }

    pub fn agent_descriptors_static() -> Vec<ToolDescriptor> {
        vec![
            ToolDescriptor {
                name: SEND_MESSAGE_TOOL.into(),
                description: "Send a live async message to a teammate by name, or broadcast to all teammates with to=\"*\". Peers receive messages at their next model turn.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "to": { "type": "string", "description": "Teammate name, without @. Use * for broadcast." },
                        "message": { "type": "string", "description": "Plain text peer message." }
                    },
                    "required": ["to", "message"],
                    "additionalProperties": false
                }),
            },
            ToolDescriptor {
                name: TASK_LIST_TOOL.into(),
                description: "Mutate the agent team's shared task board. The latest board is injected as a system reminder before every model call, so do not call this tool just to inspect tasks. Tasks with unresolved blockedBy dependencies stay blocked; update/delete are allowed, but status cannot become pending, in_progress, or completed until dependencies finish.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["create", "update", "delete", "claim"],
                            "description": "Operation to perform. claim assigns ownership only; use update with status=in_progress when actually starting work."
                        },
                        "status": {
                            "type": "string",
                            "enum": ["pending", "in_progress", "blocked", "completed"],
                            "description": "For action=update, new task status. Tasks with unresolved blockedBy dependencies cannot be set to pending, in_progress, or completed."
                        },
                        "taskId": {
                            "oneOf": [{ "type": "integer", "minimum": 1 }, { "type": "string" }],
                            "description": "Task ID for update/delete/claim."
                        },
                        "id": {
                            "oneOf": [{ "type": "integer", "minimum": 1 }, { "type": "string" }],
                            "description": "Alias for taskId."
                        },
                        "subject": {
                            "type": "string",
                            "description": "Short task title for create/update."
                        },
                        "owner": {
                            "type": "string",
                            "description": "Owner teammate name for create/update/claim. Claim does not change task status."
                        },
                        "clear_owner": {
                            "type": "boolean",
                            "description": "For action=update, clear the current owner."
                        },
                        "description": {
                            "type": "string",
                            "description": "Detailed task instructions for create/update. Empty string clears it on update."
                        },
                        "addBlockedBy": {
                            "type": "array",
                            "items": { "type": "integer", "minimum": 1 },
                            "description": "Additional task IDs that block this task, e.g. [1, 3, 4]. Use this for task dependencies."
                        },
                        "blockedBy": {
                            "type": "array",
                            "items": { "type": "integer", "minimum": 1 },
                            "description": "Replace dependencies with task IDs only, e.g. [1, 3, 4]. Unresolved dependencies keep the task blocked and clear automatically only after those tasks complete."
                        }
                    },
                    "required": ["action"],
                    "additionalProperties": false
                }),
            },
        ]
    }

    pub fn summary_for_tool_name(&self, name: &str) -> Option<String> {
        match name {
            TEAM_RUN_TOOL => Some("Agent Swarm · run".to_string()),
            TEAM_CREATE_TOOL => Some("Agent Swarm · disabled create".to_string()),
            AGENT_TOOL => Some("Agent Swarm · disabled agent spawn".to_string()),
            SEND_MESSAGE_TOOL => Some("Agent Swarm · message".to_string()),
            TASK_CREATE_TOOL => Some("Task · create".to_string()),
            TASK_UPDATE_TOOL => Some("Task · update".to_string()),
            TEAM_STATUS_TOOL => Some("Agent Swarm · status".to_string()),
            TEAM_STOP_TOOL => Some("Agent Swarm · stop".to_string()),
            _ => None,
        }
    }
}

pub fn is_team_tool_name(name: &str) -> bool {
    matches!(
        name,
        TEAM_RUN_TOOL
            | TEAM_CREATE_TOOL
            | AGENT_TOOL
            | SEND_MESSAGE_TOOL
            | TASK_CREATE_TOOL
            | TASK_LIST_TOOL
            | TASK_UPDATE_TOOL
            | TEAM_STATUS_TOOL
            | TEAM_STOP_TOOL
    )
}
