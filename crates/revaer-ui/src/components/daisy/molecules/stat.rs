use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct StatProps {
    pub title: AttrValue,
    pub value: AttrValue,
    #[prop_or_default]
    pub description: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Stat)]
pub fn stat(props: &StatProps) -> Html {
    html! {
        <div class={classes!("stat", props.class.clone())}>
            <div class="stat-title">{props.title.clone()}</div>
            <div class="stat-value">{props.value.clone()}</div>
            {props.description.clone().map(|desc| html! { <div class="stat-desc">{desc}</div> }).unwrap_or_default()}
        </div>
    }
}
