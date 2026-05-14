//! Tool schemas declared to Ollama for function calling.

use serde_json::json;

/// JSON schema for the `add_task` tool — Manor's only tool in Phase 3a.
pub fn add_task_tool() -> serde_json::Value {
    json!({
        "type": "function",
        "function": {
            "name": "add_task",
            "description": "Propose adding a task to the user's task list. \
                            Use when the user asks you to remember, remind, or \
                            track something. Do not claim to have added it; the \
                            user must approve the proposal first.",
            "parameters": {
                "type": "object",
                "required": ["title"],
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "A short imperative — e.g. 'Pick up prescription'"
                    },
                    "due_date": {
                        "type": "string",
                        "description": "Optional. ISO date, format YYYY-MM-DD. Omit for today."
                    }
                }
            }
        }
    })
}

pub fn add_chore_tool() -> serde_json::Value {
    json!({
        "type": "function",
        "function": {
            "name": "add_chore",
            "description": "Propose adding one or more recurring household chores. \
                            Use for repeated household work like dishes, bins, laundry, \
                            cleaning, alternating duties, or named-person rotations. \
                            Do not use add_task for recurring chores.",
            "parameters": {
                "oneOf": [
                    { "$ref": "#/$defs/chore" },
                    {
                        "type": "array",
                        "items": { "$ref": "#/$defs/chore" },
                        "minItems": 1
                    }
                ],
                "$defs": {
                    "chore": {
                        "type": "object",
                        "required": ["title", "rrule"],
                        "properties": {
                            "title": {
                                "type": "string",
                                "description": "Short chore title, e.g. 'Do dishes'"
                            },
                            "emoji": {
                                "type": "string",
                                "description": "Optional user-content marker. Use '.' when unsure."
                            },
                            "rrule": {
                                "type": "string",
                                "description": "RFC 5545 RRULE like FREQ=WEEKLY;BYDAY=MO, or a casual phrase like weekly/every Monday/alternating."
                            },
                            "first_due_ms": {
                                "type": "integer",
                                "description": "Optional first due date as Unix milliseconds. Omit when unclear."
                            },
                            "rotation_names": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Optional household member names in round-robin order."
                            }
                        }
                    }
                }
            }
        }
    })
}

/// All tools available in Phase 3a.
pub fn all_tools() -> Vec<serde_json::Value> {
    vec![add_task_tool(), add_chore_tool()]
}
