use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct FooterProps {
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Footer)]
pub fn footer(props: &FooterProps) -> Html {
    html! { <footer class={classes!("footer", "p-10", "bg-base-200", "text-base-content", props.class.clone())}>{ for props.children.iter() }</footer> }
}
