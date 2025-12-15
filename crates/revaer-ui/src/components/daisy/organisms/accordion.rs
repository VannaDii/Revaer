use yew::prelude::*;

#[derive(Clone, PartialEq)]
pub struct AccordionItem {
    pub title: AttrValue,
    pub content: Html,
    pub open: bool,
}

#[derive(Properties, PartialEq)]
pub struct AccordionProps {
    #[prop_or_default]
    pub items: Vec<AccordionItem>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Accordion)]
pub fn accordion(props: &AccordionProps) -> Html {
    html! {
        <div class={classes!("join", "join-vertical", "w-full", props.class.clone())}>
            {for props.items.iter().map(|item| {
                html! {
                    <details class="collapse collapse-arrow join-item border border-base-300" open={item.open}>
                        <summary class="collapse-title text-lg font-medium">{item.title.clone()}</summary>
                        <div class="collapse-content">{item.content.clone()}</div>
                    </details>
                }
            })}
        </div>
    }
}
