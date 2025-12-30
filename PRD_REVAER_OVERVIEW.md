# Revaer PRD — Media Domains & Functionality (Priority-Ordered)

## Document Intent

This PRD defines:

-   What Revaer is and is not
-   The priority order of media domains and why
-   The minimum complete feature set per domain (MVP boundaries)
-   The cross-cutting platform responsibilities (DB, events, config, API, CLI, UI, observability)
-   The decision model: explainability, determinism, and auditable automation
-   What Codex must treat as hard requirements, not suggestions

This PRD is written to be compatible with:

-   Rust 2024 workspace discipline
-   Minimal deps
-   Stored-procedure-only DB interactions
-   Deterministic behavior and testability
-   “No dead code” / “no stubs” / “just ci passes”

---

## 1. Product Definition

### 1.1 What Revaer Is

Revaer is a local-first media acquisition and management platform with:

-   A torrent engine as the foundational acquisition substrate
-   Domain managers for distinct media types (audiobooks, movies, TV, adult, ebooks)
-   Artifact pipelines (subtitles, transcoding) that reduce ongoing management burden
-   A system-wide commitment to:
    -   deterministic automation
    -   explainable decisions
    -   normalized data
    -   observable state transitions
    -   reversible operations (where feasible)

### 1.2 What Revaer Is Not

Revaer is not:

-   A streaming player
-   A recommendation system
-   A hosted SaaS
-   A plugin marketplace
-   A magical AI-driven “guess what I want” system
-   An “Arr clone” in UX philosophy (it should be clearer and more inspectable)

---

## 2. Priority Order (Locked)

The system is built in layers where upstream domains provide durable primitives downstream domains rely on.

**Priority Order:**

1. Torrenting
2. Audiobooks
3. Movies
4. TV Shows
5. Subtitles
6. Transcoding (must persist extracted subtitles)
7. Adult Content (movies + scenes)
8. Ebooks

**Why This Order (Operational Rationale):**

-   Torrenting is the acquisition substrate
-   Audiobooks are under-served and require domain-specific logic early (multi-file sets, chaptering, narrators/editions)
-   Movies/TV are high volume and benefit from stable selection + post-processing primitives
-   Subtitles + transcoding are cross-cutting artifact pipelines; they should be driven by actual inventory and decisions from movie/TV/audiobooks
-   Adult content requires strict isolation and explicit policy boundaries
-   Ebooks are simpler and can reuse acquisition + library primitives

---

## 3. Product Goals and Success Criteria

### 3.1 Goals

-   Reduce the number of separate services required for acquisition and management.
-   Make automation deterministic and explainable.
-   Minimize manual “subtitle hunt” and “transcode chores.”
-   Treat audiobooks as first-class media, not an afterthought.
-   Provide consistent APIs and CLI behaviors across domains.

### 3.2 Success Criteria (Measurable)

-   “Why did it pick this release?” is always answerable via:
-   UI rationale panel
-   API response rationale fields
-   logs/spans with structured fields
-   Subtitle burden reduction:
    -   extracted subtitles are persisted and indexed
    -   re-acquisition due to missing subtitles approaches zero over time
-   Deterministic re-run:
    -   given the same configuration and indexer results, selection is repeatable
-   Safety and stability:
    -   failures degrade gracefully (no panics; actionable errors)
-   no hidden background actions without traceable events

---

## 4. Global Constraints (Codex Must Not Violate)

These are product requirements that mirror AGENT.md.

### 4.1 Architectural Constraints

-   Library-first: bins only wire; all logic in libs
-   Each media domain is a crate or sub-crate vertical slice (no grab-bags)
-   All runtime DB interaction via stored procedures only
-   Normalized schema; JSONB banned
-   All operations are accessible via:
    -   API
    -   CLI
    -   UI (where applicable)

### 4.2 Engineering Constraints

-   Rust 2024 only
-   No dead code, no unused items
-   Minimal dependencies; new dependency requires rationale in task record
-   All ops through just; CI runs only `just ...`
-   Observability required for externally visible operations:
    -   tracing spans
    -   metrics
    -   events

### 4.3 Behavioral Constraints

-   No “silent automation.”
-   Every automated action must produce events and rationale.
-   No irreversible destructive operations by default.
-   Destructive actions require explicit policy configuration or confirmation (UI/CLI).
-   No “guessing” semantics across domains.
-   Torrenting does not decide media identity; domain managers do.

---

## 5. Shared Mental Model: Entities, Artifacts, Decisions

Revaer must separate:

-   Acquisition objects (torrents, downloads)
-   Inventory artifacts (files, streams, subtitles)
-   Domain entities (audiobook, movie, episode, scene, ebook)
-   Decisions (selection, upgrade, re-download, transcode) with rationales

### 5.1 Core Entity Categories

**Acquisition:**

-   Torrent
-   Magnet
-   IndexerResult
-   Tracker

**Inventory / Artifacts:**

-   FileArtifact (stable byte identity: hash/fingerprint, size, container)
-   PathBinding (artifact → path with role: payload_path, library_path, working_path)
-   LinkKind (hardlink, symlink optional, physical copy)
-   StreamArtifact (video/audio/subtitle streams)
-   SubtitleArtifact (external or extracted)
-   TranscodeJob + TranscodeOutput

**Domain Entities:**

-   Audiobook (book-level)
-   AudiobookEdition (release/encoding variant)
-   AudiobookPart (tracks/chapters/files)
-   Movie
-   MovieEdition
-   Show
-   Season
-   Episode
-   AdultMovie
-   AdultScene
-   Ebook

