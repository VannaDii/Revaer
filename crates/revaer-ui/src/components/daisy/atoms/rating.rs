use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct RatingProps {
    #[prop_or(5u8)]
    pub max: u8,
    #[prop_or(0u8)]
    pub value: u8,
    #[prop_or_default]
    pub name: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub onchange: Callback<u8>,
}

#[function_component(Rating)]
pub fn rating(props: &RatingProps) -> Html {
    let name = props
        .name
        .clone()
        .unwrap_or_else(|| AttrValue::from("rating"));

    html! {
        <div class={classes!("rating", props.class.clone())}>
            {(1..=props.max).map(|idx| {
                let onchange = {
                    let onchange = props.onchange.clone();
                    Callback::from(move |_| onchange.emit(idx))
                };
                html! {
                    <input
                        type="radio"
                        name={name.clone()}
                        class="mask mask-star-2 bg-orange-400"
                        checked={props.value == idx}
                        onclick={onchange}
                    />
                }
            }).collect::<Html>()}
        </div>
    }
}
