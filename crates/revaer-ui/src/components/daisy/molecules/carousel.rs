use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CarouselProps {
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub full_width: bool,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Carousel)]
pub fn carousel(props: &CarouselProps) -> Html {
    let classes = classes!(
        "carousel",
        props.full_width.then_some("w-full"),
        props.class.clone()
    );
    html! { <div class={classes}>{ for props.children.iter() }</div> }
}
