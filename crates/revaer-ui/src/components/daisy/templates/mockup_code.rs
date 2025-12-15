use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct MockupCodeProps {
    #[prop_or_default]
    pub lines: Vec<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(MockupCode)]
pub fn mockup_code(props: &MockupCodeProps) -> Html {
    html! {
        <div class={classes!("mockup-code", props.class.clone())}>
            {for props.lines.iter().map(|line| html! { <pre data-prefix=">"><code>{line.clone()}</code></pre> })}
        </div>
    }
}
