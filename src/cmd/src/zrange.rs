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

use crate::CommandResult;
use crate::{AclCategory, ClientExt, Cmd, CmdFlags, CmdMeta};
use crate::{impl_cmd_clone_box, impl_cmd_meta};
use bytes::Bytes;
use client::Client;
use resp::RespData;
use storage::storage::Storage;

#[derive(Clone, Default)]
pub struct ZrangeCmd {
    meta: CmdMeta,
}

impl ZrangeCmd {
    pub fn new() -> Self {
        Self {
            meta: CmdMeta {
                name: "zrange".to_string(),
                arity: -4, // ZRANGE key start stop [WITHSCORES]
                flags: CmdFlags::READONLY,
                acl_category: AclCategory::READ | AclCategory::SORTEDSET,
                ..Default::default()
            },
        }
    }
}

impl Cmd for ZrangeCmd {
    impl_cmd_meta!();
    impl_cmd_clone_box!();

    fn do_initial(&self, client: &Client) -> bool {
        let argv = client.argv();
        let key = argv[1].clone();
        client.set_key(&key);
        true
    }

    fn do_cmd(&self, client: &Client, storage: Arc<Storage>) {
        let argv = client.argv();
        let key = client.key();

        // Parse start index
        let start_str = String::from_utf8_lossy(&argv[2]);
        let start = match start_str.parse::<i64>() {
            Ok(s) => s,
            Err(_) => {
                client.set_error(error_catalog::VALUE_NOT_INTEGER);
                return;
            }
        };

        // Parse stop index
        let stop_str = String::from_utf8_lossy(&argv[3]);
        let stop = match stop_str.parse::<i64>() {
            Ok(s) => s,
            Err(_) => {
                client.set_error(error_catalog::VALUE_NOT_INTEGER);
                return;
            }
        };

        // Check for WITHSCORES option
        let with_scores = if argv.len() > 4 {
            let option = String::from_utf8_lossy(&argv[4]).to_lowercase();
            if option == "withscores" {
                true
            } else {
                client.set_error(error_catalog::SYNTAX_ERROR);
                return;
            }
        } else {
            false
        };

        let result = storage.zrange(&key, start, stop, with_scores);

        match result {
            Ok(members) => {
                let resp_array: Vec<RespData> = members
                    .into_iter()
                    .map(|m| RespData::BulkString(Some(m.into())))
                    .collect();
                client.set_reply(RespData::Array(Some(resp_array)));
            }
            Err(e) => {
                client.set_storage_error(&e);
            }
        }
    }

    fn execute_typed(&self, client: &Client, storage: Arc<Storage>) -> Option<CommandResult> {
        let argv = client.argv();
        Some(match argv.len() {
            4 | 5 => {
                let start = match String::from_utf8_lossy(&argv[2]).parse::<i64>() {
                    Ok(start) => start,
                    Err(_) => {
                        return Some(Err(crate::error::CommandError::InvalidArgument(
                            crate::error::ArgumentError::NotInteger,
                        )));
                    }
                };
                let stop = match String::from_utf8_lossy(&argv[3]).parse::<i64>() {
                    Ok(stop) => stop,
                    Err(_) => {
                        return Some(Err(crate::error::CommandError::InvalidArgument(
                            crate::error::ArgumentError::NotInteger,
                        )));
                    }
                };
                let with_scores = argv.len() == 5;
                if with_scores && !argv[4].eq_ignore_ascii_case(b"WITHSCORES") {
                    return Some(Err(crate::error::CommandError::InvalidArgument(
                        crate::error::ArgumentError::Syntax,
                    )));
                }
                storage
                    .zrange(&argv[1], start, stop, with_scores)
                    .map(|members| {
                        RespData::Array(Some(
                            members
                                .into_iter()
                                .map(|member| RespData::BulkString(Some(Bytes::from(member))))
                                .collect(),
                        ))
                    })
                    .map_err(crate::error::CommandError::storage)
            }
            _ => Err(crate::error::CommandError::WrongArity {
                command: self.name().to_string(),
            }),
        })
    }

    fn uses_typed_execution(&self) -> bool {
        true
    }
}
