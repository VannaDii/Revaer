use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ModalProps {
    #[prop_or_default]
    pub open: bool,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub on_close: Callback<()>,
}

#[function_component(Modal)]
pub fn modal(props: &ModalProps) -> Html {
    let classes = classes!(
        "modal",
        props.open.then_some("modal-open"),
        props.class.clone()
    );

    let on_close = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| on_close.emit(()))
    };

    html! {
        <div class={classes} role="dialog" aria-modal="true">
            <div class="modal-box">
                { for props.children.iter() }
            </div>
            <button class="modal-backdrop" onclick={on_close}></button>
        </div>
    }
}
