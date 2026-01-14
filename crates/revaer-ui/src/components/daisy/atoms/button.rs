use crate::components::daisy::foundations::{DaisyColor, DaisySize, DaisyVariant, tone_class};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ButtonProps {
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub tone: Option<DaisyColor>,
    #[prop_or(DaisySize::Md)]
    pub size: DaisySize,
    #[prop_or(DaisyVariant::Solid)]
    pub variant: DaisyVariant,
    #[prop_or_default]
    pub full_width: bool,
    #[prop_or_default]
    pub circle: bool,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub loading: bool,
    #[prop_or_default]
    pub leading_icon: Option<Html>,
    #[prop_or_default]
    pub trailing_icon: Option<Html>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub button_type: Option<AttrValue>,
    #[prop_or_default]
    pub onclick: Callback<MouseEvent>,
}

#[function_component(Button)]
pub fn button(props: &ButtonProps) -> Html {
    let tone = tone_class("btn", props.tone);
    let size = props.size.with_prefix("btn");
    let mut classes = classes!(
        "btn",
        props.variant.as_class(),
        size,
        props.full_width.then_some("btn-block"),
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
            type={props.button_type.clone()}
            onclick={props.onclick.clone()}
        >
            {props.leading_icon.clone().map(|icon| html! {
                <span class="btn-icon leading">{icon}</span>
            }).unwrap_or_default()}
            { for props.children.iter() }
            {props.trailing_icon.clone().map(|icon| html! {
                <span class="btn-icon trailing">{icon}</span>
            }).unwrap_or_default()}
        </button>
    }
}