**Decision Records:**

-   SelectionDecision (what chosen, why)
-   UpgradeDecision (why replaced)
-   RejectionDecision (why not selected)
-   RemediationDecision (e.g., subtitle fetch triggered)

### 5.2 Decision Record Minimum Fields (Normative)

Every decision record must include:

-   `decision_id`
-   `decision_kind` (selection/upgrade/reject/remediate)
-   `domain` (torrent/audiobook/movie/tv/subtitles/transcode/adult/ebook)
-   `target_id` (required when targets apply)
-   `entity identifiers` (domain entity id + relevant artifact ids)
-   `policy snapshot reference` (which ruleset produced it)
-   `candidate set reference` (what was considered)
-   `rationale fields` (structured, not interpolated strings)
-   `scored_attributes` (list of score contributors)
-   `hard_constraints_passed/failed`
-   `tie_breakers_used`
-   `final_choice_reason_codes` (enum-like)
-   `timestamps`
-   `correlation_id / request_id linkage`

---

## 6. Storage Strategy & Seeding Integrity

### 6.1 Goal

Revaer must support library-convergent naming (Plex-friendly) without breaking seeding, as long as the payload bytes are unchanged. This is a core differentiator: perfect library naming with continued seeding and minimal duplicate storage.

### 6.2 Definitions (Normative)

-   Payload file: the actual bytes the torrent client is seeding (participates in piece verification).
-   Library file: the file path Plex consumes (naming/path conforms to library rules).
-   Content-mutating transform: any operation that changes bytes (transcode, remux rewrite, audio normalization rewrite).
-   Non-mutating rename/move: operations that change path/name without changing bytes.

### 6.3 Hard Requirements

-   If content has not been mutated, Revaer must allow library-visible naming to converge with Plex conventions while preserving seeding.
-   Revaer must avoid duplicate storage wherever possible.
-   Revaer must never pretend seeding is intact when the payload bytes have changed.

### 6.4 Allowed Import Strategies (Policy-Driven)

1. Hardlink import + rename library path
    -   Payload remains at the torrent client path.
    -   Library file is a hardlink with Plex-friendly naming.
    -   Renaming applies to the library link, not the payload.
    -   Seeding continues because the payload remains intact and referenced by the client.

2. Atomic move payload into canonical seed+library structure (same filesystem only)
    -   Payload lives in a canonical structured directory serving both seeding and library conventions.
    -   Allowed only when:
        -   same filesystem (atomic rename possible)
        -   torrent client path update is supported and confirmed

3. Copy import (fallback)
    -   Used when hardlinks are not possible (cross-filesystem, permissions, unsupported filesystem).
    -   Used when policy explicitly prefers isolation.

### 6.5 Renames and Moves: Required Support

Revaer must support renaming library-visible artifacts to converge with Plex conventions while maintaining seeding:

-   file rename (title/year/edition formatting)
-   directory rename (season folder normalization, show folder normalization)
-   moving within library root as long as it remains link-safe
-   retitling editions without byte changes

### 6.6 Renames and Moves: Must Not Happen Silently

-   If the underlying bytes change, the original torrent payload can no longer seed reliably.
-   Revaer must not imply continued seeding in that case.
-   When content-mutating transforms happen:
    -   the library file becomes a new artifact lineage
    -   seeding can continue only if:
        -   the original payload is retained and remains available to the torrent client, or
        -   a verified “reseed from new bytes” workflow exists (future, optional)

### 6.7 Required Internal Model (Storage Graph)

Revaer must model storage as artifacts and bindings, not “a path”:

-   FileArtifact: stable identity for a specific byte payload (hash/fingerprint).
-   PathBinding: maps FileArtifact → filesystem path(s), with role:
    -   payload_path (torrent client)
    -   library_path (Plex)
    -   working_path (transcode staging)
-   LinkKind:
    -   hardlink
    -   symlink (optional, policy-driven)
    -   physical copy

Renames operate on PathBinding, not on FileArtifact.

### 6.8 Required Invariants (Auditable)

-   A seeding torrent must have at least one live payload_path binding that is byte-identical to the torrent’s expected content.
-   A library item consumed by Plex must have a library_path binding.
-   If library_path is a hardlink to payload_path, Revaer must record:
    -   inode identity (or platform equivalent)
    -   filesystem id
    -   link count snapshot (optional but useful)

### 6.9 Diagnostics and Explainability (Must Exist)

Revaer must provide an “Import/Link Explain” output that answers:

-   Did we hardlink, atomic move, copy, or fail?
-   If we did not hardlink, why not?
    -   cross-filesystem
    -   insufficient permissions
    -   unsupported filesystem
    -   path policy conflict
-   If seeding would break due to a planned action, say so before doing it.

CLI + API must expose:

-   explain import / explain links for an item
-   dry-run mode for rename plans

### 6.10 Policy Knobs (Minimum)

-   preferred import mode: hardlink | atomic | copy
-   allow symlinks: true/false (default false unless policy opts in)
-   keep original payload after transcode: true/false (default true if seeding matters)
-   minimum free space safety threshold
-   rename strategy:
    -   conservative (only library link)
    -   aggressive (move payload and update client paths)

### 6.11 Edge Cases (Deterministic Handling Required)

-   Multi-file torrents with partial selection:
    -   link/copy only selected files
-   Scene releases / multi-episode bundles:
    -   map bindings per episode without touching payload bytes
