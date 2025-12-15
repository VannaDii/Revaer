use crate::components::daisy::foundations::{BasicProps, render_container};
use yew::prelude::*;

#[function_component(Join)]
pub fn join(props: &BasicProps) -> Html {
    render_container("div", "join", props)
}
