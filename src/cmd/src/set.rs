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

use crate::{ClientExt, Cmd, CmdFlags, CmdMeta, CommandResult};
use crate::{impl_cmd_clone_box, impl_cmd_meta};

#[derive(Clone, Default)]
pub struct SetCmd {
    meta: CmdMeta,
}

impl SetCmd {
    pub fn new() -> Self {
        Self {
            meta: CmdMeta {
                name: "set".to_string(),
                arity: 3,
                flags: CmdFlags::WRITE | CmdFlags::RAFT,
                ..Default::default()
            },
        }
    }
}

impl Cmd for SetCmd {
    impl_cmd_meta!();
    impl_cmd_clone_box!();

    /// SET key value
    fn do_initial(&self, client: &Client) -> bool {
        // TODO: support xx, nx, ex, px
        let argv = client.argv();

        let key = argv[1].clone();
        client.set_key(&key);

        true
    }

    fn do_cmd(&self, client: &Client, storage: Arc<Storage>) {
        let key = client.key();
        let value = &client.argv()[2];

        let result = storage.set(&key, value);

        match result {
            Ok(_) => {
                client.set_reply(RespData::SimpleString("OK".to_string().into()));
            }
            Err(e) => {
                client.set_storage_error(&e);
            }
        }
    }

    fn execute_typed(&self, client: &Client, storage: Arc<Storage>) -> Option<CommandResult> {
        let argv = client.argv();
        Some(match argv.len() {
            3 => match storage.set(&argv[1], &argv[2]) {
                Ok(()) => Ok(RespData::SimpleString("OK".into())),
                Err(error) => Err(crate::error::CommandError::storage(error)),
            },
            _ => Err(crate::error::CommandError::WrongArity {
                command: self.name().to_string(),
            }),
        })
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use client::StreamTrait;

    struct TestStream;

    #[async_trait::async_trait]
    impl StreamTrait for TestStream {
        async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, std::io::Error> {
            Ok(0)
        }

        async fn write(&mut self, _data: &[u8]) -> Result<usize, std::io::Error> {
            Ok(0)
        }
    }

    #[test]
    fn typed_set_reports_arity_without_mutating_the_client_reply() {
        let client = Client::new(Box::new(TestStream));
        client.set_argv(&[b"set".to_vec(), b"key".to_vec()]);

        let result = SetCmd::new()
            .execute_typed(&client, Arc::new(Storage::new(1, 0)))
            .expect("SET supports typed execution");

        assert!(matches!(
            result,
            Err(crate::error::CommandError::WrongArity { command }) if command == "set"
        ));
        assert_eq!(client.take_reply(), RespData::default());
    }
}