-   Subtitle sidecars:
    -   may be renamed/moved with library path
    -   do not affect seeding unless they are part of the torrent payload

---

## 7. Targets (Multi-instance Replacement)

### 7.1 Goal

Revaer must support multiple library targets per domain entity so users never need separate servers or duplicate instances.

### 7.2 Core Concept: Target

A Target is a policy envelope + storage envelope + quality envelope attached to a domain entity.

Example targets:

-   movies-hd
-   movies-uhd
-   tv-hd
-   tv-uhd
-   audiobooks-mobile
-   audiobooks-archival

### 7.3 Requirements (Normative)

-   A single movie/show/audiobook may be tracked by multiple targets concurrently.
-   Each target has:
    -   quality profile (and custom format scoring set)
    -   language/subtitle profile
    -   storage root
    -   seeding/import mode preferences
    -   upgrade/cutoff rules
-   Each target produces its own preferred edition and maintains its own lifecycle state.

### 7.4 Decision Model Implications

Selection is no longer “best overall.” It is “best for target X.”

-   decision records must be target-scoped:
    -   `decision.target_id` is mandatory where targets apply

### 7.5 UI Implications

Users must be able to:

-   see which targets an item is assigned to
-   see the preferred edition per target
-   see per-target missing/cutoff status
-   request/upgrade per target

---

## 8. Request Dashboard (Overseerr-class UX)

### 8.1 Goal

The default dashboard is a Request + Discovery hub and a live operational view of:

-   what people want
-   what Revaer is doing about it
-   what’s blocked and why

### 8.2 Required Dashboard Modules (Minimum)

1. Unified Search
    -   One search box, results grouped by domain:
        -   Movies
        -   TV
        -   Audiobooks
        -   Adult
        -   Ebooks
    -   Results show:
        -   “already have it” status (per target)
        -   “requested” status
        -   “missing” status
        -   confidence / ambiguity flags

2. Request Composer
    -   Request button from any result
    -   Request includes:
        -   target selection (multi-target is first-class)
        -   constraints (language, edition kind, narrator, etc.)
        -   priority (normal/high)
        -   optional “approval required” flag (future-ready)

3. Request Queue
    -   Lifecycle:
        -   requested → searching → candidate found → downloading → importing → validating → complete
    -   Failure modes must include explicit reasons

4. Activity Feed
    -   Human-readable but structured activity:
        -   “Picked release X because …”
        -   “Rejected Y due to constraint …”
        -   “Fetched subtitles …”
        -   “Transcode queued …”
    -   Each item links to the underlying decision record

5. Operations Snapshot
    -   Active downloads, seeding health, stalled items
    -   Subtitle backlog
    -   Transcode queue
    -   Storage warnings
    -   Indexer health warnings

### 8.3 “Why” Must Be Visible Here

Revaer’s request dashboard must show:

-   last decision per request
-   top 3 rejected candidates
-   the exact constraint or score differences that mattered

This is required for user trust and is a core differentiator.

---

## 9. Platform Capabilities (Shared Across All Domains)

These are system-level services every domain uses.

### 9.1 Configuration System

-   Typed config, validated at load
-   Load order: defaults → file → env → CLI
-   Effective config log emitted with secrets redacted
-   Policy configuration is versionable and auditable

Config must include, at minimum:

-   Indexer and tracker endpoints (if applicable)
-   Bandwidth limits and schedules
-   Storage roots and path strategies
-   Naming rules (domain-specific)
-   Language preferences (subtitles; audiobook metadata)
-   Transcoding policies
-   Adult content isolation settings
-   Retention and cleanup rules

### 9.2 Events System

Events are required for:

-   UI live updates (SSE)
-   domain decoupling
-   auditability

**Event Requirements:**

-   event kinds are enums (no dynamic strings)
-   payloads are schema-stable
-   cardinality is controlled (avoid unbounded high-card fields)
-   correlation IDs flow through

**Minimum Event Families:**

-   `engine.*` (torrent lifecycle)
-   `indexer.*` (search results, failures)
-   `media.*` (domain entity changes)
-   `artifacts.*` (subtitle extracted, transcode produced)
-   `decisions.*` (decision made, decision applied)
-   `jobs.*` (background tasks started/completed/failed)

### 9.3 API System (Axum)

-   Versioned under `/v1`
-   OpenAPI export is deterministic and committed
-   Responses include rationale references where relevant
-   API surfaces are domain-scoped:
    -   `/v1/torrents/...`
    -   `/v1/audiobooks/...`
    -   `/v1/movies/...`
    -   `/v1/tv/...`
    -   `/v1/subtitles/...`
    -   `/v1/transcode/...`
    -   `/v1/adult/...`
    -   `/v1/ebooks/...`
    -   `/v1/system/...` (health, metrics pointers, config read-only)

### 9.4 CLI System

-   Every domain must have CLI parity for core operations
-   Output modes: `json|table` (json stable)
-   Destructive operations require:
    -   explicit `--yes` or
    -   `--force` + policy allowing it

### 9.5 Storage / Filesystem Operations

-   All FS ops flow through `revaer-fsops`
-   Async-safe; `spawn_blocking` only where necessary
-   Must support:
    -   atomic moves (or best-effort with rollback)
    -   hardlink creation and verification (same filesystem, inode identity capture)
    -   disk space checks
    -   path conflict resolution strategies
    -   consistent hashing / fingerprinting

### 9.6 Observability

Every externally-triggered operation must include:

