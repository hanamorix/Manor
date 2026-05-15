//! Per-kind approver modules.
//!
//! Phase 1.D — each module exposes a uniform
//! `approve(tx, proposal_id, diff[, today_override]) -> Result<usize, ApplyError>`
//! signature. The proposal_registry (Task 1.C) will dispatch by `kind` into
//! these modules. For now, the legacy free fns in `super` (`approve_add_task`,
//! `approve_add_maintenance_schedule`) are thin shims that wrap a transaction
//! and delegate here, preserving every existing call site and test.
//!
//! Override-mode (used today by `approve_add_maintenance_schedule_with_override`
//! for the proposal-edit Drawer) is not split out yet — Task 1.E generalises
//! that into `approve_with_override`.

pub mod add_chore;
pub mod add_contract;
pub mod add_ledger_transaction;
pub mod add_maintenance_schedule;
pub mod add_recurring_block;
pub mod add_recurring_payment;
pub mod add_task;
pub mod add_time_block;
pub mod add_to_shopping_list;
pub mod complete_chore;
pub mod complete_task;
pub mod set_budget;
