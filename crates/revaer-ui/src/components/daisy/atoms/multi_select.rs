//! Multi-select input for DaisyUI-styled forms.
//!
//! # Design
//! - Keep selected values controlled by props.
//! - Emit the full selection vector on change.

use crate::components::daisy::foundations::{DaisyColor, DaisySize, tone_class};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct MultiSelectProps {
    #[prop_or_default]
    pub options: Vec<(AttrValue, AttrValue)>,
    #[prop_or_default]
    pub values: Vec<AttrValue>,
    #[prop_or_default]
    pub placeholder: Option<AttrValue>,
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
    pub class: Classes,
    #[prop_or_default]
    pub onchange: Callback<Vec<AttrValue>>,
}

#[function_component(MultiSelect)]
pub fn multi_select(props: &MultiSelectProps) -> Html {
    let tone = tone_class("select", props.tone);
    let size = props.size.with_prefix("select");
    let onchange = {
        let onchange = props.onchange.clone();
        Callback::from(move |event: Event| {
            if let Some(target) = event.target_dyn_into::<web_sys::HtmlSelectElement>() {
                let selected = target.selected_options();
                let mut values = Vec::with_capacity(selected.length() as usize);
                for idx in 0..selected.length() {
                    let Some(node) = selected.item(idx) else {
                        continue;
                    };
                    if let Some(value) = node.get_attribute("value") {
                        values.push(value.into());
                    }
                }
                onchange.emit(values);
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
            id={props.id.clone()}
            name={props.name.clone()}
            aria-label={props.aria_label.clone()}
            disabled={props.disabled}
            multiple={true}
            onchange={onchange}
        >
            {props.placeholder.clone().map(|text| html!{
                <option disabled={true} value="">
                    {text}
                </option>
            }).unwrap_or_default()}
            {for props.options.iter().map(|(value, label)| {
                let selected = props.values.iter().any(|v| v == value);
                html! { <option value={value.clone()} selected={selected}>{label.clone()}</option> }
            })}
        </select>
    }
}
