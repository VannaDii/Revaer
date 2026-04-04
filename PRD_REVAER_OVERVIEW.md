# Revaer PRD — Media Domains & Functionality (Priority-Ordered)

## Document Intent

This PRD defines:

- What Revaer is and is not
- The priority order of media domains and why
- The minimum complete feature set per domain (MVP boundaries)
- The cross-cutting platform responsibilities (DB, events, config, API, CLI, UI, observability)
- The decision model: explainability, determinism, and auditable automation
- What Codex must treat as hard requirements, not suggestions

## 1. Product Definition

### 1.1 What Revaer Is

Revaer is a local-first media acquisition and management platform with:

- A torrent engine as the foundational acquisition substrate
- Domain managers for distinct media types (audiobooks, movies, TV, adult, ebooks)
- Artifact pipelines (subtitles, transcoding) that reduce ongoing management burden
- A system-wide commitment to:
    - deterministic automation
    - explainable decisions
    - normalized data
    - observable state transitions
    - reversible operations (where feasible)

### 1.2 What Revaer Is Not

Revaer is not:

- A streaming player
- A recommendation system
- A hosted SaaS
- A plugin marketplace
- A magical AI-driven “guess what I want” system
- An “Arr clone” in UX philosophy (it should be clearer and more inspectable)

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

- Torrenting is the acquisition substrate
- Audiobooks are under-served and require domain-specific logic early (multi-file sets, chaptering, narrators/editions)
- Movies/TV are high volume and benefit from stable selection + post-processing primitives
- Subtitles + transcoding are cross-cutting artifact pipelines; they should be driven by actual inventory and decisions from movie/TV/audiobooks
- Adult content requires strict isolation and explicit policy boundaries
- Ebooks are simpler and can reuse acquisition + library primitives

## 3. Product Goals and Success Criteria

### 3.1 Goals

- Reduce the number of separate services required for acquisition and management.
- Make automation deterministic and explainable.
- Minimize manual “subtitle hunt” and “transcode chores.”
- Treat audiobooks as first-class media, not an afterthought.
- Provide consistent APIs and CLI behaviors across domains.

### 3.2 Success Criteria (Measurable)

- “Why did it pick this release?” is always answerable via:
    - UI rationale panel
    - API response rationale fields
    - logs/spans with structured fields
- Subtitle burden reduction:
    - extracted subtitles are persisted and indexed
    - re-acquisition due to missing subtitles approaches zero over time
- Deterministic re-run:
    - given the same configuration and indexer results, selection is repeatable
- Safety and stability:
    - failures degrade gracefully (no panics; actionable errors)
    - No hidden background actions without traceable events

## 4. Global Constraints (Codex Must Not Violate)

These are product requirements that mirror AGENT.md.

### 4.1 Architectural Constraints

- Library-first: bins only wire; all logic in libs
- Each media domain is a crate or sub-crate vertical slice (no grab-bags)
- All runtime DB interaction via stored procedures only
- Normalized schema; JSONB banned
- All operations are accessible via:
    - API
    - CLI
    - UI (where applicable)

### 4.2 Engineering Constraints

- Rust 2024 only
- No dead code, no unused items
- Minimal dependencies; new dependency requires rationale in task record
- All ops through just; CI runs only `just ...`
- Observability required for externally visible operations:
    - tracing spans
    - metrics
    - events

### 4.3 Behavioral Constraints

- No “silent automation.”
- Every automated action must produce events and rationale.
- No irreversible destructive operations by default.
- Destructive actions require explicit policy configuration or confirmation (UI/CLI).
- No “guessing” semantics across domains.
- Torrenting does not decide media identity; domain managers do.

## 5. Shared Mental Model: Entities, Artifacts, Decisions

Revaer must separate:

- Acquisition objects (torrents, downloads)
- Inventory artifacts (files, streams, subtitles)
- Domain entities (audiobook, movie, episode, scene, ebook)
- Decisions (selection, upgrade, re-download, transcode) with rationales

### 5.1 Core Entity Categories

**Acquisition:**

- Torrent
- Magnet
- IndexerResult
- Tracker

**Inventory / Artifacts:**

- FileArtifact (stable byte identity: hash/fingerprint, size, container)
- PathBinding (artifact → path with role: payload_path, library_path, working_path)
- LinkKind (hardlink, symlink optional, physical copy)
- StreamArtifact (video/audio/subtitle streams)
- SubtitleArtifact (external or extracted)
- TranscodeJob + TranscodeOutput

**Domain Entities:**

- Audiobook (book-level)
- AudiobookEdition (release/encoding variant)
- AudiobookPart (tracks/chapters/files)
- Movie
- MovieEdition
- Show
- Season
- Episode
- AdultMovie
- AdultScene
- Ebook

**Decision Records:**

- SelectionDecision (what chosen, why)
- UpgradeDecision (why replaced)
- RejectionDecision (why not selected)
- RemediationDecision (e.g., subtitle fetch triggered)

### 5.2 Decision Record Minimum Fields (Normative)

Every decision record must include:

- `decision_id`
- `decision_kind`: enum { selection, upgrade, reject, remediate }
- `domain` (torrent/audiobook/movie/tv/subtitles/transcode/adult/ebook)
- `target_id` (required when targets apply)
- `entity identifiers` (domain entity id + relevant artifact ids)
- `policy snapshot reference` (which ruleset produced it)
- `candidate_set_id`: reference to an immutable, persisted snapshot of candidates considered, including their raw indexer metadata and scoring inputs
- `rationale fields` (structured, not interpolated strings)
    - `scored_attributes`: ordered list of `{attribute_id, value, weight, computed_score_contribution}`
    - `hard_constraints`: list of constraint evaluations with `{constraint_id, result: pass|fail, reason_code}`
    - `tie_breakers_used`: ordered list of comparator stages that resolved a tie, with stage identifier and value comparison summary
    - `final_choice_reason_codes` (enum-like)
- `timestamps`
- `correlation_id / request_id linkage`

Candidate sets must be immutable snapshots tied to the policy snapshot and indexer state at evaluation time.
Candidate snapshots must include all scoring-relevant metadata at evaluation time and must not depend on subsequent enrichment or external re-query.
Decision Records are immutable once written and must never be retroactively modified.

Lifecycle state names are normative and must be defined centrally in domain models.

### 5.3 Decision Engine Canonical Comparator Chain (Normative)

Unless a domain-specific rule explicitly overrides an earlier stage, comparator evaluation must be deterministic and follow this order:

1. Hard constraint pass/fail filtering (policy, language, format, safety gates)
2. Coverage gain (applicable only to backlog or multi-item evaluations within set-based domains such as TV and pack evaluation)
3. Quality score (quality profile + custom format scoring)
4. Swarm health score (torrent-based domains only)
5. Net storage delta score
6. Release age preference (policy-driven newer/older bias)
7. Stable lexical tie-breaker on canonical candidate identity

