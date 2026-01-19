//! Labels feature helpers.
//!
//! # Design
//! - Keep label form updates pure for predictable state transitions.
//! - Invariants: callers always receive a new state instance.
//! - Failure modes: none; update closures are trusted to mutate safely.

use crate::features::labels::state::LabelFormState;

/// Apply a label form update and return the new state.
#[must_use]
pub(crate) fn apply_label_form_update(
    current: &LabelFormState,
    update: impl FnOnce(&mut LabelFormState),
) -> LabelFormState {
    let mut next = current.clone();
    update(&mut next);
    next
}

#[cfg(test)]
mod tests {
    use super::apply_label_form_update;
    use crate::features::labels::state::LabelFormState;

    #[test]
    fn apply_label_form_update_modifies_state() {
        let initial = LabelFormState::default();
        let updated = apply_label_form_update(&initial, |state| {
            state.cleanup_remove_data = true;
        });
        assert!(updated.cleanup_remove_data);
    }
}
