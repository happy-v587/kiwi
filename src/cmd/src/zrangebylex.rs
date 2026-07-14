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
pub struct ZrangebylexCmd {
    meta: CmdMeta,
}

impl ZrangebylexCmd {
    pub fn new() -> Self {
        Self {
            meta: CmdMeta {
                name: "zrangebylex".to_string(),
                arity: -4, // ZRANGEBYLEX key min max [LIMIT offset count]
                flags: CmdFlags::READONLY,
                acl_category: AclCategory::READ | AclCategory::SORTEDSET,
                ..Default::default()
            },
        }
    }
}

impl Cmd for ZrangebylexCmd {
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
        let min = &argv[2];
        let max = &argv[3];

        let mut offset = None;
        let mut count = None;

        // Parse LIMIT option
        let mut i = 4;
        while i < argv.len() {
            if argv[i].eq_ignore_ascii_case(b"LIMIT") {
                if i + 2 >= argv.len() {
                    client.set_error(error_catalog::SYNTAX_ERROR);
                    return;
                }
                match String::from_utf8_lossy(&argv[i + 1]).parse::<i64>() {
                    Ok(o) => offset = Some(o),
                    Err(_) => {
                        client.set_error(error_catalog::VALUE_NOT_INTEGER);
                        return;
                    }
                }
                match String::from_utf8_lossy(&argv[i + 2]).parse::<i64>() {
                    Ok(c) => count = Some(c),
                    Err(_) => {
                        client.set_error(error_catalog::VALUE_NOT_INTEGER);
                        return;
                    }
                }
                i += 3;
            } else {
                client.set_error(error_catalog::SYNTAX_ERROR);
                return;
            }
        }

        let result = storage.zrangebylex(&key, min, max, offset, count);

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
        Some(if argv.len() < 4 {
            Err(crate::error::CommandError::WrongArity {
                command: self.name().to_string(),
            })
        } else {
            let mut offset = None;
            let mut count = None;
            let mut index = 4;
            while index < argv.len() {
                if !argv[index].eq_ignore_ascii_case(b"LIMIT") || index + 2 >= argv.len() {
                    return Some(Err(crate::error::CommandError::InvalidArgument(
                        crate::error::ArgumentError::Syntax,
                    )));
                }
                offset = match String::from_utf8_lossy(&argv[index + 1]).parse::<i64>() {
                    Ok(value) => Some(value),
                    Err(_) => {
                        return Some(Err(crate::error::CommandError::InvalidArgument(
                            crate::error::ArgumentError::NotInteger,
                        )));
                    }
                };
                count = match String::from_utf8_lossy(&argv[index + 2]).parse::<i64>() {
                    Ok(value) => Some(value),
                    Err(_) => {
                        return Some(Err(crate::error::CommandError::InvalidArgument(
                            crate::error::ArgumentError::NotInteger,
                        )));
                    }
                };
                index += 3;
            }
            storage
                .zrangebylex(&argv[1], &argv[2], &argv[3], offset, count)
                .map(|members| {
                    RespData::Array(Some(
                        members
                            .into_iter()
                            .map(|member| RespData::BulkString(Some(Bytes::from(member))))
                            .collect(),
                    ))
                })
                .map_err(crate::error::CommandError::storage)
        })
    }

    fn uses_typed_execution(&self) -> bool {
        true
    }
}
