use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct TableProps {
    #[prop_or_default]
    pub headers: Vec<AttrValue>,
    #[prop_or_default]
    pub rows: Vec<Vec<AttrValue>>,
    #[prop_or_default]
    pub striped: bool,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(Table)]
pub fn table(props: &TableProps) -> Html {
    let classes = classes!(
        "table",
        props.striped.then_some("table-zebra"),
        props.class.clone()
    );
    html! {
        <div class="overflow-x-auto">
            <table class={classes}>
                {if props.headers.is_empty() {
                    html! {}
                } else {
                    html! {
                        <thead>
                            <tr>
                                {for props.headers.iter().map(|head| html! { <th>{head.clone()}</th> })}
                            </tr>
                        </thead>
                    }
                }}
                <tbody>
                    {for props.rows.iter().map(|row| html! {
                        <tr>
                            {for row.iter().map(|cell| html! { <td>{cell.clone()}</td> })}
                        </tr>
                    })}
                </tbody>
            </table>
        </div>
    }
}
