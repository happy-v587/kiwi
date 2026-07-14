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

use std::sync::Arc;

use bytes::Bytes;
use client::Client;
use resp::RespData;
use storage::storage::Storage;

use crate::{AclCategory, ClientExt, Cmd, CmdFlags, CmdMeta, CommandResult};
use crate::{impl_cmd_clone_box, impl_cmd_meta};

#[derive(Clone, Default)]
pub struct ZincrbyCmd {
    meta: CmdMeta,
}

impl ZincrbyCmd {
    pub fn new() -> Self {
        Self {
            meta: CmdMeta {
                name: "zincrby".to_string(),
                arity: 4, // ZINCRBY key increment member
                flags: CmdFlags::WRITE,
                acl_category: AclCategory::WRITE | AclCategory::SORTEDSET,
                ..Default::default()
            },
        }
    }
}

impl Cmd for ZincrbyCmd {
    impl_cmd_meta!();
    impl_cmd_clone_box!();

    /// ZINCRBY key increment member
    fn do_initial(&self, client: &Client) -> bool {
        let argv = client.argv();

        // Validate argument count (must be exactly 4: command + key + increment + member)
        if argv.len() != 4 {
            client.set_error(error_catalog::wrong_number("zincrby"));
            return false;
        }

        let key = argv[1].clone();
        client.set_key(&key);

        true
    }

    fn do_cmd(&self, client: &Client, storage: Arc<Storage>) {
        let argv = client.argv();
        let key = client.key();

        // Parse increment
        let increment_str = String::from_utf8_lossy(&argv[2]);
        let increment = match increment_str.parse::<f64>() {
            Ok(inc) => {
                // Check for valid float (not NaN or infinite)
                if inc.is_nan() || inc.is_infinite() {
                    client.set_error(error_catalog::VALUE_IS_NOT_VALID_FLOAT);
                    return;
                }
                inc
            }
            Err(_) => {
                client.set_error(error_catalog::VALUE_IS_NOT_VALID_FLOAT);
                return;
            }
        };

        // Get member
        let member = &argv[3];

        // Execute ZINCRBY
        let result = storage.zincrby(&key, increment, member);

        match result {
            Ok(new_score_bytes) => {
                client.set_reply(RespData::BulkString(Some(new_score_bytes.into())));
            }
            Err(e) => {
                client.set_storage_error(&e);
            }
        }
    }

    fn execute_typed(&self, client: &Client, storage: Arc<Storage>) -> Option<CommandResult> {
        let argv = client.argv();
        Some(match argv.len() {
            4 => match String::from_utf8_lossy(&argv[2]).parse::<f64>() {
                Ok(increment) if increment.is_finite() => storage
                    .zincrby(&argv[1], increment, &argv[3])
                    .map(|score| RespData::BulkString(Some(Bytes::from(score))))
                    .map_err(crate::error::CommandError::storage),
                _ => Err(crate::error::CommandError::InvalidArgument(
                    crate::error::ArgumentError::NotFloat,
                )),
            },
            _ => Err(crate::error::CommandError::WrongArity {
                command: self.name().to_string(),
            }),
        })
    }

    fn uses_typed_execution(&self) -> bool {
        true
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zincrby_cmd_meta() {
        let cmd = ZincrbyCmd::new();
        assert_eq!(cmd.name(), "zincrby");
        assert_eq!(cmd.meta().arity, 4);
        assert!(cmd.has_flag(CmdFlags::WRITE));
    }

    #[test]
    fn test_zincrby_cmd_check_arg() {
        let cmd = ZincrbyCmd::new();

        // Valid argument count
        assert!(cmd.check_arg(4)); // ZINCRBY key increment member

        // Invalid argument counts
        assert!(!cmd.check_arg(1)); // Too few
        assert!(!cmd.check_arg(2)); // Too few
        assert!(!cmd.check_arg(3)); // Too few
        assert!(!cmd.check_arg(5)); // Too many
    }
}
