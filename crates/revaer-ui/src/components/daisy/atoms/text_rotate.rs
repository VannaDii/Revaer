use gloo_timers::callback::Interval;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct TextRotateProps {
    #[prop_or_default]
    pub items: Vec<AttrValue>,
    #[prop_or(1500u32)]
    pub interval_ms: u32,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(TextRotate)]
pub fn text_rotate(props: &TextRotateProps) -> Html {
    let index = use_state(|| 0usize);
    {
        let index = index.clone();
        let items = props.items.clone();
        let interval_ms = props.interval_ms;
        use_effect_with_deps(
            move |_| {
                let len = items.len();
                let handle = if len == 0 {
                    None
                } else {
                    Some(Interval::new(interval_ms, move || {
                        index.set((*index + 1) % len);
                    }))
                };
                move || drop(handle)
            },
            (props.items.clone(), props.interval_ms),
        );
    }

    let current = props
        .items
        .get(*index)
        .cloned()
        .unwrap_or_else(|| AttrValue::from(""));
    html! { <span class={classes!("text-rotate", props.class.clone())}>{current}</span> }
}
