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

/// All tools available in Phase 3a.
pub fn all_tools() -> Vec<serde_json::Value> {
    vec![add_task_tool()]
}
