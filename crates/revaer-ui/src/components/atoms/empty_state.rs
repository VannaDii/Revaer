use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct EmptyStateProps {
    pub title: AttrValue,
    #[prop_or_default]
    pub body: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(EmptyState)]
pub(crate) fn empty_state(props: &EmptyStateProps) -> Html {
    html! {
        <div class={classes!("card", "bg-base-100", "border", "border-base-200", props.class.clone())}>
            <div class="card-body items-center gap-1 p-4 text-center">
                <p class="text-sm font-medium">{props.title.clone()}</p>
                {if let Some(body) = props.body.clone() {
                    html! { <p class="text-xs text-base-content/60">{body}</p> }
                } else {
                    html! {}
                }}
            </div>
        </div>
    }
}
