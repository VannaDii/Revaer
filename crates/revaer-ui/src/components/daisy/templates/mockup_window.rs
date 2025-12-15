use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct MockupWindowProps {
    #[prop_or_default]
    pub title: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(MockupWindow)]
pub fn mockup_window(props: &MockupWindowProps) -> Html {
    html! {
        <div class={classes!("mockup-window", "border", "border-base-300", props.class.clone())}>
            {props.title.clone().map(|title| html! { <div class="toolbar px-4 py-2 border-b border-base-300 font-semibold">{title}</div> }).unwrap_or_default()}
            <div class="p-4">{ for props.children.iter() }</div>
        </div>
    }
}
