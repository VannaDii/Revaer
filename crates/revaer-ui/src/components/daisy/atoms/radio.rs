use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct RadioProps {
    pub name: AttrValue,
    pub value: AttrValue,
    #[prop_or_default]
    pub label: Option<AttrValue>,
    #[prop_or_default]
    pub checked: bool,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub onchange: Callback<AttrValue>,
}

#[function_component(Radio)]
pub fn radio(props: &RadioProps) -> Html {
    let onchange = {
        let onchange = props.onchange.clone();
        let value = props.value.clone();
        Callback::from(move |_| onchange.emit(value.clone()))
    };

    html! {
        <label class="label cursor-pointer gap-2">
            {props.label.clone().map(|text| html! { <span>{text}</span> }).unwrap_or_default()}
            <input
                type="radio"
                name={props.name.clone()}
                value={props.value.clone()}
                class={classes!("radio", props.class.clone())}
                checked={props.checked}
                disabled={props.disabled}
                onclick={onchange}
            />
        </label>
    }
}
