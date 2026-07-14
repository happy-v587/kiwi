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

use bytes::Bytes;
use std::sync::Arc;

use client::Client;
use resp::RespData;
use storage::storage::Storage;

use crate::{AclCategory, ClientExt, Cmd, CmdFlags, CmdMeta, CommandResult};
use crate::{impl_cmd_clone_box, impl_cmd_meta};

#[derive(Clone, Default)]
pub struct PingCmd {
    meta: CmdMeta,
}

impl PingCmd {
    pub fn new() -> Self {
        Self {
            meta: CmdMeta {
                name: "ping".to_string(),
                arity: -1,
                flags: CmdFlags::READONLY | CmdFlags::FAST | CmdFlags::NO_AUTH,
                acl_category: AclCategory::FAST | AclCategory::CONNECTION,
                ..Default::default()
            },
        }
    }
}

impl Cmd for PingCmd {
    impl_cmd_meta!();
    impl_cmd_clone_box!();

    fn do_initial(&self, _client: &Client) -> bool {
        true
    }

    fn do_cmd(&self, client: &Client, _storage: Arc<Storage>) {
        if client.argv().len() == 1 {
            client.set_reply(RespData::SimpleString("PONG".into()));
        } else if client.argv().len() == 2 {
            let arg = client.argv()[1].clone();
            client.set_reply(RespData::BulkString(Some(Bytes::from(arg))));
        } else {
            client.set_error(error_catalog::wrong_number("ping"));
        }
    }

    fn execute_typed(&self, client: &Client, _storage: Arc<Storage>) -> Option<CommandResult> {
        let argv = client.argv();
        Some(match argv.len() {
            1 => Ok(RespData::SimpleString("PONG".into())),
            2 => Ok(RespData::BulkString(Some(Bytes::from(argv[1].clone())))),
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

    fn client_with_argv(argv: &[&[u8]]) -> Client {
        let client = Client::new(Box::new(TestStream));
        client.set_argv(&argv.iter().map(|arg| arg.to_vec()).collect::<Vec<_>>());
        client
    }

    #[test]
    fn typed_ping_returns_reply_without_mutating_client_reply() {
        let cmd = PingCmd::new();
        let client = client_with_argv(&[b"ping", b"hello"]);

        let result = cmd
            .execute_typed(&client, Arc::new(Storage::new(1, 0)))
            .expect("PING supports typed execution")
            .expect("PING argument is valid");

        assert!(
            matches!(result, RespData::BulkString(Some(value)) if value == b"hello".as_slice())
        );
        assert_eq!(client.take_reply(), RespData::default());
    }

    #[test]
    fn typed_ping_reports_its_own_arity_error() {
        let cmd = PingCmd::new();
        let client = client_with_argv(&[b"ping", b"one", b"two"]);

        let result = cmd
            .execute_typed(&client, Arc::new(Storage::new(1, 0)))
            .expect("PING supports typed execution");

        assert!(matches!(
            result,
            Err(crate::error::CommandError::WrongArity { command }) if command == "ping"
        ));
    }
}
