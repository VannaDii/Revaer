use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct KbdProps {
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Kbd)]
pub fn kbd(props: &KbdProps) -> Html {
    html! { <kbd class={classes!("kbd", props.class.clone())}>{ for props.children.iter() }</kbd> }
}
