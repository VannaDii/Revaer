use crate::components::daisy::foundations::{DaisyColor, tone_class};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ProgressProps {
    #[prop_or_default]
    pub value: f64,
    #[prop_or(100.0)]
    pub max: f64,
    #[prop_or_default]
    pub tone: Option<DaisyColor>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Progress)]
pub fn progress(props: &ProgressProps) -> Html {
    let tone = tone_class("progress", props.tone);
    let mut classes = classes!("progress", props.class.clone());
    if let Some(tone) = tone {
        classes.push(tone);
    }
    html! {
        <progress class={classes} value={props.value.to_string()} max={props.max.to_string()} />
    }
}
