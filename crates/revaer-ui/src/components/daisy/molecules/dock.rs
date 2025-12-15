use crate::components::daisy::foundations::{BasicProps, render_container};
use yew::prelude::*;

#[function_component(Dock)]
pub fn dock(props: &BasicProps) -> Html {
    render_container("div", "dock", props)
}
