//! Navigation helpers for the shell UI.
//!
//! # Design
//! - Provide minimal, pure helpers for view state classification.
//! - Invariants: returns only known class tokens for the nav styles.
//! - Failure modes: none; inputs map directly to class names.

/// Return the menu item state class name.
#[must_use]
pub(crate) const fn menu_item_state_class(active: bool) -> &'static str {
    if active { "active" } else { "false" }
}

#[cfg(test)]
mod tests {
    use super::menu_item_state_class;

    #[test]
    fn menu_item_state_class_switches() {
        assert_eq!(menu_item_state_class(true), "active");
        assert_eq!(menu_item_state_class(false), "false");
    }
}
