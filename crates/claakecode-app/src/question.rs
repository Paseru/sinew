use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use claakecode_core::ToolDescriptor;

use crate::{agent::QuestionReply, tool_run::ToolRunResult, TurnCancel};

#[derive(Debug, Clone, Default)]
pub struct QuestionTool;

impl QuestionTool {
    pub fn new() -> Self {
        Self
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        let question_schema = json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question shown to the user."
                },
                "type": {
                    "type": "string",
                    "enum": ["single_choice", "multiple_choice"],
                    "description": "single_choice lets the user select one option. multiple_choice lets the user select several options."
                },
                "options": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {
                                "type": "string",
                                "description": "Stable optional option id."
                            },
                            "label": {
                                "type": "string",
                                "description": "Visible option label."
                            },
                            "description": {
                                "type": "string",
                                "description": "Optional short supporting text for the option."
                            }
                        },
                        "required": ["label"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["question", "type", "options"],
            "additionalProperties": false
        });
        ToolDescriptor {
            name: "Question".into(),
            description: "Ask the user one or more questions in the chat UI. Use this when you need choices, preferences, or clarifications before continuing. You may pass either a single question with question/type/options, or multiple independent questions with questions[]. After calling it, wait for the user's next message.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "The question shown to the user."
                    },
                    "type": {
                        "type": "string",
                        "enum": ["single_choice", "multiple_choice"],
                        "description": "single_choice lets the user select one option. multiple_choice lets the user select several options."
                    },
                    "options": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "string",
                                    "description": "Stable optional option id."
                                },
                                "label": {
                                    "type": "string",
                                    "description": "Visible option label."
                                },
                                "description": {
                                    "type": "string",
                                    "description": "Optional short supporting text for the option."
                                }
                            },
                            "required": ["label"],
                            "additionalProperties": false
                        }
                    },
                    "questions": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 6,
                        "description": "Multiple independent questions to show in one UI block. Each question can have its own type and options.",
                        "items": question_schema
                    }
                },
                "additionalProperties": false
            }),
        }
    }

    pub async fn run(
        &self,
        tool_call_id: &str,
        input: Value,
        cancel: &TurnCancel,
    ) -> ToolRunResult {
        let parsed: QuestionInput = match serde_json::from_value(input) {
            Ok(value) => value,
            Err(err) => {
                return ToolRunResult::err(format!("invalid Question input: {err}"), Vec::new())
            }
        };

        let questions = match parsed.into_questions() {
            Ok(questions) => questions,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };

        for question in &questions {
            if question.question.trim().is_empty() {
                return ToolRunResult::err("question is required", Vec::new());
            }
            if question.options.is_empty() {
                return ToolRunResult::err("at least one option is required", Vec::new());
            }
            for option in &question.options {
                if option.label.trim().is_empty() {
                    return ToolRunResult::err("option labels cannot be empty", Vec::new());
                }
            }
        }

        let receiver = match cancel.register_question(tool_call_id.to_string()) {
            Ok(receiver) => receiver,
            Err(()) => return ToolRunResult::err("question was cancelled", Vec::new()),
        };

        match receiver.await {
            Ok(QuestionReply::Answer { answers, stop }) => {
                let normalized = normalize_answers(&questions, answers);
                let formatted = questions
                    .iter()
                    .enumerate()
                    .map(|(idx, question)| {
                        let answer = normalized
                            .get(idx)
                            .filter(|answers| !answers.is_empty())
                            .map(|answers| answers.join(", "))
                            .unwrap_or_else(|| "Unanswered".to_string());
                        format!("\"{}\"=\"{}\"", question.question.trim(), answer)
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut output = format!(
                    "User has answered your question{}: {formatted}. Continue with the user's answers in mind.",
                    if questions.len() == 1 { "" } else { "s" },
                );
                if stop {
                    output.push_str(" The user clicked Send and stop questions. Do not ask more questions in this turn.");
                }
                ToolRunResult::ok_with_meta(
                    output,
                    Vec::new(),
                    json!({
                        "question_answers": normalized,
                        "question_stop_requested": stop,
                    }),
                )
            }
            Ok(QuestionReply::Rejected) => {
                ToolRunResult::err("The user dismissed this question", Vec::new())
            }
            Err(_) => {
                cancel.unregister_question(tool_call_id);
                ToolRunResult::err("question was cancelled", Vec::new())
            }
        }
    }
}

fn normalize_answers(questions: &[QuestionPrompt], answers: Vec<Vec<String>>) -> Vec<Vec<String>> {
    questions
        .iter()
        .enumerate()
        .map(|(idx, question)| {
            let mut answer = answers.get(idx).cloned().unwrap_or_default();
            answer = answer
                .into_iter()
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect();
            if !matches!(question.kind, QuestionKind::MultipleChoice) {
                answer.truncate(1);
            }
            answer
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct QuestionInput {
    question: Option<String>,
    #[serde(rename = "type")]
    kind: Option<QuestionKind>,
    #[serde(default)]
    options: Vec<QuestionOption>,
    #[serde(default)]
    questions: Vec<QuestionPrompt>,
}

impl QuestionInput {
    fn into_questions(self) -> Result<Vec<QuestionPrompt>, String> {
        if !self.questions.is_empty() {
            return Ok(self.questions);
        }
        let question = self
            .question
            .ok_or_else(|| "question is required".to_string())?;
        let kind = self.kind.ok_or_else(|| "type is required".to_string())?;
        Ok(vec![QuestionPrompt {
            question,
            kind,
            options: self.options,
        }])
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct QuestionPrompt {
    question: String,
    #[serde(rename = "type")]
    kind: QuestionKind,
    options: Vec<QuestionOption>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum QuestionKind {
    SingleChoice,
    MultipleChoice,
}

#[derive(Debug, Deserialize, Serialize)]
struct QuestionOption {
    id: Option<String>,
    label: String,
    description: Option<String>,
}
