use yew::prelude::*;

#[derive(Clone, PartialEq)]
pub struct FilterOption {
    pub label: AttrValue,
    pub value: AttrValue,
    pub selected: bool,
}

#[derive(Properties, PartialEq)]
pub struct FilterProps {
    #[prop_or_default]
    pub options: Vec<FilterOption>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub on_select: Callback<AttrValue>,
}

#[function_component(Filter)]
pub fn filter(props: &FilterProps) -> Html {
    html! {
        <div class={classes!("join", props.class.clone())}>
            {for props.options.iter().map(|option| {
                let onchange = {
                    let on_select = props.on_select.clone();
                    let value = option.value.clone();
                    Callback::from(move |_| on_select.emit(value.clone()))
                };
                html! {
                    <input
                        type="radio"
                        name="filter"
                        aria-label={option.label.clone()}
                        class="btn join-item"
                        checked={option.selected}
                        onclick={onchange}
                    />
                }
            })}
        </div>
    }
}
