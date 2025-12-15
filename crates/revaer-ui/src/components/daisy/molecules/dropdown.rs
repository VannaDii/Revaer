use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct DropdownProps {
    #[prop_or_default]
    pub label: AttrValue,
    #[prop_or_default]
    pub open: bool,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub content_class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(Dropdown)]
pub fn dropdown(props: &DropdownProps) -> Html {
    let classes = classes!(
        "dropdown",
        props.open.then_some("dropdown-open"),
        props.class.clone()
    );

    html! {
        <div class={classes}>
            <div tabindex="0" role="button" class="btn m-1">{props.label.clone()}</div>
            <ul
                class={classes!(
                    "dropdown-content",
                    "menu",
                    "p-2",
                    "shadow",
                    "bg-base-200",
                    "rounded-box",
                    props.content_class.clone()
                )}
            >
                { for props.children.iter() }
            </ul>
        </div>
    }
}
