use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct LabelProps {
    #[prop_or_default]
    pub for_input: Option<AttrValue>,
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Label)]
pub fn label(props: &LabelProps) -> Html {
    html! {
        <label class={classes!("label", props.class.clone())} for={props.for_input.clone()}>
            { for props.children.iter() }
        </label>
    }
}
