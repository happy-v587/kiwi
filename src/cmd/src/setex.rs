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
pub struct SetexCmd {
    meta: CmdMeta,
}

impl SetexCmd {
    pub fn new() -> Self {
        Self {
            meta: CmdMeta {
                name: "setex".to_string(),
                arity: 4, // SETEX key seconds value
                flags: CmdFlags::WRITE,
                acl_category: AclCategory::STRING | AclCategory::WRITE,
                ..Default::default()
            },
        }
    }
}

impl Cmd for SetexCmd {
    impl_cmd_meta!();
    impl_cmd_clone_box!();

    /// SETEX key seconds value
    fn do_initial(&self, client: &Client) -> bool {
        let argv = client.argv();
        let key = argv[1].clone();
        client.set_key(&key);
        true
    }

    fn do_cmd(&self, client: &Client, storage: Arc<Storage>) {
        let argv = client.argv();
        let key = client.key();

        // Parse seconds parameter
        let seconds = match String::from_utf8_lossy(&argv[2]).parse::<i64>() {
            Ok(n) => n,
            Err(_) => {
                client.set_error(error_catalog::VALUE_NOT_INTEGER);
                return;
            }
        };

        // Validate seconds - must be positive
        if seconds <= 0 {
            client.set_error(error_catalog::INVALID_EXPIRE_TIME);
            return;
        }

        let value = &argv[3];

        let result = storage.setex(&key, seconds, value);

        match result {
            Ok(()) => {
                client.set_reply(RespData::SimpleString("OK".into()));
            }
            Err(e) => client.set_storage_error(&e),
        }
    }

    fn execute_typed(&self, client: &Client, storage: Arc<Storage>) -> Option<CommandResult> {
        let argv = client.argv();
        Some(match argv.len() {
            4 => match String::from_utf8_lossy(&argv[2]).parse::<i64>() {
                Ok(seconds) if seconds > 0 => match storage.setex(&argv[1], seconds, &argv[3]) {
                    Ok(()) => Ok(RespData::SimpleString("OK".into())),
                    Err(error) => Err(crate::error::CommandError::storage(error)),
                },
                Ok(_) => Err(crate::error::CommandError::InvalidArgument(
                    crate::error::ArgumentError::InvalidExpireTime,
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
