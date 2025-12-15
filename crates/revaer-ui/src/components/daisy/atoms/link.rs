use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct LinkProps {
    pub href: AttrValue,
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub external: bool,
}

#[function_component(Link)]
pub fn link(props: &LinkProps) -> Html {
    let target = props.external.then_some(AttrValue::from("_blank"));
    let rel = props
        .external
        .then_some(AttrValue::from("noreferrer noopener"));

    html! {
        <a
            class={classes!("link", props.class.clone())}
            href={props.href.clone()}
            target={target}
            rel={rel}
        >
            { for props.children.iter() }
        </a>
    }
}
