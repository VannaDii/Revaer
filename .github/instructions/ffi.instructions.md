---
applyTo:
  - "crates/revaer-torrent-libt/**"
---

`AGENTS.md` and `rust.instructions.md` apply first. This file specializes the FFI boundary.

# FFI Boundary Rules

- Unsafe code is confined to the FFI boundary modules, build scripts, and native shims only.
- `just lint` mechanically enforces that authored `unsafe` stays inside `crates/revaer-torrent-libt/src/ffi.rs` and `crates/revaer-torrent-libt/src/ffi/**`.
- The public surface exposed to the rest of the workspace must be safe Rust wrappers and translated domain types.
- Document safety invariants and failure translation at the boundary.

# `catch_unwind`

- `catch_unwind` is permitted only at the explicit FFI boundary where it prevents a Rust unwind from crossing a foreign ABI boundary.
- Allowed uses must satisfy all of the following:
  - the call site is immediately adjacent to the ABI boundary
  - the reason is documented in code comments or module docs
  - the panic is translated into a deterministic error or boundary-safe failure contract
  - the path is covered by tests
- `catch_unwind` is never acceptable as ordinary control flow, generic recovery, or a substitute for explicit error handling in normal Rust code.

# Native Shim Rules

- Keep C++ exception translation narrow and explicit where possible.
- Avoid blanket `catch (...)` handlers unless the ABI or toolchain truly requires one and the reason is documented and tested.
- Do not leak foreign exceptions or panic behavior into the rest of the Rust workspace.
