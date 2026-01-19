use yew::prelude::*;

#[derive(Clone, PartialEq)]
pub struct FilterOption {
    pub label: AttrValue,
    pub value: AttrValue,
    pub selected: bool,
    pub reset: bool,
}

#[derive(Properties, PartialEq)]
pub struct FilterProps {
    #[prop_or_default]
    pub options: Vec<FilterOption>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub button_class: Classes,
    #[prop_or_default]
    pub on_select: Callback<AttrValue>,
}

#[function_component(Filter)]
pub fn filter(props: &FilterProps) -> Html {
    html! {
        <div class={classes!("filter", props.class.clone())}>
            {for props.options.iter().map(|option| {
                let onchange = {
                    let on_select = props.on_select.clone();
                    let value = option.value.clone();
                    Callback::from(move |_| on_select.emit(value.clone()))
                };
                let button_class = classes!(
                    "btn",
                    if option.reset { Some("filter-reset") } else { None },
                    props.button_class.clone()
                );
                html! {
                    <input
                        type="radio"
                        name="filter"
                        aria-label={option.label.clone()}
                        class={button_class}
                        checked={option.selected}
                        value={option.value.clone()}
                        onchange={onchange}
                    />
                }
            })}
        </div>
    }
}
