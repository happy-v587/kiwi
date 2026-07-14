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

use cmd::error::{ArgumentError, AuthenticationError, CommandError, NumericError};
use resp::{HelloError, RespData};
use runtime::ExecutionError;

/// Converts typed request failures to Redis-compatible RESP error replies.
///
/// This is the network boundary: command code chooses a semantic variant and
/// never assembles the final protocol error text itself.
pub struct RedisErrorRenderer;

impl RedisErrorRenderer {
    pub fn render_command(error: &CommandError) -> RespData {
        RespData::error(Self::command_message(error))
    }

    /// Render dispatch failures without exposing runtime implementation details.
    pub fn render_execution(error: &ExecutionError) -> RespData {
        match error {
            ExecutionError::Command(error) => Self::render_command(error),
            ExecutionError::Timeout { .. } => RespData::error(error_catalog::COMMAND_TIMEOUT),
            ExecutionError::Unavailable { .. }
            | ExecutionError::Overloaded
            | ExecutionError::ChannelClosed
            | ExecutionError::ShuttingDown
            | ExecutionError::WorkerStopped => {
                RespData::error(error_catalog::INTERNAL_SERVER_ERROR)
            }
        }
    }

    fn command_message(error: &CommandError) -> String {
        match error {
            CommandError::WrongType => error_catalog::WRONGTYPE.to_string(),
            CommandError::WrongArity { command } => error_catalog::wrong_number(command),
            CommandError::UnknownSubcommand {
                command,
                subcommand,
            } => error_catalog::unknown_command(command, subcommand),
            CommandError::CrossSlot => error_catalog::CROSSSLOT.to_string(),
            CommandError::InvalidArgument(ArgumentError::Syntax) => {
                error_catalog::SYNTAX_ERROR.to_string()
            }
            CommandError::InvalidArgument(ArgumentError::NotInteger) => {
                error_catalog::VALUE_NOT_INTEGER.to_string()
            }
            CommandError::InvalidArgument(ArgumentError::NotFloat) => {
                error_catalog::VALUE_NOT_VALID_FLOAT.to_string()
            }
            CommandError::InvalidArgument(ArgumentError::HashValueNotInteger) => {
                error_catalog::HASH_VALUE_NOT_INTEGER.to_string()
            }
            CommandError::InvalidArgument(ArgumentError::HashValueNotFloat) => {
                error_catalog::HASH_VALUE_NOT_VALID_FLOAT.to_string()
            }
            CommandError::InvalidArgument(ArgumentError::InvalidScoreRange) => {
                error_catalog::MIN_MAX_NOT_FLOAT.to_string()
            }
            CommandError::InvalidArgument(ArgumentError::OutOfRange) => {
                error_catalog::VALUE_OUT_OF_RANGE_MUST_BE_POSITIVE.to_string()
            }
            CommandError::InvalidArgument(ArgumentError::OffsetOutOfRange) => {
                error_catalog::OFFSET_OUT_OF_RANGE.to_string()
            }
            CommandError::InvalidArgument(ArgumentError::InvalidExpireTime) => {
                error_catalog::INVALID_EXPIRE_TIME.to_string()
            }
            CommandError::InvalidArgument(ArgumentError::InvalidPexpireTime) => {
                error_catalog::INVALID_EXPIRE_TIME_PSETEX.to_string()
            }
            CommandError::InvalidArgument(ArgumentError::InvalidCursor) => {
                error_catalog::INVALID_CURSOR.to_string()
            }
            CommandError::Numeric(NumericError::Overflow) => {
                error_catalog::INCREMENT_DECREMENT_WOULD_OVERFLOW.to_string()
            }
            CommandError::Numeric(NumericError::NaNOrInfinity) => {
                error_catalog::INCR_NAN_OR_INFINITY.to_string()
            }
            CommandError::Authentication(AuthenticationError::Required) => {
                error_catalog::NOAUTH.to_string()
            }
            CommandError::Authentication(AuthenticationError::WrongPassword) => {
                error_catalog::WRONGPASS.to_string()
            }
            CommandError::Authentication(AuthenticationError::PasswordNotConfigured) => {
                error_catalog::AUTH_NO_PASSWORD_CONFIGURED.to_string()
            }
            CommandError::Authentication(AuthenticationError::AclNotSupported) => {
                error_catalog::AUTH_ACL_NOT_SUPPORTED.to_string()
            }
            CommandError::Hello(HelloError::InvalidArgument(message)) => {
                error_catalog::ensure_err_prefix(message)
            }
            CommandError::Hello(HelloError::WrongPassword) => error_catalog::WRONGPASS.to_string(),
            CommandError::Hello(HelloError::NoPasswordConfigured) => {
                error_catalog::HELLO_AUTH_NO_PASSWORD_CONFIGURED.to_string()
            }
            CommandError::Hello(HelloError::AuthenticationRequired) => {
                error_catalog::HELLO_AUTH_REQUIRED.to_string()
            }
            CommandError::Storage(_) => error_catalog::INTERNAL_SERVER_ERROR.to_string(),
            CommandError::Internal => error_catalog::INTERNAL_SERVER_ERROR.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use cmd::error::{ArgumentError, AuthenticationError, CommandError, NumericError};
    use resp::{HelloError, RespEncode, RespVersion, encode::RespEncoder};
    use runtime::ExecutionError;

    use super::RedisErrorRenderer;

    fn render(error: CommandError) -> bytes::Bytes {
        let mut encoder = RespEncoder::new(RespVersion::RESP2);
        encoder.encode_resp_data(&RedisErrorRenderer::render_command(&error));
        encoder.get_response()
    }

    #[test]
    fn renders_representative_command_errors_as_compatible_resp_bytes() {
        assert_eq!(
            render(CommandError::WrongArity {
                command: "get".to_string(),
            }),
            b"-ERR wrong number of arguments for 'get' command\r\n".as_slice()
        );
        assert_eq!(
            render(CommandError::WrongType),
            b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n".as_slice()
        );
        assert_eq!(
            render(CommandError::InvalidArgument(ArgumentError::Syntax)),
            b"-ERR syntax error\r\n".as_slice()
        );
        assert_eq!(
            render(CommandError::Numeric(NumericError::Overflow)),
            b"-ERR increment or decrement would overflow\r\n".as_slice()
        );
        assert_eq!(
            render(CommandError::Authentication(AuthenticationError::Required)),
            b"-NOAUTH Authentication required.\r\n".as_slice()
        );
        assert_eq!(
            render(CommandError::Authentication(
                AuthenticationError::AclNotSupported
            )),
            b"-ERR ACL authentication is not supported\r\n".as_slice()
        );
        assert_eq!(
            render(CommandError::Internal),
            b"-ERR internal server error\r\n".as_slice()
        );
        assert_eq!(
            render(CommandError::storage(
                storage::error::Error::InvalidFormat {
                    message: "persisted detail must not reach clients".to_string(),
                    location: snafu::Location::new(file!(), line!(), column!()),
                },
            )),
            b"-ERR internal server error\r\n".as_slice()
        );
    }

    #[test]
    fn renders_hello_command_errors_as_compatible_resp_bytes() {
        assert_eq!(
            render(CommandError::Hello(HelloError::WrongPassword)),
            b"-WRONGPASS invalid username-password pair or user is disabled.\r\n".as_slice()
        );
        assert_eq!(
            render(CommandError::Hello(HelloError::NoPasswordConfigured)),
            b"-ERR HELLO AUTH called without any password configured\r\n".as_slice()
        );
        assert_eq!(
            render(CommandError::Hello(HelloError::AuthenticationRequired)),
            b"-NOAUTH HELLO must be called with the client already authenticated, otherwise the HELLO <proto> AUTH <user> <pass> option can be used to authenticate the client and select the RESP protocol version at the same time\r\n".as_slice()
        );
    }

    #[test]
    fn renders_execution_errors_as_sanitized_resp_bytes() {
        assert_eq!(
            render_execution(ExecutionError::Timeout {
                timeout: std::time::Duration::from_secs(1),
            }),
            b"-ERR command timeout\r\n".as_slice()
        );
        assert_eq!(
            render_execution(ExecutionError::ChannelClosed),
            b"-ERR internal server error\r\n".as_slice()
        );
    }

    fn render_execution(error: ExecutionError) -> bytes::Bytes {
        let mut encoder = RespEncoder::new(RespVersion::RESP2);
        encoder.encode_resp_data(&RedisErrorRenderer::render_execution(&error));
        encoder.get_response()
    }
}
