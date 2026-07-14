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

use client::Client;
use resp::RespData;
use storage::storage::Storage;

use crate::{AclCategory, ClientExt, Cmd, CmdFlags, CmdMeta, CommandResult};
use crate::{impl_cmd_clone_box, impl_cmd_meta};

#[derive(Clone, Default)]
pub struct ZcountCmd {
    meta: CmdMeta,
}

impl ZcountCmd {
    pub fn new() -> Self {
        Self {
            meta: CmdMeta {
                name: "zcount".to_string(),
                arity: 4, // ZCOUNT key min max
                flags: CmdFlags::READONLY,
                acl_category: AclCategory::READ | AclCategory::SORTEDSET,
                ..Default::default()
            },
        }
    }
}

impl Cmd for ZcountCmd {
    impl_cmd_meta!();
    impl_cmd_clone_box!();

    fn do_initial(&self, client: &Client) -> bool {
        let argv = client.argv();

        // Validate argument count
        if argv.len() != 4 {
            client.set_error(error_catalog::wrong_number("zcount"));
            return false;
        }

        let key = argv[1].clone();
        client.set_key(&key);

        true
    }

    fn do_cmd(&self, client: &Client, storage: Arc<Storage>) {
        let argv = client.argv();
        let key = &argv[1];
        let min_str = &argv[2];
        let max_str = &argv[3];

        // Parse min and max scores
        let min = match String::from_utf8_lossy(min_str).parse::<f64>() {
            Ok(score) => score,
            Err(err_msg) => {
                client.set_error(format!("{}", err_msg));
                return;
            }
        };

        let max = match String::from_utf8_lossy(max_str).parse::<f64>() {
            Ok(score) => score,
            Err(err_msg) => {
                client.set_error(format!("{}", err_msg));
                return;
            }
        };

        // Perform the ZCOUNT operation
        match storage.zcount(key, min, max) {
            Ok(count) => {
                client.set_reply(RespData::Integer(count as i64));
            }
            Err(err_msg) => {
                client.set_error(format!("{}", err_msg));
            }
        }
    }

    fn execute_typed(&self, client: &Client, storage: Arc<Storage>) -> Option<CommandResult> {
        let argv = client.argv();
        Some(match argv.len() {
            4 => match (
                String::from_utf8_lossy(&argv[2]).parse::<f64>(),
                String::from_utf8_lossy(&argv[3]).parse::<f64>(),
            ) {
                (Ok(min), Ok(max)) => storage
                    .zcount(&argv[1], min, max)
                    .map(|count| RespData::Integer(count.into()))
                    .map_err(crate::error::CommandError::storage),
                _ => Err(crate::error::CommandError::InvalidArgument(
                    crate::error::ArgumentError::InvalidScoreRange,
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
