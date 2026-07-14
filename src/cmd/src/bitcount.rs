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
pub struct BitcountCmd {
    meta: CmdMeta,
}

impl BitcountCmd {
    pub fn new() -> Self {
        Self {
            meta: CmdMeta {
                name: "bitcount".to_string(),
                arity: -2, // BITCOUNT key [start end]
                flags: CmdFlags::READONLY,
                acl_category: AclCategory::STRING | AclCategory::READ,
                ..Default::default()
            },
        }
    }
}

impl Cmd for BitcountCmd {
    impl_cmd_meta!();
    impl_cmd_clone_box!();

    /// BITCOUNT key [start end]
    ///
    /// Count the number of set bits (population counting) in a string.
    fn do_initial(&self, client: &Client) -> bool {
        let argv = client.argv();

        // Check if the number of arguments is valid
        if argv.len() < 2 || argv.len() > 4 {
            client.set_error(error_catalog::wrong_number("bitcount"));
            return false;
        }

        // Use the key for routing in distributed setup
        let key = argv[1].clone();
        client.set_key(&key);

        true
    }

    fn do_cmd(&self, client: &Client, storage: Arc<Storage>) {
        let argv = client.argv();
        let key = client.key();

        // Parse optional start and end parameters
        let (start, end) = if argv.len() == 4 {
            // Parse start
            let start_val: i64 = match String::from_utf8_lossy(&argv[2]).parse() {
                Ok(val) => val,
                Err(_) => {
                    client.set_error(error_catalog::VALUE_NOT_INTEGER);
                    return;
                }
            };

            // Parse end
            let end_val: i64 = match String::from_utf8_lossy(&argv[3]).parse() {
                Ok(val) => val,
                Err(_) => {
                    client.set_error(error_catalog::VALUE_NOT_INTEGER);
                    return;
                }
            };

            (Some(start_val), Some(end_val))
        } else if argv.len() == 2 {
            // No range specified
            (None, None)
        } else {
            // Invalid number of arguments
            client.set_error(error_catalog::wrong_number("bitcount"));
            return;
        };

        match storage.bitcount(&key, start, end) {
            Ok(count) => {
                client.set_reply(RespData::Integer(count));
            }
            Err(e) => client.set_storage_error(&e),
        }
    }

    fn execute_typed(&self, client: &Client, storage: Arc<Storage>) -> Option<CommandResult> {
        let argv = client.argv();
        Some(match argv.len() {
            2 => storage
                .bitcount(&argv[1], None, None)
                .map(RespData::Integer)
                .map_err(crate::error::CommandError::storage),
            4 => match (
                String::from_utf8_lossy(&argv[2]).parse::<i64>(),
                String::from_utf8_lossy(&argv[3]).parse::<i64>(),
            ) {
                (Ok(start), Ok(end)) => storage
                    .bitcount(&argv[1], Some(start), Some(end))
                    .map(RespData::Integer)
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
