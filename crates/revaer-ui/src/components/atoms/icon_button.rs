use crate::components::daisy::foundations::{DaisyColor, DaisySize, DaisyVariant, tone_class};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct IconButtonProps {
    pub icon: Html,
    pub label: AttrValue,
    #[prop_or_default]
    pub tone: Option<DaisyColor>,
    #[prop_or(DaisySize::Sm)]
    pub size: DaisySize,
    #[prop_or(DaisyVariant::Ghost)]
    pub variant: DaisyVariant,
    #[prop_or_default]
    pub circle: bool,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub loading: bool,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub r#type: Option<AttrValue>,
    #[prop_or_default]
    pub onclick: Callback<MouseEvent>,
}

#[function_component(IconButton)]
pub(crate) fn icon_button(props: &IconButtonProps) -> Html {
    let tone = tone_class("btn", props.tone);
    let size = props.size.with_prefix("btn");
    let mut classes = classes!(
        "btn",
        props.variant.as_class(),
        size,
        props.circle.then_some("btn-circle"),
        props.loading.then_some("loading"),
        props.class.clone()
    );
    if let Some(tone) = tone {
        classes.push(tone);
    }

    html! {
        <button
            class={classes}
            disabled={props.disabled || props.loading}
            r#type={props.r#type.clone()}
            onclick={props.onclick.clone()}
            aria-label={props.label.clone()}
        >
            {props.icon.clone()}
        </button>
    }
}
