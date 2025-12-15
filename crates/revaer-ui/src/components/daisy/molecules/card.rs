use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CardProps {
    #[prop_or_default]
    pub title: Option<AttrValue>,
    #[prop_or_default]
    pub subtitle: Option<AttrValue>,
    #[prop_or_default]
    pub actions: Option<Html>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(Card)]
pub fn card(props: &CardProps) -> Html {
    let classes = classes!("card", "shadow", "bg-base-200", props.class.clone());
    html! {
        <div class={classes}>
            {(props.title.is_some() || props.subtitle.is_some()).then(|| {
                html! {
                    <div class="card-title px-6 pt-6">
                        <div>
                            {props.title.clone().map(|title| html! { <h3 class="text-lg font-bold">{title}</h3> }).unwrap_or_default()}
                            {props.subtitle.clone().map(|subtitle| html! { <p class="text-sm opacity-70">{subtitle}</p> }).unwrap_or_default()}
                        </div>
                    </div>
                }
            }).unwrap_or_default()}
            <div class="card-body">
                { for props.children.iter() }
            </div>
            {props.actions.clone().map(|actions| html! {
                <div class="card-actions justify-end px-6 pb-4">{actions}</div>
            }).unwrap_or_default()}
        </div>
    }
}