The comparator path used must be recorded in Decision Records for every final choice.
Hard constraints must be evaluated per logical unit of selection (for example, per episode within packs) before comparator scoring proceeds.
Hard constraints include policy rules, language/format requirements, subtitle profile requirements, disk safety thresholds, and indexer quarantine state.
For non-set domains (Movies, Audiobooks, Ebooks), coverage gain must be treated as zero and skipped.
Non-torrent domains must skip the swarm health stage without altering comparator ordering.
Coverage gain is evaluated as an absolute integer value and not a weighted ratio unless explicitly extended by policy.
Comparator ordering must be stable and produce identical results for identical candidate sets under identical policy snapshots.
All comparator dimensions must define explicit ordering direction (higher-is-better or lower-is-better).
Quality score and swarm health score are higher-is-better dimensions unless explicitly overridden by policy.
Net storage delta must favor lower net storage increase.
Release age preference must explicitly declare whether newer or older releases are preferred under the active policy snapshot.
Disk safety thresholds operate as hard constraints. Net storage delta scoring is applied only after disk safety constraints are satisfied.
Swarm health evaluation precedes release age preference and may override age bias when higher in comparator order.
Canonical candidate identity must be derived from a stable, deterministic identifier (for example, normalized release name + hash) and must not depend on runtime ordering or external query order.
Coverage gain is measured as the count of distinct logical units satisfied (for example, episodes) under the active target.
Comparator stages must be evaluated sequentially and short-circuit when a deterministic ordering is established; subsequent stages must not be evaluated unless required to resolve ties.
Comparator stage ordering is normative and must not be reordered without a policy snapshot version increment.
If no tie resolution is required beyond a given stage, `tie_breakers_used` must record only the stage at which ordering was resolved.
All comparator stages operate over the same immutable `candidate_set_id`; stages must not introduce new candidates or remove candidates except via hard constraint filtering.
Hard constraint failures are absolute and must not be overridden by scoring, tie-breaking, or policy weighting.

### 5.4 Lifecycle State Model (Normative)

Each domain edition must implement a centrally defined lifecycle state machine with at least:

- requested
- acquired
- media_validated
- subtitle_pending
- validated_complete
- failed
- blocked

State transitions must be deterministic and recorded in Decision Records or Events.
`validated_complete`, `failed`, and `blocked` are terminal states unless explicitly re-entered by user action or policy re-evaluation.

## 6. Storage Strategy & Seeding Integrity

### 6.1 Goal

Revaer must support library-convergent naming (Plex-friendly) without breaking seeding when payload bytes are unchanged.

### 6.2 Definitions (Normative)

- Payload file: the actual bytes the torrent client is seeding (participates in piece verification).
- Library file: the file path Plex consumes (naming/path conforms to library rules).
- Content-mutating transform: any operation that changes bytes (transcode, remux rewrite, audio normalization rewrite).
- Non-mutating rename/move: operations that change path/name without changing bytes.

### 6.3 Hard Requirements

- If content has not been mutated, Revaer must allow library-visible naming to converge with Plex conventions while preserving seeding.
- Revaer must avoid duplicate storage wherever possible.
- Revaer must never pretend seeding is intact when the payload bytes have changed.

### 6.4 Allowed Import Strategies (Policy-Driven)

1. Hardlink import + rename library path
    - Payload remains at the torrent client path.
    - Library file is a hardlink with Plex-friendly naming.
    - Renaming applies to the library link, not the payload.
    - Seeding continues because the payload remains intact and referenced by the client.

2. Atomic move payload into canonical seed+library structure (same filesystem only)
    - Payload lives in a canonical structured directory serving both seeding and library conventions.
    - Allowed only when:
        - same filesystem (atomic rename possible)
        - torrent client path update is supported and confirmed

3. Copy import (fallback)
    - Used when hardlinks are not possible (cross-filesystem, permissions, unsupported filesystem).
    - Used when policy explicitly prefers isolation.

### 6.5 Renames and Moves: Required Support

Revaer must support renaming library-visible artifacts to converge with Plex conventions while maintaining seeding:

- file rename (title/year/edition formatting)
- directory rename (season folder normalization, show folder normalization)
- moving within library root as long as it remains link-safe
- retitling editions without byte changes

### 6.6 Renames and Moves: Must Not Happen Silently

- If the underlying bytes change, the original torrent payload can no longer seed reliably.
- Revaer must not imply continued seeding in that case.
- When content-mutating transforms happen:
    - the library file becomes a new artifact lineage
    - seeding can continue only if:
        - the original payload is retained and remains available to the torrent client, or
        - a verified “reseed from new bytes” workflow exists (future, optional)

### 6.7 Required Internal Model (Storage Graph)

Revaer must model storage as artifacts and bindings, not “a path”:

- FileArtifact: stable identity for a specific byte payload (hash/fingerprint).
- PathBinding: maps FileArtifact → filesystem path(s), with role:
    - payload_path (torrent client)
    - library_path (Plex)
    - working_path (transcode staging)
- LinkKind:
    - hardlink
    - symlink (optional, policy-driven)
    - physical copy

## 6A. Import Contract & Filesystem Topology Validation (Normative)

Revaer must validate filesystem topology at startup, including:

- Download root vs library root separation
- Same-filesystem verification for hardlink eligibility
- Remote path mapping consistency
- Permission validation for atomic operations

Revaer must refuse unsafe configurations and emit actionable diagnostics.

Import must occur only after the download client reports completion, preventing premature ingestion.

Download roots and library roots must be distinct; configurations where they are equal must be rejected.
This invariant must be validated at startup and on configuration reload.

Renames operate on PathBinding, not on FileArtifact.

### 6.8 Required Invariants (Auditable)

- A seeding torrent must have at least one live payload_path binding that is byte-identical to the torrent’s expected content.
- A library item consumed by Plex must have a library_path binding.
- If library_path is a hardlink to payload_path, Revaer must record:
    - inode identity (or platform equivalent)
    - filesystem id
    - link count snapshot (optional but useful)

### 6.9 Diagnostics and Explainability (Must Exist)

Revaer must provide an “Import/Link Explain” output that answers:

- Did we hardlink, atomic move, copy, or fail?
- If we did not hardlink, why not?
    - cross-filesystem
    - insufficient permissions
    - unsupported filesystem
    - path policy conflict
- If seeding would break due to a planned action, say so before doing it.

CLI + API must expose:

- explain import / explain links for an item
- dry-run mode for rename plans

### 6.10 Policy Knobs (Minimum)

- preferred import mode: hardlink | atomic | copy
- allow symlinks: true/false (default false unless policy opts in)
    - symlink usage must not compromise seeding integrity or path validation invariants
- keep original payload after transcode: true/false (default true if seeding matters)
- minimum free space safety threshold
- rename strategy:
    - conservative (only library link)
    - aggressive (move payload and update client paths)

### 6.11 Edge Cases (Deterministic Handling Required)

- Multi-file torrents with partial selection:
    - link/copy only selected files
- Scene releases / multi-episode bundles:
    - map bindings per episode without touching payload bytes
