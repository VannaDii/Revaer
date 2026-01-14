use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct DropdownProps {
    pub trigger: Html,
    #[prop_or_default]
    pub trigger_label: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub trigger_class: Classes,
    #[prop_or_default]
    pub content_class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(Dropdown)]
pub fn dropdown(props: &DropdownProps) -> Html {
    let classes = classes!("dropdown", props.class.clone());
    let trigger_class = classes!("btn", props.trigger_class.clone());
    let content_class = classes!(
        "dropdown-content",
        "menu",
        "bg-base-100",
        "rounded-box",
        "z-1",
        "w-52",
        "p-2",
        "shadow-sm",
        props.content_class.clone()
    );

    html! {
        <div class={classes}>
            <div
                tabindex="0"
                role="button"
                class={trigger_class}
                aria-label={props.trigger_label.clone()}
            >
                {props.trigger.clone()}
            </div>
            <ul
                class={classes!(
                    content_class
                )}
                tabindex="-1"
            >
                { for props.children.iter() }
            </ul>
        </div>
    }
}
