#[cfg(target_arch = "wasm32")]
use crate::app::Route;
use yew::prelude::*;
#[cfg(target_arch = "wasm32")]
use yew_router::prelude::Link;

#[derive(Properties, PartialEq)]
pub(crate) struct DashboardShellProps {
    pub children: Children,
}

#[function_component(DashboardShell)]
pub(crate) fn dashboard_shell(props: &DashboardShellProps) -> Html {
    html! {
        <>
            <div class="flex items-center justify-between">
                <p class="text-lg font-medium">{"Business Overview"}</p>
                <div class="breadcrumbs hidden p-0 text-sm sm:inline">
                    <ul>
                        <li>{dashboard_root_link()}</li>
                        <li>{"Dashboards"}</li>
                        <li class="opacity-80">{"Ecommerce"}</li>
                    </ul>
                </div>
            </div>
            <div class="mt-6">
                {for props.children.iter()}
            </div>
        </>
    }
}

#[cfg(target_arch = "wasm32")]
fn dashboard_root_link() -> Html {
    html! { <Link<Route> to={Route::Dashboard}>{"Nexus"}</Link<Route>> }
}

#[cfg(not(target_arch = "wasm32"))]
fn dashboard_root_link() -> Html {
    html! { <a href="#">{"Nexus"}</a> }
}
