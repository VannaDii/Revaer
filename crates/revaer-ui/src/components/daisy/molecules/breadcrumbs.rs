use yew::prelude::*;

#[derive(Clone, PartialEq)]
pub struct Crumb {
    pub label: AttrValue,
    pub href: Option<AttrValue>,
}

#[derive(Properties, PartialEq)]
pub struct BreadcrumbsProps {
    #[prop_or_default]
    pub items: Vec<Crumb>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Breadcrumbs)]
pub fn breadcrumbs(props: &BreadcrumbsProps) -> Html {
    html! {
        <nav class={classes!("breadcrumbs", props.class.clone())} aria-label="Breadcrumb">
            <ol>
                {for props.items.iter().map(|crumb| {
                    html! {
                        <li>
                            {crumb.href.clone().map(|href| html! { <a href={href}>{crumb.label.clone()}</a> }).unwrap_or_else(|| {
                                html! { <span aria-current="page">{crumb.label.clone()}</span> }
                            })}
                        </li>
                    }
                })}
            </ol>
        </nav>
    }
}
