# 025: Seed mode admission with optional hash sampling

> Allow seed-mode admissions without full rechecks while optionally sampling hashes to guard against corrupt data.

## Status

Accepted

## Context

Users need to add torrents as already complete (seed mode) without forcing a full recheck, but we still need a safety valve to avoid seeding corrupted data. The API already exposes per-torrent knobs; we must thread seed-mode through the worker/FFI/native layers and optionally sample hashes before honouring the flag. Seed-mode should only be allowed when metainfo is present to avoid undefined behaviour on magnet-only adds.

## Decision

- Add `seed_mode` and `hash_check_sample_pct` to `AddTorrentOptions`/`TorrentCreateRequest`. Validation requires `seed_mode=true` when sampling is requested and rejects seed-mode requests without metainfo (API prefers metainfo when seed-mode/sampling is set).
- Worker forwards the flags, warns when seed-mode is requested without sampling, persists the intent in fast-resume metadata, and skips sampling when only a magnet was supplied.
- The native bridge sets `lt::torrent_flags::seed_mode` on admission when requested. When a hash sample percentage is provided, it uses libtorrent to hash an even spread of pieces from the requested save path and aborts admission on missing files or hash mismatches. Sampling uses only libtorrent/stdlib (no new dependencies).
- Stub/native tests cover seed-mode success, metadata persistence, magnet rejection, and hash-sample failure paths.

## Consequences

- Seed-mode is explicit opt-in and limited to metainfo submissions; magnet-only requests fail fast to avoid silent misbehaviour.
- Hash sampling is best-effort and can fail admission if files are missing or corrupted; callers can opt out by omitting the sample percentage (a warning is logged).
- Fast-resume metadata now tracks seed-mode and sampling preferences for future reconciliation.