-   tracing span at boundary
-   structured fields (no interpolated messages)
-   metrics for:
    -   counts
    -   durations
    -   queues
    -   failures

---

## 10. Domain PRDs (Detailed)

### 10.1 Torrenting Domain (Foundation)

#### 10.1.1 Scope

**Torrenting owns:**

-   acquisition lifecycle
-   file selection inside torrents
-   session state and resource controls
-   tracker behavior and health
-   deterministic scheduling policies

**Torrenting does not own:**

-   interpreting media semantics beyond extracted metadata signals
-   naming conventions per media domain
-   subtitle logic (except identifying embedded streams as artifacts)

#### 10.1.2 Primary User Workflows

-   Add torrent by:
-   magnet
-   .torrent file
-   indexer selection (“send to revaer”)
-   Monitor lifecycle:
    -   queued → downloading → verifying → seeding → paused/stopped → removed
-   Inspect selection:
    -   which files included/excluded and why
-   Control seeding:
    -   ratio/time policy
    -   manual override
-   Handle failures:
    -   tracker unreachable
    -   stalled
    -   disk full
    -   hash fail

#### 10.1.3 Required Features (MVP)

**Acquisition:**

-   add torrent with metadata capture
-   list torrents with state summary
-   pause/resume/remove
-   set per-torrent tags/labels (used by downstream routing)

**Selection:**

-   user-customizable glob filters (include/exclude)
-   sensible defaults:
    -   include: common archives and media extensions
    -   exclude: samples, nfo if unwanted, junk patterns
-   selection rationale recorded:
    -   matched include rule
    -   matched exclude rule
    -   size-based or priority-based pruning if configured

**State Machine:**

-   explicit states with transitions logged and evented
-   determinism:
    -   transitions must not depend on non-deterministic ordering without stable sort keys

**Resource Governance:**

-   global bandwidth caps
-   scheduled bandwidth windows (time-based)
-   max active torrents
-   max active downloads
-   queue scheduling strategy:
    -   stable priority ordering
    -   fairness rules (configurable)

**Tracker Handling:**

-   tracker list management
-   tracker health tracking
-   backoff and retries (idempotent ops only)

#### 10.1.4 Non-Trivial Requirements

-   File manifest production:
-   once metadata is known, produce a normalized manifest:
    -   file paths within torrent
    -   sizes
    -   priority
    -   selected/not selected
    -   hashes if available
-   “Explain” endpoint:
-   for a given torrent, return:
-   candidate files
-   applied filters
-   selected set
-   rationale codes
-   Safety:
    -   removing a torrent must have policy options:
        -   remove torrent only
        -   remove torrent + data
-   default should be least destructive

#### 10.1.5 Events (Minimum)

-   `engine.torrent_added`
-   `engine.torrent_state_changed`
-   `engine.torrent_removed`
-   `engine.file_selection_changed`
-   `engine.bandwidth_policy_applied`
-   `engine.tracker_health_updated`
-   `engine.error_occurred`

#### 10.1.6 API (Minimum)

-   `POST /v1/torrents` (magnet or torrent file reference)
-   `GET /v1/torrents`
-   `GET /v1/torrents/{id}`
-   `POST /v1/torrents/{id}/pause`
-   `POST /v1/torrents/{id}/resume`
-   `POST /v1/torrents/{id}/remove` (policy flags)
-   `GET /v1/torrents/{id}/explain`

#### 10.1.7 CLI (Minimum)

-   `revaer torrents add --magnet ...`
-   `revaer torrents list`
-   `revaer torrents show`
-   `revaer torrents pause/resume`
-   `revaer torrents remove [--delete-data] [--yes]`
-   `revaer torrents explain`

---

### 10.2 Audiobooks Domain (Tier-1 Priority)

#### 10.2.1 Scope

Audiobooks owns:

-   audiobook identity resolution
-   edition handling
-   multi-file set validation
-   chapter handling (where possible)
-   narrator and series mapping
-   metadata enrichment (from local/external sources if configured)

Audiobooks does not own:

-   playback
-   DRM cracking
-   “recommendations”

#### 10.2.2 Core challenges (this is why it’s special)

Audiobooks are often:

-   multi-file (many mp3s)
-   inconsistent metadata
-   ambiguous editions (narrator/version)
-   mixed quality sources
-   “book + extras + cover art” bundles

Revaer must model this explicitly rather than forcing audiobook into movie-like assumptions.

#### 10.2.3 Required Entity Model

**Audiobook:**

-   book identity (title, author(s), series, series_index)
-   language
-   publication year (if known)
-   canonical identifiers where possible (ISBN exists sometimes, but not guaranteed)

AudiobookEdition:

-   narrator(s)
-   runtime duration (estimated/declared)
-   format (m4b/mp3/flac)
-   bitrate/codec profile
-   source grouping (release group, provider hint)
-   “chapterized” boolean capability
-   cover art artifact (optional)

AudiobookPart:

-   file artifacts and ordering
-   chapter metadata links (if extracted)

#### 10.2.4 Required Workflows

**Discovery:**

-   search by:
    -   title
    -   author
    -   series
    -   narrator (optional)
-   show candidate sets with scoring + rationale

Acquisition:

-   select an edition candidate
-   route to torrenting with a routing tag (audiobooks)
-   track the download until complete
-   validate completeness

Validation:

-   ensure required audio artifacts exist
-   enforce minimum duration thresholds (if available)
-   detect “sample/trailer only”
-   detect missing segments based on file counts/duration gaps (heuristic-driven, deterministic)

