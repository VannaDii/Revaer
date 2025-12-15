use crate::components::daisy::foundations::{BasicProps, render_container};
use yew::prelude::*;

#[function_component(Hover3d)]
pub fn hover_3d(props: &BasicProps) -> Html {
    render_container("div", "hover-3d", props)
}