- Subtitle sidecars:
    - may be renamed/moved with library path
    - do not affect seeding unless they are part of the torrent payload

## 7. Targets (Multi-instance Replacement)

### 7.1 Goal

Revaer must support multiple library targets per domain entity so users never need separate servers or duplicate instances.

### 7.2 Core Concept: Target

A Target is a policy envelope + storage envelope + quality envelope attached to a domain entity.

Example targets:

- movies-hd
- movies-uhd
- tv-hd
- tv-uhd
- audiobooks-mobile
- audiobooks-archival

### 7.3 Requirements (Normative)

- A single movie/show/audiobook may be tracked by multiple targets concurrently.
- Each target has:
    - quality profile (and custom format scoring set)
    - language/subtitle profile
    - storage root
    - seeding/import mode preferences
    - upgrade/cutoff rules
- Each target produces its own preferred edition and maintains its own lifecycle state.

### 7.4 Decision Model Implications

Selection is no longer “best overall.” It is “best for target X.”

- decision records must be target-scoped:
    - `decision.target_id` is mandatory where targets apply

### 7.5 UI Implications

Users must be able to:

- see which targets an item is assigned to
- see the preferred edition per target
- see per-target missing/cutoff status
- request/upgrade per target

### 7.6 Target Conflict & Artifact Sharing Rules (Normative)

- If multiple targets choose the same `FileArtifact`, the artifact must be shared rather than duplicated.
- Shared artifacts may have distinct `PathBinding` records per target/library path.
- If targets use different storage roots on the same filesystem, hardlink-based sharing is allowed by policy; otherwise controlled copy import is required.
- A target upgrade must not invalidate another target’s active preferred edition until that target is re-evaluated under its own policy.
- An artifact must not be deleted while it remains the preferred edition of any active target.
- Frozen editions are retention-protected and must not be deleted while frozen, even when not currently the preferred edition for a target.
- Retention cleanup must execute only after successful validation and promotion of replacement editions.
- Retention cleanup must evaluate artifact eligibility across all active targets before deletion; no target may trigger deletion of an artifact still required by another target.

## 8. Request Dashboard (Overseerr-class UX)

### 8.1 Goal

The default dashboard is a Request + Discovery hub and a live operational view of:

- what people want
- what Revaer is doing about it
- what’s blocked and why

### 8.2 Required Dashboard Modules (Minimum)

1. Unified Search
    - One search box, results grouped by domain:
        - Movies
        - TV
        - Audiobooks
        - Adult
        - Ebooks
    - Results show:
        - “already have it” status (per target)
        - “requested” status
        - “missing” status
        - confidence / ambiguity flags

2. Request Composer
    - Request button from any result
    - Request includes:
        - target selection (multi-target is first-class)
        - constraints (language, edition kind, narrator, etc.)
        - priority (normal/high)
        - optional “approval required” flag
    - Approval workflows are optional and must not block core functionality in single-user deployments.

3. Request Queue
    - Lifecycle:
        - requested → searching → candidate found → downloading → importing → validating → complete
    - Failure modes must include explicit reasons

4. Activity Feed
    - Human-readable but structured activity:
        - “Picked release X because …”
        - “Rejected Y due to constraint …”
        - “Fetched subtitles …”
        - “Transcode queued …”
    - Each item links to the underlying decision record

5. Operations Snapshot
    - Active downloads, seeding health, stalled items
    - Subtitle backlog
    - Transcode queue
    - Storage warnings
    - Indexer health warnings

### 8.3 “Why” Must Be Visible Here

Revaer’s request dashboard must show:

- last decision per request
- top 3 rejected candidates
- the exact constraint or score differences that mattered

This is required for user trust.

## 9. Platform Capabilities (Shared Across All Domains)

These are system-level services every domain uses.

### 9.1 Configuration System

- Typed config, validated at load
- Load order: defaults → file → env → CLI
- Effective config log emitted with secrets redacted
- Policy configuration is versionable and auditable

Config must include, at minimum:

- Indexer and tracker endpoints (if applicable)
- Bandwidth limits and schedules
- Storage roots and path strategies
- Naming rules (domain-specific)
- Language preferences (subtitles; audiobook metadata)
- Transcoding policies
- Adult content isolation settings
- Retention and cleanup rules

### 9.2 Events System

Events are required for:

- UI live updates (SSE)
- domain decoupling
- auditability

**Event Requirements:**

- event kinds are enums (no dynamic strings)
- payloads are schema-stable
- cardinality is controlled (avoid unbounded high-card fields)
- correlation IDs flow through

**Minimum Event Families:**

- `engine.*` (torrent lifecycle)
- `indexer.*` (search results, failures)
- `media.*` (domain entity changes)
- `artifacts.*` (subtitle extracted, transcode produced)
- `decisions.*` (decision made, decision applied)
- `jobs.*` (background tasks started/completed/failed)

Revaer must persist the top N rejected candidates (configurable, default ≥ 3) for every selection decision and expose them via API and UI.

Rejected candidate data must include:

- Score breakdown
- Failed constraints
- Comparator path used

### 9.3 API System (Axum)

- Versioned under `/v1`
- OpenAPI export is deterministic and committed
- Responses include rationale references where relevant
- API surfaces are domain-scoped:
    - `/v1/torrents/...`
    - `/v1/audiobooks/...`
    - `/v1/movies/...`
    - `/v1/tv/...`
    - `/v1/subtitles/...`
    - `/v1/transcode/...`
    - `/v1/adult/...`
    - `/v1/ebooks/...`
    - `/v1/system/...` (health, metrics pointers, config read-only)

### 9.4 CLI System

- Every domain must have CLI parity for core operations
- Output modes: `json|table` (json stable)
- Destructive operations require:
    - explicit `--yes` or
    - `--force` + policy allowing it

### 9.5 Storage / Filesystem Operations

- All FS ops flow through `revaer-fsops`
- Async-safe; `spawn_blocking` only where necessary
- Must support:
    - atomic moves (or best-effort with rollback)
    - hardlink creation and verification (same filesystem, inode identity capture)
    - disk space checks
    - path conflict resolution strategies
    - consistent hashing / fingerprinting

### 9.6 Observability

Every externally-triggered operation must include:

- tracing span at boundary
- structured fields (no interpolated messages)
- metrics for:
    - counts
    - durations
    - queues
    - failures

## 10. Domain PRDs (Detailed)

### 10.1 Torrenting Domain (Foundation)

#### 10.1.1 Scope

**Torrenting owns:**

- acquisition lifecycle
- file selection inside torrents
- session state and resource controls
- tracker behavior and health
- deterministic scheduling policies

**Torrenting does not own:**

- interpreting media semantics beyond extracted metadata signals
- naming conventions per media domain
- subtitle logic (except identifying embedded streams as artifacts)

#### 10.1.2 Primary User Workflows

- Add torrent by:
    - magnet
    - .torrent file
    - indexer selection (“send to revaer”)
- Monitor lifecycle:
    - queued → downloading → verifying → seeding → paused/stopped → removed