Post-processing (policy-driven):

-   if mp3 multi-file and policy prefers m4b:
-   offer a “consolidate to m4b” transcode job
-   ensure cover art present if available
-   persist chapter metadata if extracted

Library management:

-   mark preferred edition
-   handle upgrades:
    -   better bitrate
    -   correct narrator
-   chapterized version
-   preserve history:
-   don’t delete old edition artifacts unless policy allows

#### 10.2.5 Scoring and Selection Requirements

Selection must be explainable and deterministic.

Example scoring contributors (must be structured, not free text):

-   preferred narrator match
-   preferred format match (m4b > mp3 if policy)
-   chapterized presence
-   bitrate thresholds
-   completeness confidence
-   release group trust tier
-   language match

Hard constraints examples:

-   language must match unless user overrides
-   runtime must exceed minimum threshold for a given book (if known)
-   exclude “abridged” unless requested

#### 10.2.6 Events (Minimum)

-   `media.audiobook_created`
-   `media.audiobook_edition_discovered`
-   `media.audiobook_acquisition_requested`
-   `media.audiobook_download_completed`
-   `media.audiobook_validation_failed`
-   `media.audiobook_edition_promoted` (new preferred)
-   `decisions.audiobook_selection_made`

#### 10.2.7 API (Minimum)

-   `GET /v1/audiobooks/search?q=...`
-   `POST /v1/audiobooks/{id}/request` (edition constraints)
-   `GET /v1/audiobooks`
-   `GET /v1/audiobooks/{id}`
-   `GET /v1/audiobooks/{id}/editions`
-   `POST /v1/audiobooks/{id}/editions/{edition_id}/promote`
-   `GET /v1/audiobooks/{id}/decisions`

#### 10.2.8 CLI (Minimum)

-   revaer audiobooks search “…”
-   revaer audiobooks request [constraints flags]
-   revaer audiobooks show
-   revaer audiobooks editions
-   revaer audiobooks promote <edition_id>

---

### 10.3 Movies Domain

#### 10.3.1 Scope

**Movies owns:**

-   movie identity resolution
-   edition variants (theatrical, extended, remaster)
-   quality profiles
-   upgrade policies

**Movies does not own:**

-   subtitle acquisition logic (subtitles domain)
-   transcoding logic (transcoding domain)
-   torrent file selection beyond routing tags

#### 10.3.2 Required Workflows

**Discovery:**

-   search by:
    -   title
    -   year
    -   optional imdb/tmdb id (if configured)
-   list candidates with scoring and rationale

**Acquisition:**

-   choose candidate or allow auto-pick based on policy
-   route to torrenting with routing tags (movies)
-   monitor until complete

Validation:

-   ensure primary video artifact exists
-   ensure minimum resolution/codec constraints met
-   detect “sample” or “cam” (policy-driven)
-   detect missing audio streams if required

Post-acquisition:

-   trigger subtitle pipeline based on policy:
-   require at least one subtitle in preferred language
-   or accept embedded
-   trigger transcoding based on policy:
-   container normalization
-   audio track selection
-   subtitle extraction and persistence

Upgrades:

-   define what qualifies as “better”
-   resolution tier
-   codec preference
-   HDR preference
-   audio preference
-   release group trust
-   upgrade decision must:
-   not be arbitrary
-   preserve ability to roll back if policy says “keep previous”

#### 10.3.3 Movie Identity and Edition Modeling

**Movie:**

-   title
-   year
-   canonical id (tmdb/imdb) if available
-   language/original_language (if known)

**MovieEdition:**

-   edition_kind (theatrical/extended/directors_cut/remaster/unknown)
-   resolution
-   codec
-   hdr flags
-   audio profile (channels, codec)
-   source group/provider hints
-   file artifacts

#### 10.3.4 Events (Minimum)

-   `media.movie_created`
-   `media.movie_candidate_discovered`
-   `media.movie_acquisition_requested`
-   `media.movie_download_completed`
-   `media.movie_validation_failed`
-   `media.movie_edition_promoted`
-   `decisions.movie_selection_made`

#### 10.3.5 API (Minimum)

-   `GET /v1/movies/search?q=...`
-   `POST /v1/movies/{id}/request`
-   `GET /v1/movies`
-   `GET /v1/movies/{id}`
-   `GET /v1/movies/{id}/editions`
-   `POST /v1/movies/{id}/editions/{edition_id}/promote`
-   `GET /v1/movies/{id}/decisions`

---

### 10.4 TV Shows Domain

#### 10.4.1 Scope

**TV owns:**

-   show identity resolution
-   season/episode tracking
-   episode-level acquisition and upgrades
-   multi-episode bundles

**TV does not own:**

-   subtitle and transcoding logic (separate domains)
-   torrent engine policies (except routing)

#### 10.4.2 Required Workflows

Discovery:

-   search shows by name
-   choose show entry and begin tracking

Tracking:

-   track seasons and episodes
-   allow two modes:
-   “air-date/standard”
-   “absolute” (anime-style) if configured per show

Acquisition:

-   request:
-   single episode
-   full season
-   missing episodes (bulk)
-   route to torrenting (tv tag)
-   map downloaded files to episodes deterministically

Validation:

-   detect mismatches:
-   wrong season
-   wrong episode numbering
-   multi-episode file that needs splitting semantics (logical mapping, not physical split)
-   ensure completeness per requested set

Upgrades:

