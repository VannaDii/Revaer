use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct RangeProps {
    #[prop_or(0u32)]
    pub value: u32,
    #[prop_or(0u32)]
    pub min: u32,
    #[prop_or(100u32)]
    pub max: u32,
    #[prop_or(1u32)]
    pub step: u32,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub onchange: Callback<u32>,
}

#[function_component(Range)]
pub fn range(props: &RangeProps) -> Html {
    let onchange = {
        let onchange = props.onchange.clone();
        Callback::from(move |event: InputEvent| {
            if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                let parsed = input.value_as_number() as u32;
                onchange.emit(parsed);
            }
        })
    };

    html! {
        <input
            type="range"
            class={classes!("range", props.class.clone())}
            min={props.min.to_string()}
            max={props.max.to_string()}
            step={props.step.to_string()}
            value={props.value.to_string()}
            oninput={onchange}
        />
    }
}
