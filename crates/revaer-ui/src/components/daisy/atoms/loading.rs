use crate::components::daisy::foundations::DaisySize;
use yew::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadingKind {
    Spinner,
    Dots,
    Bars,
    Ring,
}

impl Default for LoadingKind {
    fn default() -> Self {
        Self::Spinner
    }
}

impl LoadingKind {
    #[must_use]
    pub const fn class(self) -> &'static str {
        match self {
            Self::Spinner => "loading-spinner",
            Self::Dots => "loading-dots",
            Self::Bars => "loading-bars",
            Self::Ring => "loading-ring",
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct LoadingProps {
    #[prop_or(LoadingKind::Spinner)]
    pub kind: LoadingKind,
    #[prop_or(DaisySize::Md)]
    pub size: DaisySize,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub label: Option<AttrValue>,
}

#[function_component(Loading)]
pub fn loading(props: &LoadingProps) -> Html {
    let size = props.size.with_prefix("loading");
    let classes = classes!("loading", props.kind.class(), size, props.class.clone());
    html! {
        <span class={classes} role="status" aria-label={props.label.clone()} />
    }
}