-   similar to movies, but episode-level:
-   repack/proper preference
-   resolution/codec
-   audio preferences

#### 10.4.3 TV Entity Modeling

**Show:**

-   title
-   canonical id
-   mode config (air-date/absolute)
-   language preferences

**Season:**

-   season_number
-   tracked boolean

**Episode:**

-   episode_number
-   title if known
-   air_date if known
-   status:
    -   missing
    -   requested
    -   acquired
    -   validated
    -   failed

**EpisodeEdition:**

-   quality attributes
-   file artifacts
-   subtitle associations

#### 10.4.4 Events (Minimum)

-   `media.show_tracked`
-   `media.episode_requested`
-   `media.episode_acquired`
-   `media.episode_validation_failed`
-   `media.episode_edition_promoted`
-   `decisions.episode_selection_made`

#### 10.4.5 API (Minimum)

-   `GET /v1/tv/search?q=...`
-   `POST /v1/tv/shows/{id}/track`
-   `GET /v1/tv/shows`
-   `GET /v1/tv/shows/{id}`
-   `POST /v1/tv/shows/{id}/request` (missing|season|episode)
-   `GET /v1/tv/shows/{id}/episodes`
-   `GET /v1/tv/shows/{id}/decisions`

---

### 10.5 Subtitles Domain (Artifact Pipeline)

You asked for high detail here, so this section is intentionally strict.

#### 10.5.1 Scope

Subtitles owns:

-   acquisition of external subtitles
-   extraction of embedded subtitles (as artifacts)
-   normalization, storage, indexing, association to domain entities

Subtitles does not own:

-   transcoding decisions (it can request work, but doesn’t run transcoding)
-   media selection decisions (it responds to media inventory and policies)

#### 10.5.2 Subtitle Artifact Types

**SubtitleArtifact:**

-   `source_kind`:
    -   embedded (extracted from media container)
    -   external_download (e.g., opensubtitles-like)
    -   sidecar (already present next to file)
-   language (BCP-47 if possible)
-   hearing_impaired flag if known
-   format (srt/ass/vtt/pgs)
-   confidence (if derived)
-   checksum/fingerprint
-   storage path

#### 10.5.3 Hard Requirement: Persistence and Indexing

**Normative:**

-   Any subtitle discovered or extracted must be persisted
-   Persistence includes:
    -   stable storage path under Revaer-managed roots
    -   DB record linking:
        -   subtitle ↔ file artifact
        -   subtitle ↔ domain entity edition (movie edition / episode edition)
    -   searchable index on:
        -   language
        -   media id
        -   edition id
        -   source_kind
    -   DB record linking:
        -   subtitle ↔ file artifact
        -   subtitle ↔ domain entity edition (movie edition / episode edition)
    -   searchable index on:
        -   language
        -   media id
        -   edition id
        -   source_kind

#### 10.5.4 Required Workflows

Discovery:

-   inspect existing inventory for:
-   embedded subtitle streams
-   sidecar subtitles
-   register them as SubtitleArtifacts

Acquisition:

-   if missing subtitles per policy:
-   query configured providers (indexer-like abstraction)
-   rank results deterministically
-   download and validate basic integrity

Normalization:

-   optional conversions (policy-driven):
-   convert to srt/vtt where feasible
-   keep originals too, unless policy says otherwise
-   remove obvious broken artifacts deterministically (with reason codes)

Association:

-   link subtitles to the correct domain edition
-   handle ambiguous mapping:
-   store as “unbound subtitle” with candidate links and confidence
-   UI shows “needs binding” workflow

#### 10.5.5 UI Expectations

-   Media detail pages must display:
-   available subtitles by language and source
-   “preferred subtitle” selection
-   “fetch missing subtitles” action
-   “bind subtitle to edition” action if ambiguous

#### 10.5.6 Events (Minimum)

-   `artifacts.subtitle_discovered`
-   `artifacts.subtitle_extracted`
-   `artifacts.subtitle_downloaded`
-   `artifacts.subtitle_bound`
-   `artifacts.subtitle_normalized`
-   `artifacts.subtitle_failed`
-   `decisions.subtitle_selection_made`

#### 10.5.7 API (Minimum)

-   `GET /v1/subtitles/by-media/{media_kind}/{media_id}`
-   `POST /v1/subtitles/fetch` (media_kind, media_id, language prefs)
-   `POST /v1/subtitles/{id}/bind` (edition id)
-   `POST /v1/subtitles/{id}/promote` (preferred)
-   `GET /v1/subtitles/{id}`

---

### 10.6 Transcoding Domain (Artifact Pipeline)

#### 10.6.1 Scope

Transcoding owns:

-   job orchestration (queueing, scheduling, tracking)
-   artifact transformation policies
-   output artifact registration
-   subtitle extraction persistence requirement

Transcoding does not own:

-   deciding what media to acquire
-   subtitle provider acquisition (that’s subtitles domain)

#### 10.6.2 Critical Requirement: Extracted Subtitles Are Persisted

**Normative:**

-   If a transcode job inspects a file and finds embedded subtitles, those subtitles must:
    -   be extracted (if policy allows)
    -   be persisted and indexed
    -   be associated to the domain entity edition
-   This is required because it reduces future subtitle acquisition burden

#### 10.6.3 Job Model

**TranscodeJob:**

-   input file artifact id
-   job kind:
    -   container_normalize
    -   audio_normalize
    -   consolidate (audiobooks)
    -   subtitle_extract_only
