//! Torrents feature views.

use yew::prelude::*;

use crate::features::torrents::actions::{success_message, TorrentAction};
use crate::features::torrents::state::{TorrentEntry, TorrentListState};

#[derive(Properties, PartialEq)]
pub struct TorrentListProps {
    pub state: TorrentListState,
    pub on_action: Callback<TorrentAction>,
}

#[function_component(TorrentList)]
pub fn torrent_list(props: &TorrentListProps) -> Html {
    html! {
        <div class="torrent-list">
            { for props.state.entries.iter().map(|entry| render_entry(entry, &props.on_action)) }
        </div>
    }
}

fn render_entry(entry: &TorrentEntry, on_action: &Callback<TorrentAction>) -> Html {
    let id = entry.id;
    let on_pause = {
        let on_action = on_action.clone();
        Callback::from(move |_| on_action.emit(TorrentAction::Pause(id)))
    };
    let on_resume = {
        let on_action = on_action.clone();
        Callback::from(move |_| on_action.emit(TorrentAction::Resume(id)))
    };

    html! {
        <div class="torrent-entry">
            <div class="torrent-name">{ entry.name.clone().unwrap_or_else(|| "Unnamed".to_string()) }</div>
            <div class="torrent-state">{ format!("{:?}", entry.state) }</div>
            <div class="torrent-progress">{ format!("{:.1}%", entry.progress.percent_complete.unwrap_or(0.0)) }</div>
            <button onclick={on_pause}>{"Pause"}</button>
            <button onclick={on_resume}>{"Resume"}</button>
            <div class="torrent-success">{ success_message(&TorrentAction::Resume(id)) }</div>
        </div>
    }
}
