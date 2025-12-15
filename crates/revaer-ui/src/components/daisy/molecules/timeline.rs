use yew::prelude::*;

#[derive(Clone, PartialEq)]
pub struct TimelineItem {
    pub title: AttrValue,
    pub detail: Option<AttrValue>,
    pub time: Option<AttrValue>,
}

#[derive(Properties, PartialEq)]
pub struct TimelineProps {
    #[prop_or_default]
    pub items: Vec<TimelineItem>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Timeline)]
pub fn timeline(props: &TimelineProps) -> Html {
    html! {
        <ul class={classes!("timeline", props.class.clone())}>
            {for props.items.iter().map(|item| {
                html! {
                    <li>
                        <div class="timeline-start">{item.time.clone().unwrap_or_default()}</div>
                        <div class="timeline-middle">
                            <span class="badge badge-primary"></span>
                        </div>
                        <div class="timeline-end">
                            <p class="font-semibold">{item.title.clone()}</p>
                            {item.detail.clone().map(|detail| html! { <p class="opacity-70 text-sm">{detail}</p> }).unwrap_or_default()}
                        </div>
                    </li>
                }
            })}
        </ul>
    }
}
