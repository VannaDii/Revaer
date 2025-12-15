use crate::components::daisy::foundations::{BasicProps, render_container};
use yew::prelude::*;

#[function_component(Stack)]
pub fn stack(props: &BasicProps) -> Html {
    render_container("div", "stack", props)
}