- Inspect selection:
    - which files included/excluded and why
- Control seeding:
    - ratio/time policy
    - manual override
- Handle failures:
    - tracker unreachable
    - stalled
    - disk full
    - hash fail

#### 10.1.3 Required Features (MVP)

**Acquisition:**

- add torrent with metadata capture
- list torrents with state summary
- pause/resume/remove
- set per-torrent tags/labels (used by downstream routing)

**Selection:**

- user-customizable glob filters (include/exclude)
- sensible defaults:
    - include: common archives and media extensions
    - exclude: samples, nfo if unwanted, junk patterns
- candidate scoring must include swarm health as a scoring dimension for torrent-based acquisitions
- selection rationale recorded:
    - matched include rule
    - matched exclude rule
    - size-based or priority-based pruning if configured

**State Machine:**

- explicit states with transitions logged and evented
- determinism:
    - transitions must not depend on non-deterministic ordering without stable sort keys

**Resource Governance:**

- global bandwidth caps
- scheduled bandwidth windows (time-based)
- max active torrents
- max active downloads
- queue scheduling strategy:
    - stable priority ordering
    - fairness rules (configurable)

**Tracker Handling:**

- tracker list management
- tracker health tracking
- backoff and retries (idempotent ops only)

#### 10.1.4 Non-Trivial Requirements

- File manifest production:
    - once metadata is known, produce a normalized manifest:
        - file paths within torrent
        - sizes
        - priority
        - selected/not selected
        - hashes if available
- “Explain” endpoint:
    - for a given torrent, return:
        - candidate files
        - applied filters
        - selected set
        - rationale codes
        - policy snapshot reference used for evaluation
- Safety:
    - removing a torrent must have policy options:
        - remove torrent only
        - remove torrent + data
- Default behavior must be least destructive.
- Torrent clients must never download directly into library roots; downloads must land in managed payload roots and import into library roots via policy-driven binding.

#### 10.1.5 Events (Minimum)

- `engine.torrent_added`
- `engine.torrent_state_changed`
- `engine.torrent_removed`
- `engine.file_selection_changed`
- `engine.bandwidth_policy_applied`
- `engine.tracker_health_updated`
- `engine.error_occurred`

#### 10.1.6 API (Minimum)

- `POST /v1/torrents` (magnet or torrent file reference)
- `GET /v1/torrents`
- `GET /v1/torrents/{id}`
- `POST /v1/torrents/{id}/pause`
- `POST /v1/torrents/{id}/resume`
- `POST /v1/torrents/{id}/remove` (policy flags)
- `GET /v1/torrents/{id}/explain`

#### 10.1.7 CLI (Minimum)

- `revaer torrents add --magnet ...`
- `revaer torrents list`
- `revaer torrents show`
- `revaer torrents pause/resume`
- `revaer torrents remove [--delete-data] [--yes]`
- `revaer torrents explain`

### 10.2 Audiobooks Domain (Tier-1 Priority)

#### 10.2.1 Scope

Audiobooks owns:

- audiobook identity resolution
- edition handling
- multi-file set validation
- chapter handling (where possible)
- narrator and series mapping
- metadata enrichment (from local/external sources if configured)

Audiobooks does not own:

- playback
- DRM cracking
- “recommendations”

#### 10.2.2 Core Challenges

Audiobooks are often:

- multi-file (many mp3s)
- inconsistent metadata
- ambiguous editions (narrator/version)
- mixed quality sources
- “book + extras + cover art” bundles

Revaer must model this explicitly rather than forcing audiobook into movie-like assumptions.

#### 10.2.3 Required Entity Model

**Audiobook:**

- book identity (title, author(s), series, series_index)
- language
- publication year (if known)
- canonical identifiers where possible (ISBN exists sometimes, but not guaranteed)

AudiobookEdition:

- narrator(s)
- runtime duration (estimated/declared)
- format (m4b/mp3/flac)
- bitrate/codec profile
- source grouping (release group, provider hint)
- “chapterized” boolean capability
- cover art artifact (optional)

AudiobookPart:

- file artifacts and ordering
- chapter metadata links (if extracted)

#### 10.2.4 Required Workflows

**Discovery:**

- search by:
    - title
    - author
    - series
    - narrator (optional)
- show candidate sets with scoring + rationale

Acquisition:

- select an edition candidate
- route to torrenting with a routing tag (audiobooks)
- track the download until complete
- validate completeness

Validation:

- ensure required audio artifacts exist
- enforce minimum duration thresholds (if available)
- detect “sample/trailer only”
- detect missing segments based on file counts/duration gaps (heuristic-driven, deterministic)

Post-processing (policy-driven):

- if mp3 multi-file and policy prefers m4b:
    - offer a “consolidate to m4b” transcode job
- ensure cover art present if available
- persist chapter metadata if extracted

Library management:

- mark preferred edition
- handle upgrades:
    - better bitrate
    - correct narrator
- preserve chapterized version where possible
- preserve history:
    - don’t delete old edition artifacts unless policy allows

#### 10.2.5 Scoring and Selection Requirements

Selection must be explainable and deterministic.

Example scoring contributors (must be structured, not free text):

- preferred narrator match
- preferred format match (m4b > mp3 if policy)
- chapterized presence
- bitrate thresholds
- completeness confidence
- release group trust tier
- language match

Hard constraints examples:

- language must match unless user overrides
- runtime must exceed minimum threshold for a given book (if known)
- exclude “abridged” unless requested

#### 10.2.6 Events (Minimum)

- `media.audiobook_created`
- `media.audiobook_edition_discovered`
- `media.audiobook_acquisition_requested`
- `media.audiobook_download_completed`
- `media.audiobook_validation_failed`
- `media.audiobook_edition_promoted` (new preferred)
- `decisions.audiobook_selection_made`

#### 10.2.7 API (Minimum)

- `GET /v1/audiobooks/search?q=...`
- `POST /v1/audiobooks/{id}/request` (edition constraints)
- `GET /v1/audiobooks`
- `GET /v1/audiobooks/{id}`
- `GET /v1/audiobooks/{id}/editions`
- `POST /v1/audiobooks/{id}/editions/{edition_id}/promote`
- `GET /v1/audiobooks/{id}/decisions`

#### 10.2.8 CLI (Minimum)

- revaer audiobooks search “…”
- revaer audiobooks request [constraints flags]
- revaer audiobooks show
- revaer audiobooks editions
- revaer audiobooks promote <edition_id>

### 10.3 Movies Domain

#### 10.3.1 Scope

**Movies owns:**

- movie identity resolution
- edition variants (theatrical, extended, remaster)
- quality profiles
- upgrade policies

**Movies does not own:**

- subtitle acquisition logic (subtitles domain)
- transcoding logic (transcoding domain)
- torrent file selection beyond routing tags

#### 10.3.2 Required Workflows

**Discovery:**

- search by:
    - title
    - year
    - optional imdb/tmdb id (if configured)
- list candidates with scoring and rationale

**Acquisition:**

