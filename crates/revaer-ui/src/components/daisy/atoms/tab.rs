use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct TabProps {
    #[prop_or_default]
    pub label: AttrValue,
    #[prop_or_default]
    pub active: bool,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub onclick: Callback<MouseEvent>,
}

#[function_component(Tab)]
pub fn tab(props: &TabProps) -> Html {
    let classes = classes!(
        "tab",
        props.active.then_some("tab-active"),
        props.class.clone()
    );
    html! {
        <button class={classes} disabled={props.disabled} onclick={props.onclick.clone()}>
            {props.label.clone()}
        </button>
    }
}
