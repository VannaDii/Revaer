//! Label feature actions.
//!
//! # Design
//! - Capture user intent separate from rendering.
//! - Actions are UI-only and never perform side effects.

/// High-level label list actions from the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LabelAction {
    /// Start creating a new label entry.
    New,
    /// Select a label entry by name.
    Select(String),
}
