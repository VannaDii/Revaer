use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct SkeletonProps {
    #[prop_or_default]
    pub width: Option<AttrValue>,
    #[prop_or_default]
    pub height: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub rounded: bool,
}

#[function_component(Skeleton)]
pub fn skeleton(props: &SkeletonProps) -> Html {
    let mut style = String::new();
    if let Some(width) = &props.width {
        style.push_str("width:");
        style.push_str(width);
        style.push(';');
    }
    if let Some(height) = &props.height {
        style.push_str("height:");
        style.push_str(height);
        style.push(';');
    }
    let classes = classes!(
        "skeleton",
        (!props.rounded).then_some("rounded-none"),
        props.class.clone()
    );
    html! {
        <div class={classes} style={style} aria-busy="true" aria-live="polite" />
    }
}
