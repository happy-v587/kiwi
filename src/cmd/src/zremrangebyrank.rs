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

use crate::{AclCategory, ClientExt, Cmd, CmdFlags, CmdMeta, CommandResult};
use crate::{impl_cmd_clone_box, impl_cmd_meta};
use client::Client;
use resp::RespData;
use storage::storage::Storage;

#[derive(Clone, Default)]
pub struct ZremrangebyrankCmd {
    meta: CmdMeta,
}

impl ZremrangebyrankCmd {
    pub fn new() -> Self {
        Self {
            meta: CmdMeta {
                name: "zremrangebyrank".to_string(),
                arity: 4, // ZREMRANGEBYRANK key start stop
                flags: CmdFlags::WRITE,
                acl_category: AclCategory::WRITE | AclCategory::SORTEDSET,
                ..Default::default()
            },
        }
    }
}

impl Cmd for ZremrangebyrankCmd {
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

        let start = match String::from_utf8_lossy(&argv[2]).parse::<i64>() {
            Ok(s) => s,
            Err(_) => {
                client.set_error(error_catalog::VALUE_NOT_INTEGER);
                return;
            }
        };

        let stop = match String::from_utf8_lossy(&argv[3]).parse::<i64>() {
            Ok(s) => s,
            Err(_) => {
                client.set_error(error_catalog::VALUE_NOT_INTEGER);
                return;
            }
        };

        let result = storage.zremrangebyrank(&key, start, stop);

        match result {
            Ok(count) => client.set_reply(RespData::Integer(count as i64)),
            Err(e) => {
                client.set_storage_error(&e);
            }
        }
    }

    fn execute_typed(&self, client: &Client, storage: Arc<Storage>) -> Option<CommandResult> {
        let argv = client.argv();
        Some(match argv.len() {
            4 => match (
                String::from_utf8_lossy(&argv[2]).parse::<i64>(),
                String::from_utf8_lossy(&argv[3]).parse::<i64>(),
            ) {
                (Ok(start), Ok(stop)) => storage
                    .zremrangebyrank(&argv[1], start, stop)
                    .map(|count| RespData::Integer(count.into()))
                    .map_err(crate::error::CommandError::storage),
                _ => Err(crate::error::CommandError::InvalidArgument(
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
