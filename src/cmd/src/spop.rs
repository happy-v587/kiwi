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
pub struct SpopCmd {
    meta: CmdMeta,
}

impl SpopCmd {
    pub fn new() -> Self {
        Self {
            meta: CmdMeta {
                name: "spop".to_string(),
                arity: -2, // SPOP key [count]
                flags: CmdFlags::WRITE | CmdFlags::FAST,
                acl_category: AclCategory::SET | AclCategory::WRITE,
                ..Default::default()
            },
        }
    }
}

impl Cmd for SpopCmd {
    impl_cmd_meta!();
    impl_cmd_clone_box!();

    fn do_initial(&self, client: &Client) -> bool {
        let argv = client.argv();
        let key = argv[1].clone();
        client.set_key(&key);
        true
    }

    fn do_cmd(&self, client: &Client, storage: Arc<Storage>) {
        let key = client.key();
        let argv = client.argv();

        // Validate argument count (should be 2 or 3)
        if argv.len() > 3 {
            client.set_error(error_catalog::wrong_number(String::from_utf8_lossy(
                client.cmd_name().as_slice(),
            )));
            return;
        }

        // Parse optional count parameter
        let count = if argv.len() > 2 {
            match String::from_utf8_lossy(&argv[2]).parse::<i32>() {
                Ok(c) => {
                    if c < 0 {
                        client.set_error(error_catalog::VALUE_OUT_OF_RANGE_MUST_BE_POSITIVE);
                        return;
                    }
                    Some(c)
                }
                Err(_) => {
                    client.set_error(error_catalog::VALUE_NOT_INTEGER);
                    return;
                }
            }
        } else {
            None
        };

        let result = storage.spop(&key, count);

        match result {
            Ok(members) => {
                if count.is_none() {
                    // Single member case - return as bulk string or nil
                    if members.is_empty() {
                        client.set_reply(RespData::BulkString(None));
                    } else {
                        client.set_reply(RespData::BulkString(Some(
                            members[0].clone().into_bytes().into(),
                        )));
                    }
                } else {
                    // Multiple members case - return as array
                    let resp_members: Vec<RespData> = members
                        .into_iter()
                        .map(|member| RespData::BulkString(Some(member.into_bytes().into())))
                        .collect();
                    client.set_reply(RespData::Array(Some(resp_members)));
                }
            }
            Err(e) => {
                client.set_storage_error(&e);
            }
        }
    }

    fn execute_typed(&self, client: &Client, storage: Arc<Storage>) -> Option<CommandResult> {
        let argv = client.argv();
        Some(match argv.len() {
            2 => storage
                .spop(&argv[1], None)
                .map(|members| RespData::BulkString(members.into_iter().next().map(Bytes::from)))
                .map_err(crate::error::CommandError::storage),
            3 => match String::from_utf8_lossy(&argv[2]).parse::<i32>() {
                Ok(count) if count >= 0 => storage
                    .spop(&argv[1], Some(count))
                    .map(|members| {
                        RespData::Array(Some(
                            members
                                .into_iter()
                                .map(|member| RespData::BulkString(Some(Bytes::from(member))))
                                .collect(),
                        ))
                    })
                    .map_err(crate::error::CommandError::storage),
                Ok(_) => Err(crate::error::CommandError::InvalidArgument(
                    crate::error::ArgumentError::OutOfRange,
                )),
                Err(_) => Err(crate::error::CommandError::InvalidArgument(
                    crate::error::ArgumentError::NotInteger,
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
    fn test_spop_cmd_meta() {
        let cmd = SpopCmd::new();
        assert_eq!(cmd.name(), "spop");
        assert_eq!(cmd.meta().arity, -2); // SPOP key [count]
        assert!(cmd.has_flag(CmdFlags::WRITE));
        assert!(cmd.has_flag(CmdFlags::FAST));
        assert!(!cmd.has_flag(CmdFlags::READONLY));
    }

    #[test]
    fn test_spop_cmd_clone() {
        let cmd = SpopCmd::new();
        let cloned = cmd.clone_box();
        assert_eq!(cloned.name(), cmd.name());
        assert_eq!(cloned.meta().arity, cmd.meta().arity);
    }

    #[test]
    fn test_spop_acl_category() {
        let cmd = SpopCmd::new();
        assert!(cmd.acl_category().contains(AclCategory::SET));
        assert!(cmd.acl_category().contains(AclCategory::WRITE));
    }

    #[test]
    fn test_spop_argument_validation() {
        let cmd = SpopCmd::new();

        // Valid argument counts (arity -2 means 2 or more)
        assert!(cmd.check_arg(2)); // SPOP key
        assert!(cmd.check_arg(3)); // SPOP key count
        assert!(cmd.check_arg(4)); // More args allowed by arity, but handled in do_cmd

        // Invalid argument counts
        assert!(!cmd.check_arg(1)); // Missing key
        assert!(!cmd.check_arg(0)); // No arguments
    }
}
