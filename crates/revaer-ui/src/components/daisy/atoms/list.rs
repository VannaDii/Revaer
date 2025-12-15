use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ListProps {
    #[prop_or_default]
    pub ordered: bool,
    #[prop_or_default]
    pub items: Vec<AttrValue>,
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(List)]
pub fn list(props: &ListProps) -> Html {
    let tag = if props.ordered { "ol" } else { "ul" };
    let mut node = yew::virtual_dom::VTag::new(tag);
    node.add_attribute("class", classes!("list", props.class.clone()).to_string());
    if props.items.is_empty() {
        for child in props.children.iter() {
            node.add_child(child);
        }
    } else {
        for item in &props.items {
            node.add_child(html! { <li>{item.clone()}</li> });
        }
    }
    node.into()
}
