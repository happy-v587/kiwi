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

use resp::HelloError;
use thiserror::Error;

/// Errors whose meaning is defined by Redis command semantics.
///
/// This type deliberately contains no wire-level RESP text. The network layer
/// owns the mapping from these variants to client-visible replies.
#[derive(Debug, Error)]
pub enum CommandError {
    #[error("wrong key type")]
    WrongType,

    #[error("wrong number of arguments for {command}")]
    WrongArity { command: String },

    #[error("unknown subcommand {command} {subcommand}")]
    UnknownSubcommand { command: String, subcommand: String },

    #[error("keys do not share a storage slot")]
    CrossSlot,

    #[error("invalid command argument")]
    InvalidArgument(ArgumentError),

    #[error("numeric command failed")]
    Numeric(NumericError),

    #[error("authentication failed")]
    Authentication(AuthenticationError),

    #[error(transparent)]
    Hello(#[from] HelloError),

    #[error("storage command failure")]
    Storage(#[source] Box<storage::error::Error>),

    #[error("internal command failure")]
    Internal,
}

impl CommandError {
    /// Preserve storage details for server-side logging while keeping expected
    /// wrong-type failures as a Redis command semantic.
    pub fn storage(error: storage::error::Error) -> Self {
        use storage::error::Error as StorageError;

        if error.is_wrong_type() {
            return Self::WrongType;
        }

        if let StorageError::InvalidArgument { message, .. } = &error {
            if message == error_catalog::CROSSSLOT {
                return Self::CrossSlot;
            }
        }

        if let StorageError::RedisErr { message, .. } = &error {
            return match message.as_str() {
                error_catalog::VALUE_NOT_INTEGER => {
                    Self::InvalidArgument(ArgumentError::NotInteger)
                }
                error_catalog::VALUE_NOT_VALID_FLOAT => {
                    Self::InvalidArgument(ArgumentError::NotFloat)
                }
                error_catalog::HASH_VALUE_NOT_INTEGER => {
                    Self::InvalidArgument(ArgumentError::HashValueNotInteger)
                }
                error_catalog::HASH_VALUE_NOT_VALID_FLOAT => {
                    Self::InvalidArgument(ArgumentError::HashValueNotFloat)
                }
                error_catalog::INCREMENT_DECREMENT_WOULD_OVERFLOW => {
                    Self::Numeric(NumericError::Overflow)
                }
                error_catalog::INCR_NAN_OR_INFINITY => Self::Numeric(NumericError::NaNOrInfinity),
                _ => Self::Storage(Box::new(error)),
            };
        }

        Self::Storage(Box::new(error))
    }
}

/// Reusable categories for invalid command arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentError {
    Syntax,
    NotInteger,
    NotFloat,
    HashValueNotInteger,
    HashValueNotFloat,
    OutOfRange,
    OffsetOutOfRange,
    InvalidExpireTime,
    InvalidPexpireTime,
    InvalidCursor,
}

/// Reusable categories for numeric operations whose operands were valid but
/// the requested calculation cannot be represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericError {
    Overflow,
    NaNOrInfinity,
}

/// Reusable categories for authentication failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticationError {
    Required,
    WrongPassword,
    PasswordNotConfigured,
    AclNotSupported,
}

#[cfg(test)]
mod tests {
    use super::{ArgumentError, AuthenticationError, CommandError};

    #[test]
    fn command_error_keeps_redis_semantics_without_reply_text() {
        assert_eq!(
            CommandError::WrongArity {
                command: "get".to_string(),
            }
            .to_string(),
            "wrong number of arguments for get"
        );
        assert_eq!(CommandError::WrongType.to_string(), "wrong key type");
        assert_eq!(
            CommandError::InvalidArgument(ArgumentError::Syntax).to_string(),
            "invalid command argument"
        );
        assert_eq!(
            CommandError::Authentication(AuthenticationError::Required).to_string(),
            "authentication failed"
        );
        assert_eq!(
            CommandError::Internal.to_string(),
            "internal command failure"
        );
    }
}