- choose candidate or allow auto-pick based on policy
- route to torrenting with routing tags (movies)
- monitor until complete

Validation:

- ensure primary video artifact exists
- ensure minimum resolution/codec constraints met
- detect “sample” or “cam” (policy-driven)
- detect missing audio streams if required

Post-acquisition:

- trigger subtitle pipeline based on policy:
    - require at least one subtitle in preferred language
    - or accept embedded
- trigger transcoding based on policy:
    - container normalization
    - audio track selection
    - subtitle extraction and persistence

Upgrades:

- define what qualifies as “better”
    - resolution tier
    - codec preference
    - HDR preference
    - audio preference
    - release group trust
- upgrade decision must:
    - not be arbitrary
    - preserve ability to roll back if policy says “keep previous”

Repack / Proper Awareness:

- Detect PROPER, REPACK, V2, V3 indicators
- Prefer repack over original within same quality tier when policy allows
- Avoid downgrade via mis-scored custom format
- Record repack-based upgrade reasoning explicitly in Decision Records

#### 10.3.3 Movie Identity and Edition Modeling

**Movie:**

- title
- year
- canonical id (tmdb/imdb) if available
- language/original_language (if known)

**MovieEdition:**

- edition_kind (theatrical/extended/directors_cut/remaster/unknown)
- resolution
- codec
- hdr flags
- audio profile (channels, codec)
- source group/provider hints
- file artifacts

#### 10.3.4 Events (Minimum)

- `media.movie_created`
- `media.movie_candidate_discovered`
- `media.movie_acquisition_requested`
- `media.movie_download_completed`
- `media.movie_validation_failed`
- `media.movie_edition_promoted`
- `decisions.movie_selection_made`

#### 10.3.5 API (Minimum)

- `GET /v1/movies/search?q=...`
- `POST /v1/movies/{id}/request`
- `GET /v1/movies`
- `GET /v1/movies/{id}`
- `GET /v1/movies/{id}/editions`
- `POST /v1/movies/{id}/editions/{edition_id}/promote`
- `GET /v1/movies/{id}/decisions`

### 10.4 TV Shows Domain

#### 10.4.1 Scope

**TV owns:**

- show identity resolution
- season/episode tracking
- episode-level acquisition and upgrades
- multi-episode bundles

**TV does not own:**

- subtitle and transcoding logic (separate domains)
- torrent engine policies (except routing)

#### 10.4.2 Required Workflows

Discovery:

- search shows by name
- choose show entry and begin tracking

Tracking:

- track seasons and episodes
- allow two modes:
    - “air-date/standard”
    - “absolute” (anime-style) if configured per show

Acquisition:

- request:
    - single episode
    - full season
    - missing episodes (bulk)
- route to torrenting (tv tag)
- map downloaded files to episodes deterministically

Validation:

- detect mismatches:
    - wrong season
    - wrong episode numbering
    - multi-episode file that needs splitting semantics (logical mapping, not physical split)
- ensure completeness per requested set

Upgrades:

- similar to movies, but episode-level:
    - repack/proper preference
    - resolution/codec
    - audio preferences

Pack Handling:

- Detect season, multi-season, and full-series packs
- Map pack contents to episodes deterministically
- Support partial overlap with existing seasons
- Prefer pack acquisition when coverage and quality justify it
- Avoid splitting physical payloads; perform logical episode mapping only
- Generate Decision Records for pack vs single-episode decisions

#### 10.4.3 TV Entity Modeling

**Show:**

- title
- canonical id
- mode config (air-date/absolute)
- language preferences

**Season:**

- season_number
- tracked boolean

**Episode:**

- episode_number
- title if known
- air_date if known
- status:
    - missing
    - requested
    - acquired
    - media_validated
    - failed

Episode status values map onto the canonical lifecycle state model defined in §5.4.
Episode status is a projection of the preferred `EpisodeEdition` lifecycle state for the active target and must not introduce independent lifecycle semantics.

**EpisodeEdition:**

- quality attributes
- file artifacts
- subtitle associations

#### 10.4.4 Events (Minimum)

- `media.show_tracked`
- `media.episode_requested`
- `media.episode_acquired`
- `media.episode_validation_failed`
- `media.episode_edition_promoted`
- `decisions.episode_selection_made`

#### 10.4.5 API (Minimum)

- `GET /v1/tv/search?q=...`
- `POST /v1/tv/shows/{id}/track`
- `GET /v1/tv/shows`
- `GET /v1/tv/shows/{id}`
- `POST /v1/tv/shows/{id}/request` (missing|season|episode)
- `GET /v1/tv/shows/{id}/episodes`
- `GET /v1/tv/shows/{id}/decisions`

### 10.5 Subtitles Domain (Artifact Pipeline)

#### 10.5.1 Scope

Subtitles owns:

- acquisition of external subtitles
- extraction of embedded subtitles (as artifacts)
- normalization, storage, indexing, association to domain entities

Subtitles does not own:

- transcoding decisions (it can request work, but doesn’t run transcoding)
- media selection decisions (it responds to media inventory and policies)

#### 10.5.2 Subtitle Artifact Types

**SubtitleArtifact:**

- `source_kind`:
    - embedded (extracted from media container)
    - external_download (e.g., opensubtitles-like)
    - sidecar (already present next to file)
- language (BCP-47 if possible)
- hearing_impaired flag if known
- format (srt/ass/vtt/pgs)
- confidence (if derived)
- checksum/fingerprint
- storage path

#### 10.5.3 Hard Requirement: Persistence and Indexing

**Normative:**

- Any subtitle discovered or extracted must be persisted
- Persistence includes:
    - stable storage path under Revaer-managed roots
    - DB record linking:
        - subtitle ↔ file artifact
        - subtitle ↔ domain entity edition (movie edition / episode edition)
    - searchable index on:
        - language
        - media id
        - edition id
        - source_kind

#### 10.5.4 Required Workflows

**Discovery:**

- inspect existing inventory for:
    - embedded subtitle streams
    - sidecar subtitles
- register them as SubtitleArtifacts

**Acquisition:**

- if missing subtitles per policy:
    - query configured providers (indexer-like abstraction)
    - rank results deterministically
    - download and validate basic integrity

**Normalization:**

- optional conversions (policy-driven):
    - convert to srt/vtt where feasible
    - keep originals too, unless policy says otherwise
    - remove obvious broken artifacts deterministically (with reason codes)

**Association:**

- link subtitles to the correct domain edition
- handle ambiguous mapping:
    - store as “unbound subtitle” with candidate links and confidence
    - UI shows “needs binding” workflow

#### 10.5.5 UI Expectations

- Media detail pages must display:
    - available subtitles by language and source
    - “preferred subtitle” selection
    - “fetch missing subtitles” action
    - “bind subtitle to edition” action if ambiguous

#### 10.5.6 Events (Minimum)

- `artifacts.subtitle_discovered`
- `artifacts.subtitle_extracted`
- `artifacts.subtitle_downloaded`
- `artifacts.subtitle_bound`
- `artifacts.subtitle_normalized`
- `artifacts.subtitle_failed`
- `decisions.subtitle_selection_made`

