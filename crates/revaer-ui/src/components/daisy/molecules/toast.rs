use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ToastProps {
    pub message: AttrValue,
    #[prop_or_default]
    pub action: Option<Html>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Toast)]
pub fn toast(props: &ToastProps) -> Html {
    html! {
        <div class={classes!("toast", props.class.clone())}>
            <div class="alert alert-info">
                <span>{props.message.clone()}</span>
                {props.action.clone().unwrap_or_default()}
            </div>
        </div>
    }
}
