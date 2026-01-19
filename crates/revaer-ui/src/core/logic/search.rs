//! Search input helpers.
//!
//! # Design
//! - Keep keyboard handling logic pure and reusable across inputs.
//! - Invariants: only `Enter` with a modifier triggers a submit signal.
//! - Failure modes: invalid keys or modifiers safely return false.

/// Determine if a key press should submit the search immediately.
#[must_use]
pub(crate) fn should_submit_on_key(key: &str, ctrl: bool, meta: bool) -> bool {
    key == "Enter" && (ctrl || meta)
}

/// Return true when debounce should schedule delayed emissions.
#[must_use]
pub(crate) const fn debounce_enabled(debounce_ms: u32) -> bool {
    debounce_ms > 0
}

#[cfg(test)]
mod tests {
    use super::{debounce_enabled, should_submit_on_key};

    #[test]
    fn should_submit_on_key_requires_modifier() {
        assert!(!should_submit_on_key("Enter", false, false));
        assert!(should_submit_on_key("Enter", true, false));
        assert!(should_submit_on_key("Enter", false, true));
        assert!(!should_submit_on_key("Space", true, false));
    }

    #[test]
    fn debounce_enabled_requires_positive() {
        assert!(!debounce_enabled(0));
        assert!(debounce_enabled(150));
    }
}