-   policy snapshot reference
-   state:
    -   queued
    -   running
    -   succeeded
    -   failed
    -   cancelled
-   progress (bounded and stable, not spammy)
-   output artifact ids
-   logs pointer (structured events, not raw text blobs in DB)

#### 10.6.4 Required Workflows

Triggering:

-   manual trigger (UI/CLI/API)
-   automatic trigger (policy-driven) after validation:
-   “if file is mkv and prefer mp4”
-   “if audiobook is multi-mp3 and prefer m4b”
-   “if audio lacks preferred track layout”

Execution:

-   resource caps
-   queue fairness
-   cancellation support

Artifact registration:

-   output file registered as new FileArtifact
-   output replaces or sits alongside input based on policy

Rollback:

-   if transcode output fails validation, preserve input
-   avoid destructive delete unless policy allows

#### 10.6.5 Events (Minimum)

-   `jobs.transcode_queued`
-   `jobs.transcode_started`
-   `jobs.transcode_progress` (rate-limited)
-   `jobs.transcode_succeeded`
-   `jobs.transcode_failed`
-   `artifacts.subtitle_extracted` (as part of transcode flow)

#### 10.6.6 API (Minimum)

-   `POST /v1/transcode/jobs` (input, kind, policy override optional)
-   `GET /v1/transcode/jobs`
-   `GET /v1/transcode/jobs/{id}`
-   `POST /v1/transcode/jobs/{id}/cancel`

---

### 10.7 Adult Content Domain (Explicit Isolation)

#### 10.7.1 Scope

**Adult content owns:**

-   explicit media management as a first-class domain
-   separate identity model for:
    -   adult movies
    -   scenes (clip-level)
-   strict isolation boundaries and defaults

**Adult content does not own:**

-   unique acquisition engine (still torrenting)
-   transcoding implementation (uses transcode domain)
-   subtitle provider logic (uses subtitle domain when desired)

#### 10.7.2 Isolation Requirements (Normative)

-   Default storage roots must be separate from non-adult media
-   Default UI navigation must be separate and clearly labeled
-   Default policies must avoid accidental cross-domain mixing:
    -   adult tags do not route into movie/tv libraries
-   Search and list endpoints must be domain-scoped:
    -   adult results never appear in non-adult endpoints unless explicitly requested

#### 10.7.3 Scene-Level Model Requirements

**AdultScene:**

-   performer metadata (if available)
-   studio label (if available)
-   tags (normalized; no JSONB)
-   duration (if known)
-   source release group metadata

**AdultMovie:**

-   title
-   year (if known)
-   performers (normalized relation)
-   editions

#### 10.7.4 Required Workflows

-   Search and request adult movies
-   Search and request scenes
-   Validate and organize into isolated library
-   Optional subtitle handling (less common, but supported)
-   Optional transcoding

#### 10.7.5 Events (Minimum)

-   `media.adult_movie_created`
-   `media.adult_scene_created`
-   `media.adult_acquisition_requested`
-   `media.adult_download_completed`
-   `decisions.adult_selection_made`

#### 10.7.6 API (Minimum)

-   `GET /v1/adult/search?q=...`
-   `POST /v1/adult/movies/{id}/request`
-   `POST /v1/adult/scenes/{id}/request`
-   `GET /v1/adult/movies`
-   `GET /v1/adult/scenes`
-   `GET /v1/adult/{kind}/{id}`

---

### 10.8 Ebooks Domain (Lower Priority, Minimal Initial Scope)

#### 10.8.1 Scope

**Ebooks owns:**

-   acquiring ebook files
-   basic metadata validation and organization
-   series grouping where possible

**Ebooks does not own:**

-   reading UX
-   annotation sync
-   OCR

#### 10.8.2 Required Workflows

-   Search by title/author
-   Request acquisition
-   Validate integrity:
    -   file opens (basic validation)
    -   expected formats (epub/pdf)
-   Organize into library root
-   Maintain edition variants (optional, later)

#### 10.8.3 Events (Minimum)

-   `media.ebook_created`
-   `media.ebook_acquisition_requested`
-   `media.ebook_download_completed`
-   `media.ebook_validation_failed`
-   `decisions.ebook_selection_made`

#### 10.8.4 API (Minimum)

-   `GET /v1/ebooks/search?q=...`
-   `POST /v1/ebooks/{id}/request`
-   `GET /v1/ebooks`
-   `GET /v1/ebooks/{id}`

---

## 11. Routing Rules: How Torrents Map to Domains

This is the glue that prevents a “giant bucket of downloads.”

### 11.1 Routing Inputs

Routing must be driven by:

-   explicit request context (user requested movie vs audiobook)
-   tags/labels attached at request time
-   indexer category signals (if present)
-   deterministic heuristics (only as a fallback, and must be auditable)

### 11.2 Routing Outputs

Routing produces:

-   target domain
-   target entity (if known)
-   storage root selection
-   naming policy selection
-   post-processing pipeline triggers (subtitles/transcode)

### 11.3 Hard Rule

No “automatic domain reassignment” without an explicit decision record and an event.

---

## 12. Data and Schema Requirements (Stored Procedure First)

This PRD doesn’t define full SQL, but it defines what the schema must support.

### 12.1 Normalization Requirements

-   Avoid conglomerate blobs
-   Use join tables for:
    -   entity ↔ edition
    -   edition ↔ file artifacts
    -   file ↔ subtitle artifacts
    -   domain entity ↔ decision records
    -   domain entity ↔ acquisition requests

### 12.2 Stored Procedure Surface Expectations

