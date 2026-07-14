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

use crate::{AclCategory, BaseCmdGroup, ClientExt, Cmd, CmdFlags, CmdMeta, CommandResult};
use crate::{impl_cmd_clone_box, impl_cmd_meta};

pub fn new_client_group_cmd() -> BaseCmdGroup {
    let mut client_cmd = BaseCmdGroup::new(
        "client".to_string(),
        -2,
        CmdFlags::ADMIN,
        AclCategory::ADMIN,
    );

    client_cmd.add_sub_cmd(Box::new(CmdClientGetname::new()));
    client_cmd.add_sub_cmd(Box::new(CmdClientSetname::new()));

    client_cmd
}

#[derive(Clone, Default)]
pub struct CmdClientGetname {
    meta: CmdMeta,
}

impl CmdClientGetname {
    pub fn new() -> Self {
        Self {
            meta: CmdMeta {
                name: "getname".to_string(),
                arity: 2,
                flags: CmdFlags::ADMIN | CmdFlags::READONLY,
                acl_category: AclCategory::ADMIN,
                ..Default::default()
            },
        }
    }
}

impl Cmd for CmdClientGetname {
    impl_cmd_meta!();
    impl_cmd_clone_box!();

    fn do_initial(&self, _client: &Client) -> bool {
        true
    }

    fn do_cmd(&self, client: &Client, _storage: Arc<Storage>) {
        let name = String::from_utf8_lossy(&client.name()).to_string();
        client.set_reply(RespData::BulkString(Some(name.into())));
    }

    fn execute_typed(&self, client: &Client, _storage: Arc<Storage>) -> Option<CommandResult> {
        let argv = client.argv();
        Some(match argv.len() {
            2 => {
                let name = String::from_utf8_lossy(&client.name()).to_string();
                Ok(RespData::BulkString(Some(name.into())))
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

#[derive(Clone, Default)]
pub struct CmdClientSetname {
    meta: CmdMeta,
}

impl CmdClientSetname {
    pub fn new() -> Self {
        Self {
            meta: CmdMeta {
                name: "setname".to_string(),
                arity: 3,
                flags: CmdFlags::ADMIN | CmdFlags::WRITE,
                acl_category: AclCategory::ADMIN,
                ..Default::default()
            },
        }
    }
}

impl Cmd for CmdClientSetname {
    impl_cmd_meta!();
    impl_cmd_clone_box!();

    fn do_initial(&self, _client: &Client) -> bool {
        true
    }

    fn do_cmd(&self, client: &Client, _storage: Arc<Storage>) {
        let argv = client.argv();
        if argv.len() < 3 {
            client.set_error(error_catalog::WRONG_NUMBER_GENERIC);
            return;
        }
        let new_name = argv[2].clone();
        client.set_name(&new_name);
        client.set_reply(RespData::SimpleString("OK".to_string().into()));
    }

    fn execute_typed(&self, client: &Client, _storage: Arc<Storage>) -> Option<CommandResult> {
        let argv = client.argv();
        Some(match argv.len() {
            3 => {
                client.set_name(&argv[2]);
                Ok(RespData::SimpleString("OK".into()))
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
    fn typed_client_setname_updates_state_and_returns_reply() {
        let cmd = new_client_group_cmd();
        let client = client_with_argv(&[b"client", b"setname", b"worker-1"]);

        let reply = cmd
            .execute_typed(&client, Arc::new(Storage::new(1, 0)))
            .expect("CLIENT supports typed execution")
            .expect("CLIENT SETNAME is valid");

        assert_eq!(reply, RespData::SimpleString("OK".into()));
        assert_eq!(client.name().as_slice(), b"worker-1");
        assert_eq!(client.take_reply(), RespData::default());
    }
}
