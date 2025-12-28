use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct DrawerProps {
    #[prop_or_default]
    pub open: bool,
    #[prop_or_default]
    pub on_close: Callback<()>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub content: Children,
    #[prop_or_default]
    pub side: Children,
}

#[function_component(Drawer)]
pub fn drawer(props: &DrawerProps) -> Html {
    let classes = classes!(
        "drawer",
        props.open.then_some("drawer-open"),
        props.class.clone()
    );
    let on_close = {
        let on_close = props.on_close.clone();
        Callback::from(move |event: MouseEvent| {
            event.prevent_default();
            on_close.emit(());
        })
    };
    html! {
        <div class={classes}>
            <input
                type="checkbox"
                class="drawer-toggle"
                checked={props.open}
                disabled=true
                aria-hidden="true"
            />
            <div class="drawer-content">
                { for props.content.iter() }
            </div>
            <div class="drawer-side">
                <label
                    class="drawer-overlay"
                    aria-label="drawer overlay"
                    onclick={on_close}
                ></label>
                { for props.side.iter() }
            </div>
        </div>
    }
}
