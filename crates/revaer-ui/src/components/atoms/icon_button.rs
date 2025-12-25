use yew::prelude::*;

/// Props for icon-only buttons in the shell.
#[derive(Properties, PartialEq)]
pub(crate) struct IconButtonProps {
    /// Accessible label for the button.
    pub aria_label: AttrValue,
    /// Additional CSS classes.
    #[prop_or_default]
    pub class: Classes,
    /// Whether the button is disabled.
    #[prop_or_default]
    pub disabled: bool,
    /// Click handler.
    #[prop_or_default]
    pub onclick: Callback<MouseEvent>,
    /// Icon content.
    #[prop_or_default]
    pub children: Children,
}

#[function_component(IconButton)]
pub(crate) fn icon_button(props: &IconButtonProps) -> Html {
    let classes = classes!("icon-btn", "ghost", props.class.clone());
    html! {
        <button
            class={classes}
            type="button"
            aria-label={props.aria_label.clone()}
            onclick={props.onclick.clone()}
            disabled={props.disabled}
        >
            {for props.children.iter()}
        </button>
    }
}
