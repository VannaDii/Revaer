use crate::components::daisy::foundations::{DaisyColor, tone_class};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct AlertProps {
    #[prop_or_default]
    pub title: Option<AttrValue>,
    #[prop_or_default]
    pub description: Option<AttrValue>,
    #[prop_or_default]
    pub tone: Option<DaisyColor>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(Alert)]
pub fn alert(props: &AlertProps) -> Html {
    let tone = tone_class("alert", props.tone);
    let mut classes = classes!("alert", props.class.clone());
    if let Some(tone) = tone {
        classes.push(tone);
    }
    html! {
        <div class={classes} role="alert">
            {props.title.clone().map(|title| html! { <strong>{title}</strong> }).unwrap_or_default()}
            {props.description.clone().map(|desc| html! { <span>{desc}</span> }).unwrap_or_default()}
            { for props.children.iter() }
        </div>
    }
}
