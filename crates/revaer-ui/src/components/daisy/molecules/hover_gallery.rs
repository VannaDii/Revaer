use crate::components::daisy::foundations::{BasicProps, render_container};
use yew::prelude::*;

#[function_component(HoverGallery)]
pub fn hover_gallery(props: &BasicProps) -> Html {
    render_container("div", "hover-gallery", props)
}
