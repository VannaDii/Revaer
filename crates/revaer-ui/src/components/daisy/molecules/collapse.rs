use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CollapseProps {
    pub title: AttrValue,
    #[prop_or_default]
    pub open: bool,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(Collapse)]
pub fn collapse(props: &CollapseProps) -> Html {
    html! {
        <details class={classes!("collapse", props.class.clone())} open={props.open}>
            <summary class="collapse-title text-lg font-medium">{props.title.clone()}</summary>
            <div class="collapse-content">{ for props.children.iter() }</div>
        </details>
    }
}
