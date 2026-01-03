//! Routing definitions for the Revaer UI.
use yew_router::prelude::*;

#[derive(Clone, Routable, PartialEq, Eq, Debug)]
pub(crate) enum Route {
    #[at("/")]
    Dashboard,
    #[at("/torrents")]
    Torrents,
    #[at("/torrents/:id")]
    TorrentDetail { id: String },
    #[at("/settings")]
    Settings,
    #[at("/logs")]
    Logs,
    #[at("/health")]
    Health,
    #[not_found]
    #[at("/404")]
    NotFound,
}
