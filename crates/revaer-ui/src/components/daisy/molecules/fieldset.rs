use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct FieldsetProps {
    #[prop_or_default]
    pub legend: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(Fieldset)]
pub fn fieldset(props: &FieldsetProps) -> Html {
    html! {
        <fieldset class={classes!("fieldset", props.class.clone())}>
            {props.legend.clone().map(|legend| html! { <legend class="fieldset-legend">{legend}</legend> }).unwrap_or_default()}
            <div class="fieldset-body">
                { for props.children.iter() }
            </div>
        </fieldset>
    }
}
