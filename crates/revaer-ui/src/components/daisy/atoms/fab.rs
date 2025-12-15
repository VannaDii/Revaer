use crate::components::daisy::atoms::button::Button;
use crate::components::daisy::foundations::{DaisyColor, DaisySize, DaisyVariant};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct FabProps {
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub tone: Option<DaisyColor>,
    #[prop_or(DaisySize::Md)]
    pub size: DaisySize,
    #[prop_or(DaisyVariant::Solid)]
    pub variant: DaisyVariant,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub onclick: Callback<MouseEvent>,
}

#[function_component(Fab)]
pub fn fab(props: &FabProps) -> Html {
    let class = classes!("fab", "btn-circle", props.class.clone());
    html! {
        <Button
            tone={props.tone}
            size={props.size}
            variant={props.variant}
            disabled={props.disabled}
            class={class}
            circle={true}
            onclick={props.onclick.clone()}
        >
            { for props.children.iter() }
        </Button>
    }
}
