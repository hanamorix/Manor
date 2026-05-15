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

pub fn complete_task_tool() -> serde_json::Value {
    json!({
        "type": "function",
        "function": {
            "name": "complete_task",
            "description": "Propose marking an existing one-off task complete. \
                            Use task_id when known; otherwise use an exact open-task title. \
                            Do not use this for recurring chores.",
            "parameters": {
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "integer",
                        "description": "Preferred. Existing open task id."
                    },
                    "title": {
                        "type": "string",
                        "description": "Exact open task title when task_id is unavailable."
                    }
                }
            }
        }
    })
}

pub fn add_event_tool() -> serde_json::Value {
    json!({
        "type": "function",
        "function": {
            "name": "add_event",
            "description": "Propose creating one or more calendar events through the user's configured CalDAV calendar. \
                            Use this for appointments, meetings, reminders that belong on the calendar, and scheduled events. \
                            Do not use add_task for calendar events.",
            "parameters": {
                "oneOf": [
                    { "$ref": "#/$defs/event" },
                    {
                        "type": "array",
                        "items": { "$ref": "#/$defs/event" },
                        "minItems": 1
                    }
                ],
                "$defs": {
                    "event": {
                        "type": "object",
                        "required": ["title", "start_at", "end_at"],
                        "properties": {
                            "account_id": {
                                "type": "integer",
                                "description": "Optional calendar account id. Omit to use the single configured default calendar."
                            },
                            "calendar_url": {
                                "type": "string",
                                "description": "Optional CalDAV calendar collection URL. Omit to use the account default."
                            },
                            "title": {
                                "type": "string",
                                "description": "Event title."
                            },
                            "start_at": {
                                "type": "integer",
                                "description": "UTC start time as Unix seconds."
                            },
                            "end_at": {
                                "type": "integer",
                                "description": "UTC end time as Unix seconds."
                            },
                            "description": {
                                "type": "string",
                                "description": "Optional notes."
                            },
                            "location": {
                                "type": "string",
                                "description": "Optional location."
                            },
                            "all_day": {
                                "type": "boolean",
                                "description": "True for all-day events."
                            }
                        }
                    }
                }
            }
        }
    })
}

pub fn add_transaction_tool() -> serde_json::Value {
    json!({
        "type": "function",
        "function": {
            "name": "add_transaction",
            "description": "Propose recording a manual Ledger transaction. \
                            Use negative amounts for spending and positive amounts for income. \
                            Amounts are integer pence, or strings like -£12.40.",
            "parameters": {
                "type": "object",
                "required": ["amount_pence", "description"],
                "properties": {
                    "amount_pence": {
                        "description": "Signed amount. Spending is negative; income is positive. Accepts pence integer or string like -£12.40.",
                        "oneOf": [{ "type": "integer" }, { "type": "number" }, { "type": "string" }]
                    },
                    "currency": {
                        "type": "string",
                        "description": "Currency code. Defaults to GBP."
                    },
                    "description": {
                        "type": "string",
                        "description": "Transaction description, e.g. Tesco Express."
                    },
                    "merchant": {
                        "type": "string",
                        "description": "Optional merchant name."
                    },
                    "category_id": {
                        "type": "integer",
                        "description": "Optional existing Ledger category id."
                    },
                    "category_name": {
                        "type": "string",
                        "description": "Optional exact existing Ledger category name."
                    },
                    "date": {
                        "type": "integer",
                        "description": "Optional Unix timestamp in seconds. Omit for now."
                    },
                    "note": {
                        "type": "string",
                        "description": "Optional note."
                    }
                }
            }
        }
    })
}

pub fn set_budget_tool() -> serde_json::Value {
    json!({
        "type": "function",
        "function": {
            "name": "set_budget",
            "description": "Propose setting a monthly Ledger budget for an existing category. \
                            Amounts are positive integer pence, or strings like £400.",
            "parameters": {
                "type": "object",
                "required": ["amount_pence"],
                "properties": {
                    "category_id": {
                        "type": "integer",
                        "description": "Optional existing Ledger category id."
                    },
                    "category_name": {
                        "type": "string",
                        "description": "Exact existing Ledger category name, e.g. Groceries."
                    },
                    "amount_pence": {
                        "description": "Positive monthly budget amount. Accepts pence integer or string like £400.",
                        "oneOf": [{ "type": "integer" }, { "type": "number" }, { "type": "string" }]
                    }
                }
            }
        }
    })
}

pub fn add_recurring_payment_tool() -> serde_json::Value {
    json!({
        "type": "function",
        "function": {
            "name": "add_recurring_payment",
            "description": "Propose adding a recurring Ledger payment such as a bill or subscription. \
                            Amounts are positive integer pence, or strings like £12.99. \
                            The generated monthly transaction will be a debit.",
            "parameters": {
                "type": "object",
                "required": ["description", "amount_pence", "day_of_month"],
                "properties": {
                    "description": {
                        "type": "string",
                        "description": "Payment description, e.g. Netflix or Council Tax."
                    },
                    "amount_pence": {
                        "description": "Positive recurring amount. Accepts pence integer or string like £12.99.",
                        "oneOf": [{ "type": "integer" }, { "type": "number" }, { "type": "string" }]
                    },
                    "currency": {
                        "type": "string",
                        "description": "Currency code. Defaults to GBP."
                    },
                    "category_id": {
                        "type": "integer",
                        "description": "Optional existing Ledger category id."
                    },
                    "category_name": {
                        "type": "string",
                        "description": "Optional exact existing Ledger category name."
                    },
                    "day_of_month": {
                        "type": "integer",
                        "description": "Payment day of month, 1 through 28."
                    },
                    "note": {
                        "type": "string",
                        "description": "Optional note."
                    }
                }
            }
        }
    })
}

