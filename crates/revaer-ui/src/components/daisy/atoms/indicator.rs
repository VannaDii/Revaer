use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct IndicatorProps {
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub label: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Indicator)]
pub fn indicator(props: &IndicatorProps) -> Html {
    html! {
        <div class={classes!("indicator", props.class.clone())}>
            {props.label.clone().map(|label| {
                html! { <span class="indicator-item badge badge-secondary">{label}</span> }
            }).unwrap_or_default()}
            { for props.children.iter() }
        </div>
    }
}
