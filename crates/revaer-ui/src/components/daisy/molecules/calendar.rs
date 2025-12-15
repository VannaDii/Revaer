use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CalendarProps {
    #[prop_or_default]
    pub title: Option<AttrValue>,
    #[prop_or_default]
    pub subtitle: Option<AttrValue>,
    #[prop_or_default]
    pub content: Children,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Calendar)]
pub fn calendar(props: &CalendarProps) -> Html {
    let header = if props.title.is_some() || props.subtitle.is_some() {
        html! {
            <header class="flex items-center justify-between mb-2">
                <div>
                    {props.title.clone().map(|title| html! { <h4 class="font-semibold">{title}</h4> }).unwrap_or_default()}
                    {props.subtitle.clone().map(|subtitle| html! { <p class="text-sm opacity-70">{subtitle}</p> }).unwrap_or_default()}
                </div>
            </header>
        }
    } else {
        html! {}
    };
    let classes = classes!(
        "calendar",
        "p-4",
        "rounded-box",
        "bg-base-200",
        props.class.clone()
    );
    html! {
        <div class={classes}>
            {header}
            <div class="calendar-body">{ for props.content.iter() }</div>
        </div>
    }
}
