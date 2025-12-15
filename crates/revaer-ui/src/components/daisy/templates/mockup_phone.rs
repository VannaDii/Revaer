use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct MockupPhoneProps {
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(MockupPhone)]
pub fn mockup_phone(props: &MockupPhoneProps) -> Html {
    html! {
        <div class={classes!("mockup-phone", props.class.clone())}>
            <div class="camera"></div>
            <div class="display">
                <div class="artboard artboard-demo phone-1">
                    { for props.children.iter() }
                </div>
            </div>
        </div>
    }
}
