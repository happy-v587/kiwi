# Error Handling Unification Plan

## Background

Issue [#315](https://github.com/arana-db/kiwi/issues/315) identifies that the codebase currently scatters client-facing error strings across multiple layers:

- `RespData::Error(...)` is constructed ~258 times in `src/cmd/src/` alone.
- Error prefixes are inconsistent: `format!("ERR {e}")`, `"ERR syntax error"`, `message.clone()`, `format!("{}", err_msg)`, etc.
- Storage-layer `RedisErr` messages sometimes already include `ERR ` / `WRONGTYPE ` prefixes and sometimes do not, causing double-prefix bugs like `ERR ERR invalid expire time in setex`.
- Cross-runtime errors expose internal details (`STORAGE`, `CHANNEL`, `CIRCUIT_BREAKER`, `ISOLATION`, ...) directly to clients.
- The `CmdRes` enum and `RespEncode::set_res` method in `src/resp/src/encode.rs` are implemented but have zero call sites.

## Goals

1. **Single source of truth** for all client-visible error text: a dedicated `error-catalog` crate.
2. **No scattered hard-coded error strings** in `cmd`, `storage`, `net`, or `runtime` crates.
3. **Storage errors self-convert** to RESP-safe text via `storage::error::Error::to_resp_error()`.
4. **Command layer only forwards** storage errors through `client.set_storage_error(&e)`.
5. **Runtime/network errors are sanitized** before reaching the client.
6. **Dead code removed**: `CmdRes`, `set_res`, `TryFrom<i8> for CmdRes`, and the unused `RespEncoder.res` field.
7. **CI enforces** the above via an automated `error-catalog` compliance check.
8. **CLAUDE.md documents** the convention so future commands follow it by default.

## Design Principles

- **Client-visible error text lives in one file only**: `src/common/error-catalog/src/lib.rs`.
- **Static errors are constants**; dynamic errors are formatting functions in the same catalog.
- **Prefix logic is centralized**: `to_resp_error()` and `has_error_class()` guarantee no double `ERR` / `WRONGTYPE` prefixing.
- **Internal failures are opaque**: clients see `ERR internal server error` or `ERR command timeout`; details go to structured logs.
- **Incremental migration is allowed**: the CI script uses an `ALLOWLIST` so existing files can be migrated in small PRs without breaking the build.

## Proposed Architecture

```text
┌─────────────────────────────────────┐
│         error-catalog crate         │
│  (constants + formatting helpers)   │
└──────────────┬──────────────────────┘
               │
    ┌──────────┼──────────┐
    ▼          ▼          ▼
 storage      cmd        net/runtime
  │            │            │
  │  Error::to_resp_error() │
  │            │            │
  └──────► RespData::Error ◄┘
```

## File-Level Change List

### New files

- `src/common/error-catalog/Cargo.toml`
- `src/common/error-catalog/src/lib.rs`
- `scripts/check-error-catalog.py`

### Modified files

- `Cargo.toml` — add workspace member and dependency alias.
- `Makefile` — add `error-catalog-check` target.
- `.github/workflows/ci.yml` — add `error-catalog-check` job.
- `CLAUDE.md` — add `Error Handling` section and update `Adding a Redis Command` checklist.
- `src/resp/src/encode.rs` — remove `CmdRes`, `set_res`, `TryFrom<i8>`, `RespEncoder.res`.
- `src/resp/src/lib.rs` — drop `CmdRes` re-export.
- `src/storage/Cargo.toml` — add `error-catalog` dependency.
- `src/storage/src/error.rs` — implement `to_resp_error()`; import catalog helpers.
- `src/storage/src/redis.rs` — use `error_catalog::WRONGTYPE`.
- `src/storage/src/redis_strings.rs` — use catalog constants.
- `src/storage/src/redis_hashes.rs` — use catalog constants.
- `src/storage/src/redis_lists.rs` — use catalog constants.
- `src/storage/src/redis_sets.rs` — use catalog constants.
- `src/storage/src/redis_zsets.rs` — use catalog constants.
- `src/cmd/Cargo.toml` — add `error-catalog` dependency.
- `src/cmd/src/lib.rs` — add `reply_wrong_number` helper; use catalog for arity/unknown-command errors.
- `src/cmd/src/*.rs` — replace `format!("ERR {e}")` and raw literals with catalog constants / `client.set_storage_error(&e)`.
- `src/client/src/lib.rs` — add `Client::set_storage_error()` helper.
- `src/net/Cargo.toml` — add `error-catalog` dependency.
- `src/net/src/network_handle.rs` — unify/sanitize `generate_storage_error_response`.
- `src/net/src/executor_ext.rs` — replace `format_storage_error` with shared catalog-based mapping.

## PR / Commit Breakdown

To keep reviews small and bisectable, the work is split into four commits on the same branch:

### Commit 1: Infrastructure — error-catalog, docs, CI

- Create `error-catalog` crate.
- Update root `Cargo.toml`.
- Add `scripts/check-error-catalog.py`.
- Add `Makefile` target.
- Add CI job.
- Update `CLAUDE.md`.

**Verification:** `make error-catalog-check` passes (ALLOWLIST initially contains all unmigrated files).

### Commit 2: Storage layer — catalog + self-conversion

- Add `Error::to_resp_error()` in `src/storage/src/error.rs`.
- Migrate `storage` construction points to catalog constants.
- Add `Client::set_storage_error()` helper.

**Verification:** `cargo check -p storage` passes; storage unit tests pass.

### Commit 3: Command layer — forward instead of format

- Replace `format!("ERR {e}")` / `message.clone()` with `client.set_storage_error(&e)`.
- Replace static literals with catalog constants.
- Update `BaseCmdGroup` arity/unknown-command replies.

**Verification:** `cargo check -p cmd`; `make test`.

### Commit 4: Net/runtime cleanup + dead code removal

- Sanitize `DualRuntimeError` mapping in `net`.
- Remove `CmdRes` / `set_res` / `TryFrom<i8>` / `RespEncoder.res`.
- Shrink `ALLOWLIST` to empty.

**Verification:** `make fmt && make lint && make test && make error-catalog-check`.

## Verification Steps

1. `make fmt-check`
2. `make lint`
3. `make error-catalog-check`
4. `make test`
5. `make -C tests test-python` (if server integration is affected)

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Large blast radius across 250+ call sites | Split into commits; each commit passes `cargo check` and targeted tests. |
| Accidentally changing client-visible error text | Add unit tests asserting exact `-ERR ...` / `-WRONGTYPE ...` wire output for representative commands. |
| CI script false positives | Use `ALLOWLIST` for legitimate exceptions; review regexes against real codebase. |
| Runtime `Storage` variant still wraps `String` | For now map all runtime variants to catalog constants; future refactor can preserve typed `storage::error::Error`. |

## Success Criteria

- `make error-catalog-check` passes with an empty `ALLOWLIST`.
- No `CmdRes`, `set_res`, `format!("ERR {e}")`, or raw `"ERR ..."` literals remain outside `error-catalog`.
- Client integration tests still see expected Redis-compatible error replies.
- CLAUDE.md contains an `Error Handling` section and a checklist for new commands.
