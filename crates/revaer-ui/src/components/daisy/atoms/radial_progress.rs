use crate::components::daisy::foundations::{DaisyColor, tone_class};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct RadialProgressProps {
    #[prop_or(0u32)]
    pub value: u32,
    #[prop_or(100u32)]
    pub max: u32,
    #[prop_or(64u32)]
    pub size_px: u32,
    #[prop_or(6u32)]
    pub thickness_px: u32,
    #[prop_or_default]
    pub tone: Option<DaisyColor>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(RadialProgress)]
pub fn radial_progress(props: &RadialProgressProps) -> Html {
    let tone = tone_class("text", props.tone);
    let ratio = if props.max == 0 {
        0
    } else {
        props.value * 100 / props.max
    };
    let style = format!(
        "--value:{};--size:{}px;--thickness:{}px;",
        ratio, props.size_px, props.thickness_px
    );
    let mut classes = classes!("radial-progress", props.class.clone());
    if let Some(tone) = tone {
        classes.push(tone);
    }
    html! {
        <div class={classes} style={style}>
            { for props.children.iter() }
        </div>
    }
}
