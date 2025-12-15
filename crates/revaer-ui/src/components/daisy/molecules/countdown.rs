use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CountdownProps {
    #[prop_or(0u32)]
    pub value: u32,
    #[prop_or_default]
    pub label: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Countdown)]
pub fn countdown(props: &CountdownProps) -> Html {
    let style = format!("--value:{};", props.value);
    html! {
        <div class={classes!("countdown", props.class.clone())}>
            <span style={style}>{props.label.clone().unwrap_or_else(|| props.value.to_string().into())}</span>
        </div>
    }
}
