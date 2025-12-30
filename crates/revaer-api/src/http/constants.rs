//! Shared HTTP constants (headers, problem URIs, pagination defaults).

pub(crate) const HEADER_SETUP_TOKEN: &str = "x-revaer-setup-token";
pub(crate) const HEADER_API_KEY: &str = "x-revaer-api-key";
pub(crate) const HEADER_API_KEY_LEGACY: &str = "x-api-key";
pub(crate) const HEADER_REQUEST_ID: &str = "x-request-id";
pub(crate) const HEADER_LAST_EVENT_ID: &str = "last-event-id";
pub(crate) const HEADER_RATE_LIMIT_LIMIT: &str = "x-ratelimit-limit";
pub(crate) const HEADER_RATE_LIMIT_REMAINING: &str = "x-ratelimit-remaining";
pub(crate) const HEADER_RATE_LIMIT_RESET: &str = "x-ratelimit-reset";
pub(crate) const SSE_KEEP_ALIVE_SECS: u64 = 20;
pub(crate) const API_KEY_TTL_DAYS: i64 = 14;

pub(crate) const PROBLEM_INTERNAL: &str = "https://revaer.dev/problems/internal";
pub(crate) const PROBLEM_UNAUTHORIZED: &str = "https://revaer.dev/problems/unauthorized";
#[cfg(feature = "compat-qb")]
pub(crate) const PROBLEM_FORBIDDEN: &str = "https://revaer.dev/problems/forbidden";
pub(crate) const PROBLEM_BAD_REQUEST: &str = "https://revaer.dev/problems/bad-request";
pub(crate) const PROBLEM_CONFLICT: &str = "https://revaer.dev/problems/conflict";
pub(crate) const PROBLEM_CONFIG_INVALID: &str = "https://revaer.dev/problems/config-invalid";
pub(crate) const PROBLEM_SETUP_REQUIRED: &str = "https://revaer.dev/problems/setup-required";
pub(crate) const PROBLEM_SERVICE_UNAVAILABLE: &str =
    "https://revaer.dev/problems/service-unavailable";
pub(crate) const PROBLEM_NOT_FOUND: &str = "https://revaer.dev/problems/not-found";
pub(crate) const PROBLEM_RATE_LIMITED: &str = "https://revaer.dev/problems/rate-limited";

pub(crate) const MAX_METAINFO_BYTES: usize = 5 * 1024 * 1024;
pub(crate) const DEFAULT_PAGE_SIZE: usize = 50;
pub(crate) const MAX_PAGE_SIZE: usize = 200;
pub(crate) const EVENT_KIND_WHITELIST: &[&str] = &[
    "torrent_added",
    "files_discovered",
    "progress",
    "state_changed",
    "completed",
    "torrent_removed",
    "fsops_started",
    "fsops_progress",
    "fsops_completed",
    "fsops_failed",
    "settings_changed",
    "health_changed",
    "selection_reconciled",
];
