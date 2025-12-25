use crate::components::daisy::foundations::{DaisyColor, DaisySize, tone_class};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct InputProps {
    #[prop_or_default]
    pub value: AttrValue,
    #[prop_or_default]
    pub placeholder: Option<AttrValue>,
    #[prop_or_default]
    pub input_type: Option<AttrValue>,
    #[prop_or_default]
    pub id: Option<AttrValue>,
    #[prop_or_default]
    pub name: Option<AttrValue>,
    #[prop_or_default]
    pub aria_label: Option<AttrValue>,
    #[prop_or_default]
    pub tone: Option<DaisyColor>,
    #[prop_or(DaisySize::Md)]
    pub size: DaisySize,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub input_ref: NodeRef,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub oninput: Callback<String>,
}

#[function_component(Input)]
pub fn input(props: &InputProps) -> Html {
    let tone = tone_class("input", props.tone);
    let size = props.size.with_prefix("input");
    let oninput = {
        let oninput = props.oninput.clone();
        Callback::from(move |event: InputEvent| {
            if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                oninput.emit(input.value());
            }
        })
    };
    html! {
        <input
            class={{
                let mut classes = classes!("input", size, props.class.clone());
                if let Some(tone) = tone {
                    classes.push(tone);
                }
                classes
            }}
            placeholder={props.placeholder.clone()}
            value={props.value.clone()}
            type={props.input_type.clone().unwrap_or_else(|| AttrValue::from("text"))}
            id={props.id.clone()}
            name={props.name.clone()}
            aria-label={props.aria_label.clone()}
            disabled={props.disabled}
            oninput={oninput}
            ref={props.input_ref.clone()}
        />
    }
}
