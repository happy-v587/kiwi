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
pub struct SscanCmd {
    meta: CmdMeta,
}

impl SscanCmd {
    pub fn new() -> Self {
        Self {
            meta: CmdMeta {
                name: "sscan".to_string(),
                arity: -3, // SSCAN key cursor [MATCH pattern] [COUNT count]
                flags: CmdFlags::READONLY | CmdFlags::FAST,
                acl_category: AclCategory::SET | AclCategory::READ,
                ..Default::default()
            },
        }
    }
}

impl Cmd for SscanCmd {
    impl_cmd_meta!();
    impl_cmd_clone_box!();

    fn do_initial(&self, client: &Client) -> bool {
        let argv = client.argv();
        if argv.len() < 3 {
            return false;
        }

        // Set the key for locking
        let key = argv[1].clone();
        client.set_key(&key);
        true
    }

    fn do_cmd(&self, client: &Client, storage: Arc<Storage>) {
        let argv = client.argv();

        let key = argv[1].as_slice();
        let cursor_str = String::from_utf8_lossy(&argv[2]);

        // Parse cursor
        let cursor = match cursor_str.parse::<u64>() {
            Ok(c) => c,
            Err(_) => {
                client.set_error(error_catalog::INVALID_CURSOR);
                return;
            }
        };

        // Parse optional MATCH and COUNT parameters
        let mut pattern: Option<String> = None;
        let mut count: Option<usize> = None;

        let mut i = 3;
        while i < argv.len() {
            let arg = String::from_utf8_lossy(&argv[i]).to_uppercase();
            match arg.as_str() {
                "MATCH" if i + 1 < argv.len() => {
                    pattern = Some(String::from_utf8_lossy(&argv[i + 1]).to_string());
                    i += 2;
                }
                "MATCH" => {
                    client.set_error(error_catalog::SYNTAX_ERROR);
                    return;
                }
                "COUNT" if i + 1 < argv.len() => {
                    match String::from_utf8_lossy(&argv[i + 1]).parse::<usize>() {
                        Ok(c) if c > 0 => {
                            count = Some(c);
                            i += 2;
                        }
                        _ => {
                            client.set_error(error_catalog::VALUE_NOT_INTEGER);
                            return;
                        }
                    }
                }
                "COUNT" => {
                    client.set_error(error_catalog::SYNTAX_ERROR);
                    return;
                }
                _ => {
                    client.set_error(error_catalog::SYNTAX_ERROR);
                    return;
                }
            }
        }

        let result = storage.sscan(key, cursor, pattern.as_deref(), count);

        match result {
            Ok((next_cursor, members)) => {
                // Convert members to RespData
                let resp_members: Vec<RespData> = members
                    .into_iter()
                    .map(|member| RespData::BulkString(Some(member.into_bytes().into())))
                    .collect();

                // Return [next_cursor, [members...]]
                let response = vec![
                    RespData::BulkString(Some(next_cursor.to_string().into_bytes().into())),
                    RespData::Array(Some(resp_members)),
                ];

                client.set_reply(RespData::Array(Some(response)));
            }
            Err(e) => {
                client.set_storage_error(&e);
            }
        }
    }

    fn execute_typed(&self, client: &Client, storage: Arc<Storage>) -> Option<CommandResult> {
        let argv = client.argv();
        Some(match argv.len() {
            0..3 => Err(crate::error::CommandError::WrongArity {
                command: self.name().to_string(),
            }),
            _ => {
                let cursor = match String::from_utf8_lossy(&argv[2]).parse::<u64>() {
                    Ok(cursor) => cursor,
                    Err(_) => {
                        return Some(Err(crate::error::CommandError::InvalidArgument(
                            crate::error::ArgumentError::InvalidCursor,
                        )));
                    }
                };

                let mut pattern = None;
                let mut count = None;
                let mut index = 3;
                while index < argv.len() {
                    match String::from_utf8_lossy(&argv[index])
                        .to_uppercase()
                        .as_str()
                    {
                        "MATCH" if index + 1 < argv.len() => {
                            pattern = Some(String::from_utf8_lossy(&argv[index + 1]).to_string());
                            index += 2;
                        }
                        "COUNT" if index + 1 < argv.len() => {
                            count = match String::from_utf8_lossy(&argv[index + 1]).parse::<usize>()
                            {
                                Ok(count) if count > 0 => Some(count),
                                _ => {
                                    return Some(Err(crate::error::CommandError::InvalidArgument(
                                        crate::error::ArgumentError::NotInteger,
                                    )));
                                }
                            };
                            index += 2;
                        }
                        _ => {
                            return Some(Err(crate::error::CommandError::InvalidArgument(
                                crate::error::ArgumentError::Syntax,
                            )));
                        }
                    }
                }

                storage
                    .sscan(&argv[1], cursor, pattern.as_deref(), count)
                    .map(|(next_cursor, members)| {
                        RespData::Array(Some(vec![
                            RespData::BulkString(Some(Bytes::from(next_cursor.to_string()))),
                            RespData::Array(Some(
                                members
                                    .into_iter()
                                    .map(|member| RespData::BulkString(Some(Bytes::from(member))))
                                    .collect(),
                            )),
                        ]))
                    })
                    .map_err(crate::error::CommandError::storage)
            }
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
    fn test_sscan_cmd_meta() {
        let cmd = SscanCmd::new();
        assert_eq!(cmd.name(), "sscan");
        assert_eq!(cmd.meta().arity, -3); // SSCAN key cursor [MATCH pattern] [COUNT count]
        assert!(cmd.has_flag(CmdFlags::READONLY));
        assert!(cmd.has_flag(CmdFlags::FAST));
        assert!(!cmd.has_flag(CmdFlags::WRITE));
    }

    #[test]
    fn test_sscan_cmd_clone() {
        let cmd = SscanCmd::new();
        let cloned = cmd.clone_box();
        assert_eq!(cloned.name(), cmd.name());
        assert_eq!(cloned.meta().arity, cmd.meta().arity);
    }

    #[test]
    fn test_sscan_acl_category() {
        let cmd = SscanCmd::new();
        assert!(cmd.acl_category().contains(AclCategory::SET));
        assert!(cmd.acl_category().contains(AclCategory::READ));
    }

    #[test]
    fn test_sscan_argument_validation() {
        let cmd = SscanCmd::new();

        // Valid argument counts
        assert!(cmd.check_arg(3)); // SSCAN key cursor
        assert!(cmd.check_arg(5)); // SSCAN key cursor MATCH pattern
        assert!(cmd.check_arg(7)); // SSCAN key cursor MATCH pattern COUNT count

        // Invalid argument counts
        assert!(!cmd.check_arg(1)); // Missing key and cursor
        assert!(!cmd.check_arg(2)); // Missing cursor
        assert!(!cmd.check_arg(0)); // No arguments
    }
}
