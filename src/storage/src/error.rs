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

//! Error types for the storage engine

use std::io;

use bytes::Bytes;
use common_macro::stack_trace_debug;
use error_catalog::{INTERNAL_SERVER_ERROR, ensure_err_prefix};
use snafu::{Location, Snafu};

use crate::storage::BgTask;

pub type Result<T> = std::result::Result<T, Error>;

#[allow(dead_code)]
#[derive(Snafu)]
#[stack_trace_debug]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("IO error"))]
    Io {
        #[snafu(source)]
        error: io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("RocksDB error"))]
    Rocks {
        #[snafu(source)]
        error: rocksdb::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Mpsc error"))]
    Mpsc {
        #[snafu(source)]
        error: tokio::sync::mpsc::error::SendError<BgTask>,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Encoding error: {}", message))]
    Encoding {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Key not found: {}", key))]
    KeyNotFound {
        key: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Invalid format: {}", message))]
    InvalidFormat {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Transaction error: {}", message))]
    Transaction {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Batch operation error: {}", message))]
    Batch {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Compaction error: {}", message))]
    Compaction {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Configuration error: {}", message))]
    Config {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("System error: {}", message))]
    System {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Unknown error: {}", message))]
    Unknown {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Option is none: {}", message))]
    OptionNone {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Option is not dynamically modifiable: {}", message))]
    OptionNotDynamicallyModifiable {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Invalid argument: {}", message))]
    InvalidArgument {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    // all the redis error use this error type
    #[snafu(display("{}", message))]
    RedisErr {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("LogIndex error: {}", message))]
    LogIndex {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },
}

impl Error {
    /// Convert this storage error into a RESP-safe error string.
    ///
    /// The returned text is intended to be placed directly into
    /// `RespData::Error(...)` and sent to the client. Redis-level semantic
    /// messages that already carry a standard error class (`ERR`, `WRONGTYPE`,
    /// etc.) are passed through verbatim; other messages are prefixed with
    /// `ERR `. Internal failures are sanitized to a fixed message.
    pub fn to_resp_error(&self) -> Bytes {
        let msg = match self {
            Error::RedisErr { message, .. } => ensure_err_prefix(message),
            Error::InvalidFormat { message, .. }
            | Error::InvalidArgument { message, .. }
            | Error::Encoding { message, .. } => ensure_err_prefix(message),
            Error::KeyNotFound { .. } => error_catalog::KEY_NOT_FOUND.to_string(),
            _ => INTERNAL_SERVER_ERROR.to_string(),
        };
        msg.into()
    }
}
