use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct SwapProps {
    #[prop_or_default]
    pub active: bool,
    #[prop_or_default]
    pub on: Option<Html>,
    #[prop_or_default]
    pub off: Option<Html>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Swap)]
pub fn swap(props: &SwapProps) -> Html {
    let classes = classes!(
        "swap",
        props.active.then_some("swap-active"),
        props.class.clone()
    );
    html! {
        <div class={classes}>
            <div class="swap-on">{props.on.clone().unwrap_or_else(|| html! {})}</div>
            <div class="swap-off">{props.off.clone().unwrap_or_else(|| html! {})}</div>
        </div>
    }
}
