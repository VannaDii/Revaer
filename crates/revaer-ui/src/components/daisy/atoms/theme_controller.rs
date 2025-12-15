use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ThemeControllerProps {
    #[prop_or_default]
    pub themes: Vec<AttrValue>,
    #[prop_or_default]
    pub value: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub onchange: Callback<AttrValue>,
}

#[function_component(ThemeController)]
pub fn theme_controller(props: &ThemeControllerProps) -> Html {
    let onchange = {
        let onchange = props.onchange.clone();
        Callback::from(move |event: Event| {
            if let Some(select) = event.target_dyn_into::<web_sys::HtmlSelectElement>() {
                onchange.emit(select.value().into());
            }
        })
    };

    html! {
        <select
            class={classes!("select", "select-bordered", "w-full", props.class.clone())}
            value={props.value.clone()}
            onchange={onchange}
            data-choose-theme=""
        >
            {for props.themes.iter().map(|theme| {
                let selected = props.value.as_ref().map(|value| value == theme).unwrap_or(false);
                html! { <option value={theme.clone()} selected={selected}>{theme.clone()}</option> }
            })}
        </select>
    }
}