For each domain:

-   create/update entities
-   record discoveries (candidates)
-   record acquisitions
-   record validations
-   record decisions
-   link artifacts
-   query views used by API/UI

Every “meaningful action” should map to a stored procedure call that can be tested.

---

## 13. Background Jobs and Schedulers

Revaer will have background jobs; they must be explicit and observable.

### 13.1 Job Categories

-   Indexer refresh jobs (polling, scheduled searches)
-   Torrent tick/scheduler jobs
-   Validation jobs (post-download checks)
-   Subtitle fetch jobs (policy-driven)
-   Transcode queue workers
-   Cleanup/retention jobs

### 13.2 Requirements

-   Each job type has:
    -   stable name (enum-like)
    -   explicit config
    -   metrics for run count/duration/failures
    -   events for start/end/failure
-   Job concurrency is bounded
-   Jobs must be cancellable where meaningful

---

## 14. UI Product Requirements (High-Level but Strict)

You told me you prefer “too much data visualized,” so this section leans that way.

### 14.1 Navigation (Initial)

-   Request Dashboard (Overseerr-class front door)
-   Torrents
-   Audiobooks
-   Movies
-   TV
-   Subtitles
-   Transcoding
-   Adult
-   Ebooks
-   System (health/config/diagnostics)

### 14.2 Common UI Patterns Across Domains

-   List views:
    -   filterable
    -   searchable
    -   status chips
    -   queue state visibility
-   Detail views:

    -   artifacts panel (files, streams, subtitles)
    -   decisions panel (why chosen, why rejected)
    -   events timeline (recent state changes)
    -   actions panel (request, pause, promote edition, fetch subs, transcode)

### 14.3 Target-Aware UI (Normative)

-   Every domain item view must display target assignments.
-   Per-target preferred edition and cutoff status must be visible.
-   Requests and upgrades must be target-scoped and explicitly labeled.

### 14.4 “Rationale first” UI requirement

Every selection/upgrade view must show:

-   the chosen candidate
-   the top rejected candidates
-   the structured reasons and scores
-   the constraints that excluded others

No “trust me” automation.

---

## 15. Failure Modes and Expected Behavior

Revaer must not be brittle.

### 15.1 Common Failure Classes

-   Indexer fails or returns junk
-   Torrents stall
-   Disk space low
-   Validation fails (bad release)
-   Subtitles unavailable
-   Transcode fails due to codec/tooling
-   Ambiguous mapping (episode parsing, edition identity)

### 15.2 Required Behaviors

-   Failures create:
    -   decision records (rejection/remediation)
    -   events
    -   visible UI state
-   System offers remediation actions:
    -   retry with backoff
    -   request alternate candidate
    -   manual bind/override
-   No infinite retry loops without caps

---

## 16. Security and Privacy Requirements (Local-First)

-   Secrets never logged.
-   Access tokens are masked.
-   API should support auth extractors (even if initial deployments are trusted LAN).
-   Rate limiting and request size limits exist (per AGENT).

---

## 17. Release Philosophy (To Keep Codex Honest)

A feature is “done” only if:

-   it has:
-   DB procs + migrations
-   API surface
-   CLI surface (where relevant)
-   events
-   metrics/spans
-   tests meeting coverage gates
-   and just ci passes cleanly

No “implemented but unused” code is allowed.

---

## 18. Concrete “Definition of Done” per Domain (MVP Line)

This is the minimum complete boundary. Anything beyond is a follow-on, not required for baseline.

Torrenting MVP done when:

-   can add/list/pause/resume/remove torrents
-   can configure selection rules and see explain output
-   scheduler respects bandwidth + concurrency caps
-   state transitions are evented and observable
-   produces file manifest artifacts

Audiobooks MVP done when:

-   can search, request, download, validate audiobook editions
-   edition model exists with narrator/format distinctions
-   can promote preferred edition
-   can optionally trigger consolidate/transcode (policy-driven) without destroying originals

Movies MVP done when:

-   can search, request, download, validate movie editions
-   quality scoring with rationale exists
-   upgrades are explainable and policy-driven
-   subtitle and transcode pipelines can be triggered

TV MVP done when:

-   can track show, request missing episodes, map downloads to episodes deterministically
-   supports repacks/proper upgrades
-   exposes episode status lifecycle

Subtitles MVP done when:

-   subtitles are discovered/extracted/downloaded
-   subtitles persist and are indexed
-   missing subtitle fetch workflow exists
-   subtitles can be bound and promoted

Transcoding MVP done when:

-   jobs can be queued/run/cancelled
-   outputs are registered as artifacts
-   extracted subtitles are persisted and indexed
-   rollback preserves originals unless policy says otherwise

Adult MVP done when:

-   adult storage isolation exists
-   adult movies + scenes exist as separate entities
-   domain-scoped search/request/download works
-   no accidental crossover into movie/tv libraries

Ebooks MVP done when:

-   basic acquisition + validation + organization works
-   domain stays separated and doesn’t pollute other libraries

---

## 19. The Competitive Differentiators

For Revaer to actually replace the Arr stack rather than become “another service,” the killer differentiators are be:

-   Audiobooks done better than anyone else
-   Subtitle persistence (especially from transcoding) as a “forever gift to future you”
-   First-class decision records (audit trail) so you can trust the automation
-   Library-convergent naming while seeding without duplicate storage
-   Multi-target lifecycle management without separate instances
-   Request-first dashboard with visible “why” and rejected candidates