#### 10.5.7 API (Minimum)

- `GET /v1/subtitles/by-media/{media_kind}/{media_id}`
- `POST /v1/subtitles/fetch` (media_kind, media_id, language prefs)
- `POST /v1/subtitles/{id}/bind` (edition id)
- `POST /v1/subtitles/{id}/promote` (preferred)
- `GET /v1/subtitles/{id}`

### 10.5.8 Subtitle Policy Profile (Normative)

Each target must define a `SubtitleProfile` that is evaluated as part of hard constraint processing and validation.

`SubtitleProfile` must include:

- `preferred_languages`: ordered list (BCP‑47 codes)
- `require_full_subtitles`: boolean
- `allow_forced_only`: boolean
- `allow_hearing_impaired`: boolean
- `prefer_embedded_over_external`: boolean
- `minimum_confidence_score`: numeric threshold (provider confidence or heuristic confidence)
- `reject_if_unavailable`: boolean
- `fallback_language_policy`: enum { none, next_preferred, any_available }

Hard Constraint Rules:

- If `reject_if_unavailable` is true and required subtitle conditions are not met, the candidate must fail hard constraint evaluation before comparator scoring.
- SubtitleProfile evaluation must occur per logical unit (episode within pack, movie edition, audiobook part where applicable).
- Partial pack eligibility must reflect subtitle compliance per episode.

Ranking Rules:

When multiple subtitle candidates exist, ranking must follow:

1. Language order match (exact order precedence)
2. Embedded vs external preference
3. Hearing-impaired preference (if allowed)
4. Confidence score
5. Stable lexical tie-break

Lifecycle Enforcement:

- An edition must not transition to `validated_complete` unless SubtitleProfile requirements are satisfied or explicitly waived.
- `subtitle_status` must reflect { complete, partial, missing, blocked } and gate lifecycle transitions.
- Upgrade decisions must not promote an edition that violates SubtitleProfile constraints.

Persistence Guarantees:

- Extracted subtitles must be persisted and indexed.
- Subtitle artifacts must be reusable across targets when compatible.

### 10.6 Transcoding Domain (Artifact Pipeline)

#### 10.6.1 Scope

Transcoding owns:

- job orchestration (queueing, scheduling, tracking)
- artifact transformation policies
- output artifact registration
- subtitle extraction persistence requirement

Transcoding does not own:

- deciding what media to acquire
- subtitle provider acquisition (that’s subtitles domain)

#### 10.6.2 Critical Requirement: Extracted Subtitles Are Persisted

**Normative:**

- If a transcode job inspects a file and finds embedded subtitles, those subtitles must:
    - be extracted (if policy allows)
    - be persisted and indexed
    - be associated to the domain entity edition
- This is required because it reduces future subtitle acquisition burden

#### 10.6.3 Job Model

**TranscodeJob:**

- input file artifact id
- job kind:
    - container_normalize
    - audio_normalize
    - consolidate (audiobooks)
    - subtitle_extract_only
- policy snapshot reference
- state:
    - queued
    - running
    - succeeded
    - failed
    - cancelled
- progress (bounded and stable, not spammy)
- output artifact ids
- logs pointer (structured events, not raw text blobs in DB)

#### 10.6.4 Required Workflows

Triggering:

- manual trigger (UI/CLI/API)
- automatic trigger (policy-driven) after validation:
    - “if file is mkv and prefer mp4”
    - “if audiobook is multi-mp3 and prefer m4b”
    - “if audio lacks preferred track layout”

Execution:

- resource caps
- queue fairness
- cancellation support

Artifact registration:

- output file registered as new FileArtifact
- output replaces or sits alongside input based on policy

Rollback:

- if transcode output fails validation, preserve input
- avoid destructive delete unless policy allows
- original payload artifacts required for active seeding must not be deleted by transcode workflows

#### 10.6.5 Events (Minimum)

- `jobs.transcode_queued`
- `jobs.transcode_started`
- `jobs.transcode_progress` (rate-limited)
- `jobs.transcode_succeeded`
- `jobs.transcode_failed`
- `artifacts.subtitle_extracted` (as part of transcode flow)

#### 10.6.6 API (Minimum)

- `POST /v1/transcode/jobs` (input, kind, policy override optional)
- `GET /v1/transcode/jobs`
- `GET /v1/transcode/jobs/{id}`
- `POST /v1/transcode/jobs/{id}/cancel`

### 10.7 Adult Content Domain (Explicit Isolation)

#### 10.7.1 Scope

**Adult content owns:**

- explicit media management as a first-class domain
- separate identity model for:
    - adult movies
    - scenes (clip-level)
- strict isolation boundaries and defaults

**Adult content does not own:**

- unique acquisition engine (still torrenting)
- transcoding implementation (uses transcode domain)
- subtitle provider logic (uses subtitle domain when desired)

#### 10.7.2 Isolation Requirements (Normative)

- Default storage roots must be separate from non-adult media
- Default UI navigation must be separate and clearly labeled
- Default policies must avoid accidental cross-domain mixing:
    - adult tags do not route into movie/tv libraries
- Search and list endpoints must be domain-scoped:
    - adult results never appear in non-adult endpoints unless explicitly requested

#### 10.7.3 Scene-Level Model Requirements

**AdultScene:**

- performer metadata (if available)
- studio label (if available)
- tags (normalized; no JSONB)
- duration (if known)
- source release group metadata

**AdultMovie:**

- title
- year (if known)
- performers (normalized relation)
- editions

#### 10.7.4 Required Workflows

- Search and request adult movies
- Search and request scenes
- Validate and organize into isolated library
- Optional subtitle handling (less common, but supported)
- Optional transcoding

#### 10.7.5 Events (Minimum)

- `media.adult_movie_created`
- `media.adult_scene_created`
- `media.adult_acquisition_requested`
- `media.adult_download_completed`
- `decisions.adult_selection_made`

#### 10.7.6 API (Minimum)

- `GET /v1/adult/search?q=...`
- `POST /v1/adult/movies/{id}/request`
- `POST /v1/adult/scenes/{id}/request`
- `GET /v1/adult/movies`
- `GET /v1/adult/scenes`
- `GET /v1/adult/{kind}/{id}`

### 10.8 Ebooks Domain (Lower Priority, Minimal Initial Scope)

#### 10.8.1 Scope

**Ebooks owns:**

- acquiring ebook files
- basic metadata validation and organization
- series grouping where possible

**Ebooks does not own:**

- reading UX
- annotation sync
- OCR

#### 10.8.2 Required Workflows

- Search by title/author
- Request acquisition
- Validate integrity:
    - file opens (basic validation)
    - expected formats (epub/pdf)
- Organize into library root
- Maintain edition variants (optional, later)

#### 10.8.3 Events (Minimum)

- `media.ebook_created`
- `media.ebook_acquisition_requested`
- `media.ebook_download_completed`
- `media.ebook_validation_failed`
- `decisions.ebook_selection_made`

