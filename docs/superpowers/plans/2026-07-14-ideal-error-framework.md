# Ideal Error Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the main request-path error handling with bounded, typed parse, command, storage, execution, Raft, and startup error layers while preserving Redis client responses.

**Architecture:** Errors flow only upward: `ParseError` handles wire parsing; `StorageError` retains engine and state failures; `CommandError` represents command semantics and wraps storage/Raft failures; `ExecutionError` represents dispatch failures. `net::error_response` is the only production module that renders Redis error text.

**Tech Stack:** Rust nightly-2025-08-20, `thiserror`, Tokio, RocksDB, RESP.

## Global Constraints

- Preserve every existing client-visible Redis error text and RESP encoding.
- Preserve error sources; never stringify an error merely to classify it.
- Do not use error text for retry, severity, or recovery classification.
- Keep normal outcomes such as missing keys and conditional-write misses out of Error types.
- Each independently testable phase is committed with a Conventional Commit title.
- Run TDD red-green verification for every behavior change.
- Finish each phase with `make fmt`, targeted tests, `make error-catalog-check`, and the phase-specific build/test command.

---

### Task 1: Rename RESP parsing errors and remove non-parser variants

**Files:**
- Modify: `src/resp/src/error.rs`
- Modify: `src/resp/src/lib.rs`
- Modify: `src/resp/src/command.rs`
- Modify: `src/resp/src/parse.rs`
- Modify: `src/resp/src/negotiation.rs`
- Modify: `src/resp/tests/integration_tests.rs`

**Consumes:** Existing `RespError` and `RespResult` parser API.

**Produces:** `ParseError` and `ParseResult`; HELLO uses a dedicated error rather than a parser error for command semantics.

- [ ] Write compile-focused tests proving parser APIs use `ParseError` and malformed RESP still yields `ParseError::Incomplete` or `ParseError::InvalidData`.
- [ ] Run `cargo test -p resp` and confirm the new test fails because `ParseError` does not exist.
- [ ] Rename `RespError` to `ParseError`, rename `RespResult` to `ParseResult`, and remove command-semantic variants (`UnknownCommand`, `UnknownSubCommand`, `SyntaxError`, `WrongNumberOfArguments`, `UnknownError`).
- [ ] Update parser and RESP command conversion call sites to use the new names.
- [ ] Run `cargo test -p resp` and `make fmt`; confirm all RESP tests pass.
- [ ] Commit: `refactor(resp): rename parser error type`.

### Task 2: Introduce typed command errors and RESP rendering

**Files:**
- Create: `src/cmd/src/error.rs`
- Modify: `src/cmd/src/lib.rs`
- Modify: `src/cmd/Cargo.toml`
- Create: `src/net/src/error_response.rs`
- Modify: `src/net/src/lib.rs`
- Modify: `src/net/Cargo.toml`
- Test: `src/cmd/src/error.rs`
- Test: `src/net/src/error_response.rs`

**Consumes:** `error-catalog` values and `resp::RespData`.

**Produces:** `CommandError`, `ArgumentError`, `AuthenticationError`, and `RedisErrorRenderer` with byte-for-byte compatible responses.

- [ ] Write failing unit tests for `CommandError::WrongArity { command: "get" }`, `WrongType`, `ArgumentError::Syntax`, `AuthenticationError::Required`, and `Internal`; assert exact existing RESP error bytes.
- [ ] Run `cargo test -p cmd command_error` and `cargo test -p net error_response`; confirm the tests fail because the types/module do not exist.
- [ ] Implement the small command error enums and renderer; dynamic messages must call existing catalog helpers.
- [ ] Add `CommandError::into_resp_error()` only if it does not introduce a crate cycle; otherwise keep rendering in `net::error_response`.
- [ ] Run the new tests, `cargo test -p cmd`, `cargo test -p net`, `make fmt`, and `make error-catalog-check`.
- [ ] Commit: `feat(error): add typed command errors and renderer`.

### Task 3: Make HELLO return typed command errors

**Files:**
- Modify: `src/resp/src/negotiation.rs`
- Modify: `src/cmd/src/hello.rs`
- Modify: `src/cmd/src/error.rs`
- Modify: `src/cmd/src/table.rs`
- Modify: `src/resp/tests/integration_tests.rs`

**Consumes:** `ParseError` for wire decoding and `CommandError` for command semantics.

**Produces:** HELLO negotiation returns a dedicated typed semantic error; no business error string is transported inside a parser error.

