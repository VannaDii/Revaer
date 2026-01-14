//! Server action dropdown for the top bar.
//!
//! # Design
//! - Compose actions as menu items to match the Nexus template layout.
//! - Keep behavior in callbacks; component only renders markup and emits events.
//! - Use daisyUI dropdown styles to align with shared UI primitives.

use crate::components::atoms::icons::{IconAlertTriangle, IconFileText, IconRefreshCw, IconServer};
use crate::components::daisy::Dropdown;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct ServerMenuProps {
    pub on_server_restart: Callback<()>,
    pub on_server_logs: Callback<()>,
    pub on_factory_reset: Callback<()>,
}

#[function_component(ServerMenu)]
pub(crate) fn server_menu(props: &ServerMenuProps) -> Html {
    let on_restart = props.on_server_restart.clone();
    let on_logs = props.on_server_logs.clone();
    let on_factory_reset = props.on_factory_reset.clone();

    html! {
        <Dropdown
            class={classes!("dropdown-bottom", "dropdown-end")}
            trigger_label={Some(AttrValue::from("Server menu"))}
            trigger_class={classes!("btn-ghost", "btn-circle", "btn-sm")}
            content_class={classes!("mt-2", "w-44", "p-1", "shadow", "z-[70]")}
            trigger={html! { <IconServer size={Some(AttrValue::from("4.5"))} /> }}
        >
            <li>
                <button type="button" onclick={Callback::from(move |_| on_restart.emit(()))}>
                    <IconRefreshCw size={Some(AttrValue::from("4"))} />
                    <span>{"Restart server"}</span>
                </button>
            </li>
            <li>
                <button type="button" onclick={Callback::from(move |_| on_logs.emit(()))}>
                    <IconFileText size={Some(AttrValue::from("4"))} />
                    <span>{"View logs"}</span>
                </button>
            </li>
            <li>
                <hr class="my-1 border-base-200" />
            </li>
            <li>
                <button
                    type="button"
                    class="text-error"
                    onclick={Callback::from(move |_| on_factory_reset.emit(()))}
                >
                    <IconAlertTriangle size={Some(AttrValue::from("4"))} />
                    <span>{"Factory reset"}</span>
                </button>
            </li>
        </Dropdown>
    }
}