#### 10.8.4 API (Minimum)

- `GET /v1/ebooks/search?q=...`
- `POST /v1/ebooks/{id}/request`
- `GET /v1/ebooks`
- `GET /v1/ebooks/{id}`

## 11. Routing Rules: How Torrents Map to Domains

This is the glue that prevents a “giant bucket of downloads.”

### 11.1 Routing Inputs

Routing must be driven by:

- explicit request context (user requested movie vs audiobook)
- tags/labels attached at request time
- indexer category signals (if present)
- deterministic heuristics (only as a fallback, and must be auditable)

### 11.2 Routing Outputs

Routing produces:

- target domain
- target entity (if known)
- storage root selection
- naming policy selection
- post-processing pipeline triggers (subtitles/transcode)

### 11.3 Hard Rule

No “automatic domain reassignment” without an explicit decision record and an event.

## 11A. Pack-Aware Acquisition & Set-Cover Optimization (Normative)

### 11A.1 Goal

Revaer must treat multi-episode, multi-season, and full-series packs as first-class acquisition units rather than incidental multi-file releases.

Backlog filling must minimize redundant downloads while maximizing coverage and quality.

### 11A.2 Pack Types (Explicit Modeling Required)

Revaer must detect and model:

- Episode pack (multi-episode bundle)
- Season pack
- Multi-season pack
- Full-series pack

Pack classification must be deterministic and produce structured reason codes.

### 11A.3 Set-Cover Optimization Requirement

Backlog acquisition must be evaluated as a set-cover optimization problem.

For a given target and domain entity, candidate releases must be evaluated based on:

- Coverage gain (number of missing episodes satisfied)
- Quality tier and scoring output
- Redundancy overlap with existing editions
- Net storage delta
- Swarm health
- Policy constraints

Hard constraint filtering must occur before coverage gain is calculated for pack evaluation.

Revaer must prefer a pack over individual releases when the pack provides a superior coverage-to-cost ratio under policy rules.

All pack decisions must generate Decision Records.

### 11A.4 Partial Overlap Handling

Revaer must support:

- Importing only missing episodes from a pack when quality rules allow
- Replacing existing lower-quality seasons with higher-quality pack versions
- Rejecting overlapping content deterministically with structured reason codes
- Avoiding redundant re-download of episodes already at cutoff

Pack overlap resolution must be explicit and auditable.
Pack handling must perform logical mapping of covered units without altering the physical payload structure unless explicitly required by policy.

### 11A.5 Pack Hard Constraint Evaluation (Normative)

- Hard constraints must be evaluated per episode within a pack.
- A pack may be:
    - fully eligible (all covered episodes pass constraints)
    - partially eligible (only a subset pass constraints)
    - fully rejected (none pass constraints)
- Partial eligibility must be recorded explicitly and may result in partial logical import under policy.
- SubtitleProfile rejection rules must be applied per episode before pack eligibility classification.
- When partially eligible, coverage gain must be calculated only on compliant episodes.
- A partially eligible pack remains a single logical candidate with coverage calculated only on compliant episodes; it must not be implicitly split into multiple derived candidates.
- Pack comparator evaluation occurs within the same candidate context as non-pack releases and must not introduce separate comparator paths.

### 11A.6 Deterministic Pack Comparator Tie-Breakers (Normative)

Pack decisions must apply the Canonical Comparator Chain defined in §5.3 after hard constraints and pack eligibility checks.

Policy may set maximum acceptable pack size and must reject pack choices that downgrade episodes already at cutoff.
A pack must not downgrade any episode already meeting cutoff under the active target, even if overall pack score is higher.
This rule applies regardless of pack-level quality score.

## 12. Data and Schema Requirements (Stored Procedure First)

This section defines required schema capabilities.

### 12.1 Normalization Requirements

- Avoid conglomerate blobs
- Use join tables for:
    - entity ↔ edition
    - edition ↔ file artifacts
    - file ↔ subtitle artifacts
    - domain entity ↔ decision records
    - domain entity ↔ acquisition requests

### 12.2 Stored Procedure Surface Expectations

For each domain:

- create/update entities
- record discoveries (candidates)
- record acquisitions
- record validations
- record decisions
- link artifacts
- query views used by API/UI

Every “meaningful action” should map to a stored procedure call that can be tested.

## 13. Background Jobs and Schedulers

Revaer will have background jobs; they must be explicit and observable.

### 13.1 Job Categories

- Indexer refresh jobs (polling, scheduled searches)
- Torrent tick/scheduler jobs
- Validation jobs (post-download checks)
- Subtitle fetch jobs (policy-driven)
- Transcode queue workers
- Cleanup/retention jobs

### 13.2 Requirements

- Each job type has:
    - stable name (enum-like)
    - explicit config
    - metrics for run count/duration/failures
    - events for start/end/failure
- Job concurrency is bounded
- Jobs must be cancellable where meaningful

## 13A. Backlog Scheduler & Search Budget Model (Normative)

### 13A.1 Priority Tiers

Backlog processing must be prioritized:

1. Manual user requests
2. Newly tracked current-season items
3. Missing historical backlog
4. List/import-driven additions

Priority tiers must be deterministic and configurable.
Backlog prioritization must be domain-aware and must prevent starvation of higher-priority requests across domains.
Manual user requests must preempt lower-priority scheduled backlog work when resource limits are constrained.

### 13A.2 Search Budget Controls

Revaer must enforce:

- Indexer request rate limits
- Batch search windows
- Per-target search quotas
- Backoff under repeated failure
- Maximum concurrent search jobs

Revaer must prevent search storms when large backlogs are introduced.
Backlog scheduling must incorporate aggregate transient disk simulation to prevent concurrent acquisitions from exceeding configured storage safety thresholds.

### 13A.3 Indexer Capability & Reliability Model (Normative)

Revaer must model indexer capabilities and health explicitly, including:

- RSS support flag
- interactive search support flag
- capability detection state (verified/assumed/unknown)
- reliability score history
- health-based quarantine status

Backlog and scheduler decisions must gate behavior by capability and quarantine state to avoid dispatching unsupported or unhealthy work.
Automatic backlog searches must use only indexers with verified RSS support when operating in RSS-driven modes.

Indexer reliability score must influence candidate ranking and search scheduling decisions, and quarantined indexers must not participate in automatic searches.
Indexer reliability must not override quality score as the primary ranking factor; it may influence tie-breaking and search ordering but must not downgrade higher-quality candidates solely due to indexer reliability.
Indexer reliability influence must be bounded such that it cannot invert quality tier precedence.
Reliability influence must be applied consistently across all candidates within a decision context and must not be dynamically weighted per candidate in a non-deterministic manner.
Indexer quarantine state must define deterministic recovery conditions (for example, N consecutive successful interactions).

## 14. UI Product Requirements (High-Level but Strict)

### 14.1 Navigation (Initial)

