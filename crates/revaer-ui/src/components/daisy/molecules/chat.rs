use yew::prelude::*;

#[derive(Clone, PartialEq)]
pub struct ChatMessage {
    pub author: AttrValue,
    pub content: AttrValue,
    pub end: bool,
    pub timestamp: Option<AttrValue>,
}

#[derive(Properties, PartialEq)]
pub struct ChatProps {
    #[prop_or_default]
    pub messages: Vec<ChatMessage>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Chat)]
pub fn chat(props: &ChatProps) -> Html {
    html! {
        <div class={classes!("chat", "chat-start", props.class.clone())}>
            {for props.messages.iter().map(|message| {
                let alignment = if message.end { "chat-end" } else { "chat-start" };
                html! {
                    <div class={classes!("chat", alignment)}>
                        <div class="chat-header">
                            {message.author.clone()}
                            {message.timestamp.clone().map(|t| html! { <time class="text-xs opacity-50 ml-2">{t}</time> }).unwrap_or_default()}
                        </div>
                        <div class="chat-bubble">{message.content.clone()}</div>
                    </div>
                }
            })}
        </div>
    }
}
