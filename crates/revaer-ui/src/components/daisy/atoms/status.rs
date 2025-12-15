use crate::components::daisy::foundations::{DaisyColor, tone_class};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct StatusProps {
    #[prop_or_default]
    pub text: AttrValue,
    #[prop_or_default]
    pub tone: Option<DaisyColor>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Status)]
pub fn status(props: &StatusProps) -> Html {
    let tone = tone_class("badge", props.tone);
    let mut classes = classes!("badge", "badge-outline", props.class.clone());
    if let Some(tone) = tone {
        classes.push(tone);
    }
    html! { <span class={classes}>{props.text.clone()}</span> }
}
