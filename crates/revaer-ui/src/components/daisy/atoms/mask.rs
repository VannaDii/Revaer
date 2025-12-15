use yew::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskShape {
    Squircle,
    Heart,
    Star,
    Hexagon,
    Circle,
}

impl MaskShape {
    #[must_use]
    pub const fn as_class(self) -> &'static str {
        match self {
            Self::Squircle => "mask-squircle",
            Self::Heart => "mask-heart",
            Self::Star => "mask-star",
            Self::Hexagon => "mask-hexagon",
            Self::Circle => "mask-circle",
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct MaskProps {
    #[prop_or_default]
    pub src: Option<AttrValue>,
    #[prop_or_default]
    pub alt: Option<AttrValue>,
    #[prop_or(MaskShape::Squircle)]
    pub shape: MaskShape,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(Mask)]
pub fn mask(props: &MaskProps) -> Html {
    let classes = classes!("mask", props.shape.as_class(), props.class.clone());
    if let Some(src) = &props.src {
        html! {
            <img class={classes} src={src.clone()} alt={props.alt.clone().unwrap_or_else(|| AttrValue::from("mask"))} />
        }
    } else {
        html! {
            <div class={classes}>{ for props.children.iter() }</div>
        }
    }
}
