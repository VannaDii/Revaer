use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ToggleProps {
    #[prop_or_default]
    pub label: Option<AttrValue>,
    #[prop_or_default]
    pub checked: bool,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub onchange: Callback<bool>,
}

#[function_component(Toggle)]
pub fn toggle(props: &ToggleProps) -> Html {
    let onchange = {
        let onchange = props.onchange.clone();
        Callback::from(move |event: Event| {
            if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                onchange.emit(input.checked());
            }
        })
    };

    html! {
        <label class="label cursor-pointer gap-2">
            {props.label.clone().map(|text| html! { <span>{text}</span> }).unwrap_or_default()}
            <input
                type="checkbox"
                class={classes!("toggle", props.class.clone())}
                checked={props.checked}
                disabled={props.disabled}
                onchange={onchange}
            />
        </label>
    }
}
