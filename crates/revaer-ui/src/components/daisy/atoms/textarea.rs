use crate::components::daisy::foundations::{DaisyColor, DaisySize, tone_class};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct TextareaProps {
    #[prop_or_default]
    pub value: AttrValue,
    #[prop_or_default]
    pub placeholder: Option<AttrValue>,
    #[prop_or_default]
    pub tone: Option<DaisyColor>,
    #[prop_or(DaisySize::Md)]
    pub size: DaisySize,
    #[prop_or(4u32)]
    pub rows: u32,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub oninput: Callback<String>,
}

#[function_component(Textarea)]
pub fn textarea(props: &TextareaProps) -> Html {
    let tone = tone_class("textarea", props.tone);
    let size = props.size.with_prefix("textarea");
    let oninput = {
        let oninput = props.oninput.clone();
        Callback::from(move |event: InputEvent| {
            if let Some(input) = event.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                oninput.emit(input.value());
            }
        })
    };
    html! {
        <textarea
            class={{
                let mut classes = classes!("textarea", size, props.class.clone());
                if let Some(tone) = tone {
                    classes.push(tone);
                }
                classes
            }}
            placeholder={props.placeholder.clone()}
            value={props.value.clone()}
            rows={props.rows.to_string()}
            disabled={props.disabled}
            oninput={oninput}
        />
    }
}
