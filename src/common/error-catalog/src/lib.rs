// Copyright (c) 2024-present, arana-db Community.  All rights reserved.
//
// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements.  See the NOTICE file distributed with
// this work for additional information regarding copyright ownership.
// The ASF licenses this file to You under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License.  You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Centralized Redis-compatible RESP error message catalog.
//!
//! This crate is the single source of truth for all client-visible error
//! strings. Do not hard-code `"ERR ..."`, `"WRONGTYPE ..."`, or similar
//! messages in other crates; import them from here instead.
//!
//! # Rules
//!
//! 1. All client-visible error text must come from this crate.
//! 2. Prefer constants for static messages; add a helper function when dynamic
//!    parameters are required.
//! 3. Before adding a new error, check whether an existing constant already
//!    covers the same semantic.
//! 4. Internal/system errors should use [`INTERNAL_SERVER_ERROR`] and log
//!    details server-side; never leak internal component names to clients.

/// Known Redis error class prefixes.
///
/// Used by [`has_error_class`] to avoid double-prefixing messages that already
/// contain a standard Redis error class.
pub const KNOWN_ERROR_CLASSES: &[&str] = &[
    "ERR ",
    "WRONGTYPE ",
    "NOAUTH ",
    "WRONGPASS ",
    "NOSCRIPT ",
    "LOADING ",
    "CLUSTERDOWN ",
    "MOVED ",
    "ASK ",
    "CROSSSLOT ",
    "TRYAGAIN ",
];

/// Returns `true` if `msg` already starts with a standard Redis error class.
pub fn has_error_class(msg: &str) -> bool {
    KNOWN_ERROR_CLASSES
        .iter()
        .any(|&prefix| msg.starts_with(prefix))
}

// -------------------- Static error messages --------------------

/// WRONGTYPE Operation against a key holding the wrong kind of value
pub const WRONGTYPE: &str = "WRONGTYPE Operation against a key holding the wrong kind of value";

pub const NOAUTH: &str = "NOAUTH Authentication required.";
pub const WRONGPASS: &str = "WRONGPASS invalid username-password pair or user is disabled.";

pub const SYNTAX_ERROR: &str = "ERR syntax error";
pub const VALUE_NOT_INTEGER: &str = "ERR value is not an integer or out of range";
pub const VALUE_NOT_VALID_FLOAT: &str = "ERR value is not a valid float";
pub const MIN_MAX_NOT_FLOAT: &str = "ERR min or max is not a float";
pub const INVALID_CURSOR: &str = "ERR invalid cursor";
pub const OFFSET_OUT_OF_RANGE: &str = "ERR offset is out of range";
pub const BIT_OFFSET_NOT_INTEGER: &str = "ERR bit offset is not an integer or out of range";
pub const INVALID_EXPIRE_TIME: &str = "ERR invalid expire time in setex";
pub const INCR_NAN_OR_INFINITY: &str = "ERR increment would produce NaN or Infinity";
pub const INTERNAL_SERVER_ERROR: &str = "ERR internal server error";
pub const COMMAND_TIMEOUT: &str = "ERR command timeout";
pub const NOT_LEADER: &str = "ERR not leader";
pub const KEY_NOT_FOUND: &str = "ERR no such key";
pub const SERVER_OVERLOADED: &str = "ERR server overloaded";
pub const EMPTY_COMMAND: &str = "ERR empty command";
pub const INVALID_COMMAND_FORMAT: &str = "ERR invalid command format";

// Auth / admin errors
pub const AUTH_NO_PASSWORD_CONFIGURED: &str = "ERR AUTH called without any password configured";
pub const AUTH_ACL_NOT_SUPPORTED: &str = "ERR ACL authentication is not supported";
pub const HELLO_AUTH_NO_PASSWORD_CONFIGURED: &str =
    "ERR HELLO AUTH called without any password configured";
pub const HELLO_AUTH_REQUIRED: &str = "NOAUTH HELLO must be called with the client already authenticated, otherwise the HELLO <proto> AUTH <user> <pass> option can be used to authenticate the client and select the RESP protocol version at the same time";
pub const CONFIG_RUNTIME_CHANGES_NOT_SUPPORTED: &str =
    "ERR runtime configuration changes not supported";

// Sorted-set aggregation errors
pub const ZSTORE_NUMKEYS_GT_ZERO: &str = "ERR numkeys should be greater than 0";
pub const ZSTORE_WEIGHT_NOT_FLOAT: &str = "ERR weight value is not a float";

// Generic argument errors
pub const VALUE_OUT_OF_RANGE_MUST_BE_POSITIVE: &str = "ERR value is out of range, must be positive";

