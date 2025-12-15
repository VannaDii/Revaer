use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct NavbarProps {
    #[prop_or_default]
    pub start: Children,
    #[prop_or_default]
    pub center: Children,
    #[prop_or_default]
    pub end: Children,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Navbar)]
pub fn navbar(props: &NavbarProps) -> Html {
    html! {
        <div class={classes!("navbar", "bg-base-200", props.class.clone())}>
            <div class="navbar-start">{ for props.start.iter() }</div>
            <div class="navbar-center">{ for props.center.iter() }</div>
            <div class="navbar-end gap-2">{ for props.end.iter() }</div>
        </div>
    }
}
