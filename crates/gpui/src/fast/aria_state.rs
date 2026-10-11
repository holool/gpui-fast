//! Accessibility states the gpui-arabic fork offered and upstream GPUI lacks:
//! a disabled control, hidden content and a modal dialog.
//!
//! Holool API extension (docs/upstream-sync.md): the setters are
//! [`AriaStateExt`], a trait every stateful element implements, exported from
//! the crate root. The states live in one field of upstream's
//! `AriaProperties` and reach AccessKit through one hook in
//! `Interactivity::write_a11y_info`.

use crate::StatefulInteractiveElement;

/// The three states, all off by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct AriaStates {
    disabled: bool,
    hidden: bool,
    modal: bool,
}

impl AriaStates {
    /// No state set; the value an element without accessibility properties reads.
    pub(crate) const NONE: Self = Self {
        disabled: false,
        hidden: false,
        modal: false,
    };
}

/// Writes the states that are set onto an AccessKit node.
pub(crate) fn write(states: &AriaStates, node: &mut accesskit::Node) {
    if states.disabled {
        node.set_disabled();
    }
    if states.hidden {
        node.set_hidden();
    }
    if states.modal {
        node.set_modal();
    }
}

/// Disabled, hidden and modal accessibility states for any stateful element.
pub trait AriaStateExt: StatefulInteractiveElement {
    /// Mark this control as disabled for assistive technologies. It stays
    /// in the accessibility tree; activation is still the element's to refuse.
    fn aria_disabled(mut self, disabled: bool) -> Self {
        self.interactivity().aria.fast_states.disabled = disabled;
        self
    }

    /// Hide this element and its descendants from assistive technologies,
    /// for content that is laid out but not shown (for example collapsed).
    fn aria_hidden(mut self) -> Self {
        self.interactivity().aria.fast_states.hidden = true;
        self
    }

    /// Mark a dialog as modal: assistive technologies treat the content
    /// outside it as inert while it is open.
    fn aria_modal(mut self, modal: bool) -> Self {
        self.interactivity().aria.fast_states.modal = modal;
        self
    }
}

impl<E: StatefulInteractiveElement> AriaStateExt for E {}

#[cfg(test)]
mod tests {
    use super::AriaStateExt;
    use crate::{InteractiveElement, div};

    #[test]
    fn disabled_hidden_and_modal_states_reach_the_accessibility_node() {
        let mut element = div()
            .id("dialog")
            .aria_disabled(true)
            .aria_hidden()
            .aria_modal(true);
        let mut node = accesskit::Node::new(accesskit::Role::Dialog);
        element.interactivity().write_a11y_info(&mut node);
        assert!(node.is_disabled(), "a disabled control must say so");
        assert!(node.is_hidden(), "hidden content must leave the tree");
        assert!(node.is_modal(), "a modal dialog must say so");

        let mut plain = accesskit::Node::new(accesskit::Role::Button);
        let mut element = div().id("plain");
        element.interactivity().write_a11y_info(&mut plain);
        assert!(!plain.is_disabled() && !plain.is_hidden() && !plain.is_modal());
    }
}
