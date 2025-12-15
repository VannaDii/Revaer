use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct MockupBrowserProps {
    #[prop_or_default]
    pub url: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(MockupBrowser)]
pub fn mockup_browser(props: &MockupBrowserProps) -> Html {
    html! {
        <div class={classes!("mockup-browser", "border", "border-base-300", props.class.clone())}>
            <div class="mockup-browser-toolbar">
                <div class="input">{props.url.clone().unwrap_or_else(|| AttrValue::from("https://example.com"))}</div>
            </div>
            <div class="p-4">{ for props.children.iter() }</div>
        </div>
    }
}
