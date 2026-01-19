use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct DashboardShellProps {
    pub children: Children,
}

#[function_component(DashboardShell)]
pub(crate) fn dashboard_shell(props: &DashboardShellProps) -> Html {
    html! {
        {for props.children.iter()}
    }
}
