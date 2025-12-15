use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct StepsProps {
    #[prop_or_default]
    pub steps: Vec<AttrValue>,
    #[prop_or(1usize)]
    pub current: usize,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Steps)]
pub fn steps(props: &StepsProps) -> Html {
    html! {
        <ul class={classes!("steps", props.class.clone())}>
            {for props.steps.iter().enumerate().map(|(idx, label)| {
                let active = idx + 1 <= props.current;
                let item_class = classes!("step", active.then_some("step-primary"));
                html! { <li class={item_class}>{label.clone()}</li> }
            })}
        </ul>
    }
}
