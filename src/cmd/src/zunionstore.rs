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
pub struct ZunionstoreCmd {
    meta: CmdMeta,
}

impl ZunionstoreCmd {
    pub fn new() -> Self {
        Self {
            meta: CmdMeta {
                name: "zunionstore".to_string(),
                arity: -4, // ZUNIONSTORE destination numkeys key [key ...] [WEIGHTS weight [weight ...]] [AGGREGATE SUM|MIN|MAX]
                flags: CmdFlags::WRITE,
                acl_category: AclCategory::WRITE | AclCategory::SORTEDSET | AclCategory::SLOW,
                ..Default::default()
            },
        }
    }
}

impl Cmd for ZunionstoreCmd {
    impl_cmd_meta!();
    impl_cmd_clone_box!();

    fn do_initial(&self, client: &Client) -> bool {
        let argv = client.argv();
        let destination = argv[1].clone();
        client.set_key(&destination);
        true
    }

    fn do_cmd(&self, client: &Client, storage: Arc<Storage>) {
        let argv = client.argv();

        if argv.len() < 3 {
            client.set_error(error_catalog::wrong_number("zunionstore"));
            return;
        }

        let destination = &argv[1];

        // Parse numkeys
        let numkeys_str = String::from_utf8_lossy(&argv[2]);
        let numkeys = match numkeys_str.parse::<usize>() {
            Ok(n) if n > 0 => n,
            _ => {
                client.set_error(error_catalog::ZSTORE_NUMKEYS_GT_ZERO);
                return;
            }
        };

        if argv.len() < 3 + numkeys {
            client.set_error(error_catalog::SYNTAX_ERROR);
            return;
        }

        // Extract keys
        let keys: Vec<Vec<u8>> = argv[3..3 + numkeys].to_vec();

        // Parse optional WEIGHTS and AGGREGATE
        let mut weights: Vec<f64> = Vec::new();
        let mut aggregate = "SUM".to_string();
        let mut idx = 3 + numkeys;

        while idx < argv.len() {
            let option = String::from_utf8_lossy(&argv[idx]).to_uppercase();

            match option.as_str() {
                "WEIGHTS" => {
                    idx += 1;
                    if idx + numkeys > argv.len() {
                        client.set_error(error_catalog::SYNTAX_ERROR);
                        return;
                    }

                    for i in 0..numkeys {
                        let weight_str = String::from_utf8_lossy(&argv[idx + i]);
                        match weight_str.parse::<f64>() {
                            Ok(w) => weights.push(w),
                            Err(_) => {
                                client.set_error(error_catalog::ZSTORE_WEIGHT_NOT_FLOAT);
                                return;
                            }
                        }
                    }
                    idx += numkeys;
                }
                "AGGREGATE" => {
                    idx += 1;
                    if idx >= argv.len() {
                        client.set_error(error_catalog::SYNTAX_ERROR);
                        return;
                    }

                    aggregate = String::from_utf8_lossy(&argv[idx]).to_uppercase();
                    if aggregate != "SUM" && aggregate != "MIN" && aggregate != "MAX" {
                        client.set_error(error_catalog::SYNTAX_ERROR);
                        return;
                    }
                    idx += 1;
                }
                _ => {
                    client.set_error(error_catalog::SYNTAX_ERROR);
                    return;
                }
            }
        }

        let result = storage.zunionstore(destination, &keys, &weights, &aggregate);

        match result {
            Ok(count) => {
                client.set_reply(RespData::Integer(count as i64));
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
            let numkeys = match String::from_utf8_lossy(&argv[2]).parse::<usize>() {
                Ok(numkeys) if numkeys > 0 => numkeys,
                _ => {
                    return Some(Err(crate::error::CommandError::InvalidArgument(
                        crate::error::ArgumentError::ZStoreNumKeys,
                    )));
                }
            };
            if argv.len() < 3 + numkeys {
                return Some(Err(crate::error::CommandError::InvalidArgument(
                    crate::error::ArgumentError::Syntax,
                )));
            }
            let keys = argv[3..3 + numkeys].to_vec();
            let mut weights = Vec::new();
            let mut aggregate = "SUM".to_string();
            let mut index = 3 + numkeys;
            while index < argv.len() {
                match String::from_utf8_lossy(&argv[index])
                    .to_uppercase()
                    .as_str()
                {
                    "WEIGHTS" if index + numkeys < argv.len() => {
                        for weight in &argv[index + 1..index + 1 + numkeys] {
                            match String::from_utf8_lossy(weight).parse::<f64>() {
                                Ok(weight) => weights.push(weight),
                                Err(_) => {
                                    return Some(Err(crate::error::CommandError::InvalidArgument(
                                        crate::error::ArgumentError::ZStoreWeightNotFloat,
                                    )));
                                }
                            }
                        }
                        index += numkeys + 1;
                    }
                    "AGGREGATE" if index + 1 < argv.len() => {
                        aggregate = String::from_utf8_lossy(&argv[index + 1]).to_uppercase();
                        if !matches!(aggregate.as_str(), "SUM" | "MIN" | "MAX") {
                            return Some(Err(crate::error::CommandError::InvalidArgument(
                                crate::error::ArgumentError::Syntax,
                            )));
                        }
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
                .zunionstore(&argv[1], &keys, &weights, &aggregate)
                .map(|count| RespData::Integer(count.into()))
                .map_err(crate::error::CommandError::storage)
        })
    }

    fn uses_typed_execution(&self) -> bool {
        true
    }
}
