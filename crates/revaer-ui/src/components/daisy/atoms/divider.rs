use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct DividerProps {
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub vertical: bool,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Divider)]
pub fn divider(props: &DividerProps) -> Html {
    let classes = classes!(
        "divider",
        props.vertical.then_some("divider-vertical"),
        props.class.clone()
    );
    html! { <div class={classes}>{ for props.children.iter() }</div> }
}
