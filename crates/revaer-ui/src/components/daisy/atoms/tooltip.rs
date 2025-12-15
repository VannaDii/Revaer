use yew::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TooltipPosition {
    Top,
    Bottom,
    Left,
    Right,
}

impl TooltipPosition {
    #[must_use]
    pub const fn as_class(self) -> &'static str {
        match self {
            Self::Top => "tooltip-top",
            Self::Bottom => "tooltip-bottom",
            Self::Left => "tooltip-left",
            Self::Right => "tooltip-right",
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct TooltipProps {
    pub tip: AttrValue,
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub position: Option<TooltipPosition>,
    #[prop_or_default]
    pub open: bool,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Tooltip)]
pub fn tooltip(props: &TooltipProps) -> Html {
    let classes = classes!(
        "tooltip",
        props.position.map(|p| p.as_class()),
        props.open.then_some("tooltip-open"),
        props.class.clone()
    );
    html! {
        <div class={classes} data-tip={props.tip.clone()}>
            { for props.children.iter() }
        </div>
    }
}
