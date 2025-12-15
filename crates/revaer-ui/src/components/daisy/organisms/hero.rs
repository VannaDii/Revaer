use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct HeroProps {
    pub title: AttrValue,
    #[prop_or_default]
    pub subtitle: Option<AttrValue>,
    #[prop_or_default]
    pub cta: Option<Html>,
    #[prop_or_default]
    pub image: Option<Html>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(Hero)]
pub fn hero(props: &HeroProps) -> Html {
    html! {
        <div class={classes!("hero", "min-h-[300px]", "bg-base-200", props.class.clone())}>
            <div class="hero-content flex-col lg:flex-row">
                {props.image.clone().unwrap_or_default()}
                <div>
                    <h1 class="text-4xl font-bold">{props.title.clone()}</h1>
                    {props.subtitle.clone().map(|subtitle| html! { <p class="py-6 text-lg opacity-80">{subtitle}</p> }).unwrap_or_default()}
                    {props.cta.clone().unwrap_or_default()}
                    { for props.children.iter() }
                </div>
            </div>
        </div>
    }
}
