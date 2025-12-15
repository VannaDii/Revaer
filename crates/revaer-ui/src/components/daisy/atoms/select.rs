use crate::components::daisy::foundations::{DaisyColor, DaisySize, tone_class};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct SelectProps {
    #[prop_or_default]
    pub options: Vec<(AttrValue, AttrValue)>,
    #[prop_or_default]
    pub value: Option<AttrValue>,
    #[prop_or_default]
    pub placeholder: Option<AttrValue>,
    #[prop_or_default]
    pub tone: Option<DaisyColor>,
    #[prop_or(DaisySize::Md)]
    pub size: DaisySize,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub onchange: Callback<AttrValue>,
}

#[function_component(Select)]
pub fn select(props: &SelectProps) -> Html {
    let tone = tone_class("select", props.tone);
    let size = props.size.with_prefix("select");
    let onchange = {
        let onchange = props.onchange.clone();
        Callback::from(move |event: Event| {
            if let Some(target) = event.target_dyn_into::<web_sys::HtmlSelectElement>() {
                onchange.emit(target.value().into());
            }
        })
    };

    html! {
        <select
            class={{
                let mut classes = classes!("select", size, props.class.clone());
                if let Some(tone) = tone {
                    classes.push(tone);
                }
                classes
            }}
            value={props.value.clone()}
            disabled={props.disabled}
            onchange={onchange}
        >
            {props.placeholder.clone().map(|text| html!{
                <option selected={props.value.is_none()} disabled={true} value="">
                    {text}
                </option>
            }).unwrap_or_default()}
            {for props.options.iter().map(|(value, label)| {
                let selected = props.value.as_ref().map(|v| v == value).unwrap_or(false);
                html! { <option value={value.clone()} selected={selected}>{label.clone()}</option> }
            })}
        </select>
    }
}
