use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct PaginationProps {
    #[prop_or(1usize)]
    pub current: usize,
    #[prop_or(1usize)]
    pub total: usize,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub on_change: Callback<usize>,
}

#[function_component(Pagination)]
pub fn pagination(props: &PaginationProps) -> Html {
    let current = props.current.max(1).min(props.total.max(1));
    let on_change = props.on_change.clone();
    let total = props.total;

    let go_prev = {
        let on_change = on_change.clone();
        Callback::from(move |_| {
            if current > 1 {
                on_change.emit(current - 1);
            }
        })
    };
    let go_next = Callback::from(move |_| {
        if current < total {
            on_change.emit(current + 1);
        }
    });

    html! {
        <div class={classes!("join", "pagination", props.class.clone())}>
            <button class="btn join-item" disabled={current <= 1} onclick={go_prev}>{"«"}</button>
            <button class="btn join-item">{format!("Page {current} / {}", props.total.max(1))}</button>
            <button class="btn join-item" disabled={current >= props.total} onclick={go_next}>{"»"}</button>
        </div>
    }
}