// Storage-layer internal diagnostics. These are used in log messages and
// error traces; they are sanitized to [`INTERNAL_SERVER_ERROR`] before being
// sent to clients by `storage::error::Error::to_resp_error`.
pub const DB_NOT_INITIALIZED: &str = "db is not initialized";
pub const DATABASE_NOT_INITIALIZED: &str = "Database is not initialized";
pub const CF_NOT_INITIALIZED: &str = "cf is not initialized";
pub const CF_DATA_NOT_INITIALIZED: &str = "cf data is not initialized";
pub const CF_SCORE_NOT_INITIALIZED: &str = "cf score is not initialized";
pub const META_CF_NOT_INITIALIZED: &str = "MetaCF is not initialized";

// Storage-specific Redis semantic errors
pub const HASH_SIZE_OVERFLOW: &str = "ERR hash size overflow";
pub const HASH_SIZE_UNDERFLOW: &str = "ERR hash size underflow";
pub const HASH_VALUE_NOT_INTEGER: &str = "ERR hash value is not an integer";
pub const HASH_VALUE_NOT_VALID_FLOAT: &str = "ERR hash value is not a valid float";
pub const INVALID_SCORE_FORMAT: &str = "ERR invalid score format";
pub const ZSET_SIZE_OVERFLOW: &str = "ERR zset size overflow";
pub const STRING_EXCEEDS_MAX_SIZE: &str = "ERR string exceeds maximum allowed size";
pub const INVALID_EXPIRE_TIME_PSETEX: &str = "ERR invalid expire time in psetex";
pub const INVALID_EXPIRE_TIME_PEXPIRE: &str = "ERR invalid expire time in pexpire";
pub const BIT_IS_NOT_INTEGER: &str = "ERR bit is not an integer or out of range";
pub const BIT_MUST_BE_1_OR_0: &str = "ERR The bit argument must be 1 or 0";
pub const VALUE_IS_NOT_VALID_FLOAT: &str = "ERR value is not a valid float";
pub const INCREMENT_DECREMENT_WOULD_OVERFLOW: &str = "ERR increment or decrement would overflow";
pub const FAILED_TO_READ_ETIME: &str = "ERR Failed to read etime";
pub const FAILED_TO_READ_COUNT: &str = "ERR Failed to read count";
pub const FAILED_TO_LOCK_SCAN_CURSORS_STORE: &str = "ERR Failed to lock scan_cursors_store";
pub const CROSSSLOT: &str = "CROSSSLOT Keys in request don't hash to the same slot";
pub const BITOP_NOT_SINGLE_SOURCE: &str = "ERR BITOP NOT must be called with a single source key";
pub const MIN_MAX_NOT_VALID_STRING_RANGE_ITEM: &str = "ERR min or max not valid string range item";

// -------------------- Dynamic error helpers --------------------

/// `ERR wrong number of arguments for '<cmd>' command`
pub fn wrong_number(cmd: impl AsRef<str>) -> String {
    format!(
        "ERR wrong number of arguments for '{}' command",
        cmd.as_ref()
    )
}

/// `ERR unknown command '<cmd> <sub>'`
pub fn unknown_command(cmd: impl AsRef<str>, sub: impl AsRef<str>) -> String {
    format!("ERR unknown command '{} {}'", cmd.as_ref(), sub.as_ref())
}

/// `ERR unknown command '<cmd>'`
pub fn unknown_command_name(cmd: impl AsRef<str>) -> String {
    format!("ERR unknown command '{}'", cmd.as_ref())
}

/// `ERR unknown CONFIG subcommand '<sub>'`
pub fn unknown_config_subcommand(sub: impl AsRef<str>) -> String {
    format!("ERR unknown CONFIG subcommand '{}'", sub.as_ref())
}

/// Generic wrong-number message used when the command name is not available.
pub const WRONG_NUMBER_GENERIC: &str = "ERR wrong number of arguments for command";

/// Ensures `msg` starts with a Redis error class, prefixing `ERR ` when it
/// doesn't. Used by low-level error mapping that receives messages whose
/// provenance may or may not already be catalog constants.
pub fn ensure_err_prefix(msg: impl AsRef<str>) -> String {
    let msg = msg.as_ref();
    if has_error_class(msg) {
        msg.to_string()
    } else {
        format!("ERR {msg}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_error_class() {
        assert!(has_error_class("ERR syntax error"));
        assert!(has_error_class("WRONGTYPE Operation against a key"));
        assert!(has_error_class("NOAUTH Authentication required."));
        assert!(!has_error_class("value is not an integer"));
    }

    #[test]
    fn test_wrong_number() {
        assert_eq!(
            wrong_number("GET"),
            "ERR wrong number of arguments for 'GET' command"
        );
    }
}
