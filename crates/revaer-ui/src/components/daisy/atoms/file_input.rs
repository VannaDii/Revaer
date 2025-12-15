use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct FileInputProps {
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub multiple: bool,
    #[prop_or_default]
    pub accept: Option<AttrValue>,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub onchange: Callback<Vec<String>>,
}

#[function_component(FileInput)]
pub fn file_input(props: &FileInputProps) -> Html {
    let onchange = {
        let onchange = props.onchange.clone();
        Callback::from(move |event: Event| {
            if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                if let Some(files) = input.files() {
                    let mut names = Vec::new();
                    for idx in 0..files.length() {
                        if let Some(file) = files.item(idx) {
                            names.push(file.name());
                        }
                    }
                    onchange.emit(names);
                }
            }
        })
    };

    html! {
        <input
            type="file"
            class={classes!("file-input", props.class.clone())}
            multiple={props.multiple}
            accept={props.accept.clone()}
            disabled={props.disabled}
            onchange={onchange}
        />
    }
}
