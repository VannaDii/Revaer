use yew::prelude::*;

#[derive(Clone, PartialEq)]
pub struct MenuItem {
    pub label: AttrValue,
    pub href: Option<AttrValue>,
    pub active: bool,
}

#[derive(Properties, PartialEq)]
pub struct MenuProps {
    #[prop_or_default]
    pub items: Vec<MenuItem>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(Menu)]
pub fn menu(props: &MenuProps) -> Html {
    let classes = classes!("menu", "bg-base-200", "rounded-box", props.class.clone());
    if props.items.is_empty() {
        html! { <ul class={classes}>{ for props.children.iter() }</ul> }
    } else {
        html! {
            <ul class={classes}>
                {for props.items.iter().map(|item| {
                    let item_class = classes!(item.active.then_some("active"));
                    html! {
                        <li class={item_class}>
                            {item.href.clone().map(|href| html! { <a href={href}>{item.label.clone()}</a> }).unwrap_or_else(|| html! { <span>{item.label.clone()}</span> })}
                        </li>
                    }
                })}
            </ul>
        }
    }
}