- Request Dashboard (Overseerr-class front door)
- Torrents
- Audiobooks
- Movies
- TV
- Subtitles
- Transcoding
- Adult
- Ebooks
- System (health/config/diagnostics)

### 14.2 Common UI Patterns Across Domains

- List views:
    - filterable
    - searchable
    - status chips
    - queue state visibility
- Detail views:
    - artifacts panel (files, streams, subtitles)
    - decisions panel (why chosen, why rejected)
    - events timeline (recent state changes)
    - actions panel (request, pause, promote edition, fetch subs, transcode)

### 14.3 Target-Aware UI (Normative)

- Every domain item view must display target assignments.
- Per-target preferred edition and cutoff status must be visible.
- Requests and upgrades must be target-scoped and explicitly labeled.

### 14.4 “Rationale first” UI requirement

Every selection/upgrade view must show:

- the chosen candidate
- the top rejected candidates
- the structured reasons and scores
- the constraints that excluded others

No “trust me” automation.

## 15. Failure Modes and Expected Behavior

Revaer must not be brittle.

### 15.1 Common Failure Classes

- Indexer fails or returns junk
- Torrents stall
- Disk space low
- Validation fails (bad release)
- Subtitles unavailable
- Transcode fails due to codec/tooling
- Ambiguous mapping (episode parsing, edition identity)

### 15.2 Required Behaviors

- Failures create:
    - decision records (rejection/remediation)
    - events
    - visible UI state
- System offers remediation actions:
    - retry with backoff
    - request alternate candidate
    - manual bind/override
- No infinite retry loops without caps

## 15A. Disk-Aware Decision Engine (Normative)

Before acquisition or upgrade, Revaer must simulate net storage impact.

Decision scoring must consider:

- Download size
- Size of replaced artifacts
- Available free space
- Configured minimum free space threshold
- Target-specific storage limits

Revaer must reject or defer acquisitions that violate storage safety policies, producing explicit Decision Records with storage-related reason codes.
Disk simulation must consider peak transient storage requirements during download and import, not only final steady-state net delta.
Disk-aware evaluation must apply equally to pack acquisitions and single-release acquisitions.
Disk simulation must account for retention policies that may delete replaced artifacts only after successful validation of replacements.
When disk safety thresholds are violated, acquisition must be deferred rather than auto-rejected unless explicitly configured otherwise.

## 16. Security and Privacy Requirements (Local-First)

- Secrets never logged.
- Access tokens are masked.
- API should support auth extractors (even if initial deployments are trusted LAN).
- Rate limiting and request size limits exist (per AGENT).

## 16A. Policy Snapshotting & Re-Evaluation (Normative)

Revaer must version policy configurations and attach a policy snapshot reference to every Decision Record.
A policy snapshot must include a deterministic version identifier and a hash of its effective configuration state.

### 16A.1 Re-Evaluation Workflow

Revaer must support:

- Re-evaluating existing library items under a new policy snapshot
- Producing a structured diff preview:
    - items that would upgrade
    - items that would downgrade
    - items unaffected
- Applying or canceling proposed changes explicitly

No silent bulk re-evaluation is permitted.

### 16A.2 Freeze Controls

Revaer must support:

- Freeze edition (prevent further upgrades)
- Freeze target (prevent automatic re-evaluation)
- Complete-and-freeze lifecycle state (for ended series)

All re-evaluation actions must generate Decision Records.
Complete-and-freeze must disable automatic search and upgrade evaluation for the affected entity under that target.
Freeze must suppress automatic upgrade decisions even if higher-scoring candidates become available.

## 17. Release Philosophy (To Keep Codex Honest)

A feature is “done” only if:

- it has:
    - DB procs + migrations
    - API surface
    - CLI surface (where relevant)
    - events
    - metrics/spans
    - tests meeting coverage gates
    - and just ci passes cleanly

No “implemented but unused” code is allowed.

## 18. Concrete “Definition of Done” per Domain (MVP Line)

This is the minimum complete boundary. Anything beyond is a follow-on, not required for baseline.

Torrenting MVP done when:

- can add/list/pause/resume/remove torrents
- can configure selection rules and see explain output
- scheduler respects bandwidth + concurrency caps
- state transitions are evented and observable
- produces file manifest artifacts

Audiobooks MVP done when:

- can search, request, download, validate audiobook editions
- edition model exists with narrator/format distinctions
- can promote preferred edition
- can optionally trigger consolidate/transcode (policy-driven) without destroying originals

Movies MVP done when:

- can search, request, download, validate movie editions
- quality scoring with rationale exists
- upgrades are explainable and policy-driven
- subtitle and transcode pipelines can be triggered

TV MVP done when:

- can track show, request missing episodes, map downloads to episodes deterministically
- supports repacks/proper upgrades
- exposes episode status lifecycle

Subtitles MVP done when:

- subtitles are discovered/extracted/downloaded
- subtitles persist and are indexed
- missing subtitle fetch workflow exists
- subtitles can be bound and promoted

Transcoding MVP done when:

- jobs can be queued/run/cancelled
- outputs are registered as artifacts
- extracted subtitles are persisted and indexed
- rollback preserves originals unless policy says otherwise
- transcode outputs must not trigger deletion of original artifacts if those artifacts remain preferred or frozen under any active target

Adult MVP done when:

- adult storage isolation exists
- adult movies + scenes exist as separate entities
- domain-scoped search/request/download works
- no accidental crossover into movie/tv libraries

Ebooks MVP done when:

- basic acquisition + validation + organization works
- domain stays separated and doesn’t pollute other libraries

## 19. The Competitive Differentiators

For Revaer to actually replace the Arr stack rather than become “another service,” the killer differentiators are:

- Audiobooks done better than anyone else
- Subtitle persistence (especially from transcoding) as a “forever gift to future you”
- First-class decision records (audit trail) so you can trust the automation
- Library-convergent naming while seeding without duplicate storage
- Multi-target lifecycle management without separate instances
- Request-first dashboard with visible “why” and rejected candidates

## 20. Simulation & Dry-Run Capabilities (Normative)

Revaer must provide simulation endpoints and CLI commands for:

- Simulate adding a show or movie
- Simulate applying a new policy snapshot
- Simulate pack acquisition impact
- Simulate target reassignment

Simulation responses must include:

- Proposed downloads
- Proposed upgrades
- Net storage delta
- Subtitle jobs that would be triggered
- Transcode jobs that would be triggered
- Target-specific effects

Simulation must not mutate persistent state.
Simulation must execute the same comparator and validation logic as production acquisition, differing only in side-effect suppression.
Simulation runs must generate a `simulation_id` and produce decision previews structurally identical to production Decision Records, differing only in persistence and side effects.
Simulation must respect indexer capability and quarantine state exactly as production execution.
Simulation must not emit persistent events into the production event stream.
Simulation must not alter scheduler queues, backlog priorities, or rate-limit counters.
Simulation of multi-item operations must include aggregate scheduling disk impact when applicable.
Simulation must bind to a specific policy snapshot and indexer state at invocation time.