- [ ] Write failing tests that assert HELLO wrong password, missing password configuration, and unauthenticated HELLO retain their existing replies through `CommandError` rendering.
- [ ] Run focused HELLO tests and confirm failure because negotiation still returns parser errors.
- [ ] Add `HelloError` only if HELLO needs fields not present in `CommandError`; otherwise return `CommandError` directly.
- [ ] Replace `format_hello_error()` with typed rendering and remove hard-coded client error strings from `resp::negotiation`.
- [ ] Run `cargo test -p cmd hello`, `cargo test -p resp`, `make fmt`, and `make error-catalog-check`.
- [ ] Commit: `refactor(hello): separate protocol and command errors`.

### Task 4: Replace storage error taxonomy with typed storage failures

**Files:**
- Modify: `src/storage/src/error.rs`
- Modify: every production caller of `RedisErrSnafu`, `InvalidFormatSnafu`, `InvalidArgumentSnafu`, `EncodingSnafu`, `OptionNoneSnafu`, `SystemSnafu`, and `UnknownSnafu` under `src/storage/src/`
- Modify: `src/storage/Cargo.toml`
- Modify: `src/storage/tests/*.rs`

**Consumes:** `StorageError::{Engine, Io, Corruption, InvalidState, WrongType}`.

**Produces:** Storage APIs no longer use `RedisErr(String)` or broad stringly variants; `WrongType` remains distinguishable and all internal failures retain context/source.

- [ ] Add failing unit tests for `StorageError::WrongType`, a malformed encoded value mapping to `Corruption`, and an impossible internal condition mapping to `InvalidState`.
- [ ] Run `cargo test -p storage storage_error`; confirm tests fail because the new variants and conversions do not exist.
- [ ] Replace the Snafu enum with `thiserror` variants and focused constructors that carry operation context and sources.
- [ ] Migrate every storage error construction mechanically but classify each constructor by behavior: key type mismatch → `WrongType`; persisted byte decode failure → `Corruption`; impossible program state → `InvalidState`; RocksDB/IO source → `Engine`/`Io`.
- [ ] Change storage APIs that use key absence as an error to return `Option` or a command-specific outcome where applicable.
- [ ] Remove `to_resp_error()` and all error-catalog dependencies from storage.
- [ ] Run `cargo test -p storage`, `make fmt`, and `cargo clippy -p storage -- -D warnings -D clippy::unwrap_used`.
- [ ] Commit: `refactor(storage): replace stringly error taxonomy`.

### Task 5: Return `Result<Reply, CommandError>` from commands

**Files:**
- Modify: `src/cmd/src/lib.rs`
- Modify: every command implementation under `src/cmd/src/*.rs`
- Modify: `src/client/src/lib.rs`
- Modify: `src/common/runtime/storage_server.rs`
- Modify: command tests under `src/cmd/src/` and `src/net/tests/`

**Consumes:** `CommandError` and `StorageError`.

**Produces:** Command implementations return replies/errors instead of mutating client reply state for error propagation.

- [ ] Write a failing command test using GET on a wrong-type key that expects `Err(CommandError::Storage(StorageError::WrongType { .. }))` before rendering.
- [ ] Run the focused test and confirm it fails because `Cmd::execute` returns unit and stores replies on `Client`.
- [ ] Introduce `Reply` as the command result alias or dedicated wrapper around `RespData`.
- [ ] Change `Cmd::do_initial`, `Cmd::do_cmd`, and `Cmd::execute` to return `Result<Reply, CommandError>`; preserve `Client` only for connection state and command arguments.
- [ ] Migrate commands in small compiler-driven batches; replace `client.set_error` and `set_storage_error` calls with typed `Err` values.
- [ ] Update storage-server dispatch to return command results through `StorageResponse`.
- [ ] Run `cargo test -p cmd`, `cargo test -p net storage_command_e2e`, `make fmt`, and `make lint`.
- [ ] Commit: `refactor(cmd): return typed command results`.

### Task 6: Replace `DualRuntimeError` with bounded `ExecutionError`

**Files:**
- Modify: `src/common/runtime/error.rs`
- Modify: `src/common/runtime/message.rs`
- Modify: `src/common/runtime/storage_server.rs`
- Modify: `src/common/runtime/manager.rs`
- Modify: `src/common/runtime/error_logging.rs`
- Modify: `src/common/runtime/{tests.rs,additional_unit_tests.rs,stress_tests.rs}`
- Modify: `src/net/src/{storage_client.rs,executor_ext.rs,network_handle.rs}`

