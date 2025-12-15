use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ValidatorProps {
    #[prop_or_default]
    pub valid: bool,
    #[prop_or_default]
    pub message: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Validator)]
pub fn validator(props: &ValidatorProps) -> Html {
    let tone = if props.valid {
        "text-success"
    } else {
        "text-error"
    };
    let label = if props.valid { "valid" } else { "invalid" };
    let classes = classes!("validator", tone, props.class.clone());
    html! {
        <p class={classes} role="status">
            <span class="font-semibold uppercase tracking-wide text-xs">{label}</span>
            {props.message.clone().map(|msg| html! { <span class="ml-2">{msg}</span> }).unwrap_or_default()}
        </p>
    }
}
