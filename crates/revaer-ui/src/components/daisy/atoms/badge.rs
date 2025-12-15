use crate::components::daisy::foundations::{DaisyColor, DaisySize, tone_class};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct BadgeProps {
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub tone: Option<DaisyColor>,
    #[prop_or(DaisySize::Md)]
    pub size: DaisySize,
    #[prop_or_default]
    pub outline: bool,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Badge)]
pub fn badge(props: &BadgeProps) -> Html {
    let tone = tone_class("badge", props.tone);
    let size = props.size.with_prefix("badge");
    let mut classes = classes!(
        "badge",
        size,
        props.outline.then_some("badge-outline"),
        props.class.clone()
    );
    if let Some(tone) = tone {
        classes.push(tone);
    }

    html! {
        <span class={classes}>
            { for props.children.iter() }
        </span>
    }
}
