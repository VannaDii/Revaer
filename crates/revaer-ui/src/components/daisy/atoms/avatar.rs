use crate::components::daisy::foundations::DaisySize;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct AvatarProps {
    #[prop_or_default]
    pub src: Option<AttrValue>,
    #[prop_or_default]
    pub alt: Option<AttrValue>,
    #[prop_or(DaisySize::Md)]
    pub size: DaisySize,
    #[prop_or_default]
    pub fallback: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Avatar)]
pub fn avatar(props: &AvatarProps) -> Html {
    let classes = classes!("avatar", props.class.clone());
    let size_class = size_class(props.size);
    let alt = props
        .alt
        .clone()
        .unwrap_or_else(|| AttrValue::from("avatar"));

    html! {
        <div class={classes}>
            <div class={classes!(
                size_class,
                "rounded-full",
                "ring",
                "ring-offset-base-100",
                "ring-offset-2"
            )}>
                {if let Some(src) = &props.src {
                    html! { <img src={src.clone()} alt={alt} /> }
                } else if let Some(fallback) = &props.fallback {
                    html! { <div class="placeholder">{fallback.clone()}</div> }
                } else {
                    html! { <div class="placeholder" aria-hidden="true"></div> }
                }}
            </div>
        </div>
    }
}

fn size_class(size: DaisySize) -> &'static str {
    match size {
        DaisySize::Xs => "w-8 h-8",
        DaisySize::Sm => "w-12 h-12",
        DaisySize::Md => "w-16 h-16",
        DaisySize::Lg => "w-24 h-24",
    }
}