**Consumes:** `CommandError` from command dispatch and typed `StorageError` sources.

**Produces:** `ExecutionError::{Command, Timeout, Unavailable, Overloaded, ChannelClosed, ShuttingDown, WorkerStopped}`; no string-based storage error classification.

- [ ] Write failing runtime tests for exact timeout, channel-closed, unavailable, and command-error variants; assert no `Storage(String)` variant exists.
- [ ] Run `cargo test -p runtime error`; confirm failures reflect the old `DualRuntimeError` API.
- [ ] Replace `DualRuntimeError` and its string constructors with `ExecutionError`; preserve source errors through nested variants.
- [ ] Remove `is_recoverable`, `is_storage_error_recoverable`, severity, and circuit-breaker decisions that classify Display strings; move retry decisions into an explicit policy that receives command metadata and execution stage.
- [ ] Make network dispatch use `RedisErrorRenderer` as the sole client-error conversion site; delete duplicate mapping functions.
- [ ] Run `cargo test -p runtime`, `cargo test -p net`, `make fmt`, and `make lint`.
- [ ] Commit: `refactor(runtime): use typed execution errors`.

### Task 7: Isolate Raft and startup errors

**Files:**
- Create or modify: `src/raft/src/error.rs`
- Modify: Raft command and transport error call sites under `src/raft/src/`
- Create: `src/server/src/error.rs`
- Modify: `src/server/src/main.rs`
- Test: `src/raft/src/error.rs`
- Test: `src/server/src/error.rs`

**Consumes:** `StorageError`, `CommandError`, and `ExecutionError`.

**Produces:** `RaftError` and `StartupError`; Raft leader semantics map to command errors, while initialization failure exits through startup errors.

- [ ] Write failing tests for Raft not-leader conversion and startup storage-open context.
- [ ] Run focused crate tests and confirm types do not yet exist.
- [ ] Add bounded `RaftError` and `StartupError` types with sources and operation context.
- [ ] Map only `RaftError::NotLeader` to the client-visible leader redirect; all other Raft failures are internal client errors with detailed server logs.
- [ ] Make `main::run` return `Result<(), StartupError>` and map only at the process boundary.
- [ ] Run `cargo test -p raft`, `cargo test -p server`, `make fmt`, and `make lint`.
- [ ] Commit: `refactor(server): separate raft and startup errors`.

### Task 8: Enforce the final boundary and audit behavior

**Files:**
- Modify: `scripts/check-error-catalog.py` or replace it with `scripts/check-redis-error-rendering.py`
- Modify: `.github/workflows/ci.yml`
- Modify: `CLAUDE.md`
- Modify: `docs/ideal-error-handling-framework-design.md`
- Modify: relevant tests in `src/net/tests/`, `src/resp/tests/`, and `src/storage/tests/`

**Consumes:** The final error layers and renderer.

**Produces:** CI rejects Redis protocol error text outside the renderer, rejected legacy APIs are absent, and compatibility tests prove client responses are unchanged.

- [ ] Add failing checks that detect `ERR`, `WRONGTYPE`, `NOAUTH`, and `WRONGPASS` literals outside the renderer and tests.
- [ ] Add snapshot-style tests for representative wrong type, syntax, arity, auth, timeout, internal, and leader responses.
- [ ] Update the script and CI to enforce the renderer-only rule and reject `RedisErr`, `DualRuntimeError`, and `to_resp_error` production references.
- [ ] Update contributor documentation with the four primary request-path error choices.
- [ ] Run `make error-catalog-check`, `make fmt-check`, `make lint`, `make build`, and `make test`.
- [ ] Commit: `test(error): enforce typed error boundaries`.

## Completion Audit

- [ ] `rg` confirms only the renderer and tests contain Redis error class literals.
- [ ] `rg` confirms no production `RedisErr`, `DualRuntimeError`, `to_resp_error`, or `set_storage_error` references remain.
- [ ] `ParseError`, `StorageError`, `CommandError`, `ExecutionError`, `RaftError`, and `StartupError` exist at the documented boundaries.
- [ ] Storage missing-key and conditional outcomes are modeled as normal values where affected APIs expose them.
- [ ] No retry/recovery code classifies errors through `to_string()` or `contains()`.
- [ ] Fresh `make fmt-check`, `make lint`, `make build`, `make test`, and CI error-boundary check all pass.
