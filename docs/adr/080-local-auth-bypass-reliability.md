# 080 - Local Auth Bypass Reliability (Task Record)

- Status: Accepted
- Date: 2026-01-11

## Motivation
- Local-network auth bypass should remain usable during UI startup and on common LAN hostnames.
- Prevent UI crashes from invalid attribute names in component props.

## Design Notes
- Expand local host detection to cover loopback/private/link-local IPs plus common LAN hostnames.
- Allow anonymous prompt options on local hosts even when auth mode is not yet known; auto-enable anonymous only once the backend reports no-auth.
- Replace raw-identifier button prop names to avoid invalid DOM attributes in Yew.

## Decision
- Update local host detection and IPv6 base URL formatting in UI preferences.
- Adjust auth bypass gating to keep anonymous mode stable and prompt-friendly on local hosts.
- Rename button props from `r#type` to `button_type` in shared components.

## Consequences
- More reliable local auth bypass and fewer startup dead-ends.
- Anonymous option may appear on local hosts before auth mode is confirmed.

## Test Coverage Summary
- UI behavior validated by existing integration flows; no new automated tests added.

## Observability Updates
- None.

## Risk & Rollback
- Risk: local anonymous option could be offered briefly when auth mode still resolves.
- Rollback: revert local host detection and auth bypass gating changes.

## Dependency Rationale
- No new dependencies; uses `std` IP parsing only.