pub fn add_contract_tool() -> serde_json::Value {
    json!({
        "type": "function",
        "function": {
            "name": "add_contract",
            "description": "Propose adding a Ledger supplier contract with renewal tracking. \
                            Use for broadband, phone, insurance, utilities, and similar fixed-term services.",
            "parameters": {
                "type": "object",
                "required": ["provider", "monthly_cost_pence", "term_start", "term_end"],
                "properties": {
                    "provider": {
                        "type": "string",
                        "description": "Supplier name, e.g. Zen Internet."
                    },
                    "kind": {
                        "type": "string",
                        "description": "Contract kind such as broadband, phone, insurance, utility, or other. Defaults to other."
                    },
                    "description": {
                        "type": "string",
                        "description": "Optional description."
                    },
                    "monthly_cost_pence": {
                        "description": "Positive monthly cost. Accepts pence integer or string like £30.",
                        "oneOf": [{ "type": "integer" }, { "type": "number" }, { "type": "string" }]
                    },
                    "term_start": {
                        "type": "integer",
                        "description": "Contract start as Unix timestamp in seconds."
                    },
                    "term_end": {
                        "type": "integer",
                        "description": "Contract end or renewal date as Unix timestamp in seconds."
                    },
                    "exit_fee_pence": {
                        "description": "Optional non-negative exit fee. Accepts pence integer or string like £50.",
                        "oneOf": [{ "type": "integer" }, { "type": "number" }, { "type": "string" }]
                    },
                    "renewal_alert_days": {
                        "type": "integer",
                        "description": "Days before term_end to show renewal alert. Defaults to 30."
                    },
                    "recurring_payment_id": {
                        "type": "integer",
                        "description": "Optional existing recurring payment id linked to this contract."
                    },
                    "note": {
                        "type": "string",
                        "description": "Optional note."
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

pub fn complete_chore_tool() -> serde_json::Value {
    json!({
        "type": "function",
        "function": {
            "name": "complete_chore",
            "description": "Propose marking a household chore complete. \
                            Use chore_id when known; otherwise use an exact chore title. \
                            Do not claim completion until the user approves the proposal.",
            "parameters": {
                "type": "object",
                "properties": {
                    "chore_id": {
                        "type": "integer",
                        "description": "Preferred. Existing chore id."
                    },
                    "title": {
                        "type": "string",
                        "description": "Exact existing chore title when chore_id is unavailable."
                    },
                    "completed_by": {
                        "type": "integer",
                        "description": "Optional existing person id."
                    },
                    "completed_by_name": {
                        "type": "string",
                        "description": "Optional existing person name."
                    }
                }
            }
        }
    })
}

pub fn add_time_block_tool() -> serde_json::Value {
    json!({
        "type": "function",
        "function": {
            "name": "add_time_block",
            "description": "Propose adding a one-off Rhythm time block such as focus, admin, errands, or do-not-disturb time.",
            "parameters": {
                "type": "object",
                "required": ["title", "date_ms", "start_time", "end_time"],
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Short block title."
                    },
                    "kind": {
                        "type": "string",
                        "description": "Block kind. Prefer one of focus, admin, errands, dnd. Defaults to focus."
                    },
                    "date_ms": {
                        "type": "integer",
                        "description": "The calendar date at local midnight as Unix milliseconds."
                    },
                    "start_time": {
                        "type": "string",
                        "description": "Start time in 24-hour HH:MM."
                    },
                    "end_time": {
                        "type": "string",
                        "description": "End time in 24-hour HH:MM."
                    }
                }
            }
        }
    })
}

pub fn add_recurring_block_tool() -> serde_json::Value {
    json!({
        "type": "function",
        "function": {
            "name": "add_recurring_block",
            "description": "Propose adding a recurring Rhythm time block. \
                            Use for repeated focus/admin/errand/DND blocks rather than one-off events.",
            "parameters": {
                "type": "object",
                "required": ["title", "date_ms", "start_time", "end_time", "rrule"],
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Short block title."
                    },
                    "kind": {
                        "type": "string",
                        "description": "Block kind. Prefer one of focus, admin, errands, dnd. Defaults to focus."
                    },
                    "date_ms": {
                        "type": "integer",
                        "description": "First block date at local midnight as Unix milliseconds."
                    },
                    "start_time": {
                        "type": "string",
                        "description": "Start time in 24-hour HH:MM."
                    },
                    "end_time": {
                        "type": "string",
                        "description": "End time in 24-hour HH:MM."
                    },
                    "rrule": {
                        "type": "string",
                        "description": "RFC 5545 RRULE like FREQ=WEEKLY;BYDAY=MO, or a casual phrase like weekly/every weekday."
                    }
                }
            }
        }
    })
}

/// All tools available to the assistant.
pub fn all_tools() -> Vec<serde_json::Value> {
    vec![
        add_task_tool(),
        complete_task_tool(),
        add_event_tool(),
        add_transaction_tool(),
        set_budget_tool(),
        add_recurring_payment_tool(),
        add_contract_tool(),
        add_chore_tool(),
        complete_chore_tool(),
        add_time_block_tool(),
        add_recurring_block_tool(),
    ]
}
