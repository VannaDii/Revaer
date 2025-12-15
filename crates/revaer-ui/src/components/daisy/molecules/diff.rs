use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct DiffProps {
    pub before: AttrValue,
    pub after: AttrValue,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Diff)]
pub fn diff(props: &DiffProps) -> Html {
    html! {
        <div class={classes!("diff", "mockup-code", props.class.clone())}>
            <pre data-prefix="-" class="text-error">{&props.before}</pre>
            <pre data-prefix="+" class="text-success">{&props.after}</pre>
        </div>
    }
}
