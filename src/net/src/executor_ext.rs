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

//! Executor extensions for network operations
//!
//! This module provides extensions to CmdExecutor to support network-aware
//! command execution with StorageClient for async storage operations.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::LazyLock;

use client::Client;
use cmd::error::CommandError;
use cmd::{ClientExt, Cmd, CmdFlags};
use executor::CmdExecutor;
use log::{debug, error};
use runtime::DualRuntimeError;
use storage::storage::Storage;

use crate::error_response::RedisErrorRenderer;
use crate::network_execution::NetworkCmdExecution;

/// Extension trait for CmdExecutor to support network operations
pub trait CmdExecutorNetworkExt {
    /// Execute a network command using StorageClient for dual runtime architecture
    fn execute_network(
        &self,
        exec: NetworkCmdExecution,
    ) -> Pin<Box<dyn Future<Output = Result<(), DualRuntimeError>> + Send + '_>>;
}

impl CmdExecutorNetworkExt for CmdExecutor {
    fn execute_network(
        &self,
        exec: NetworkCmdExecution,
    ) -> Pin<Box<dyn Future<Output = Result<(), DualRuntimeError>> + Send + '_>> {
        Box::pin(async move {
            let cmd_name = String::from_utf8_lossy(&exec.client.cmd_name()).to_lowercase();
            debug!("Executing network command: {}", cmd_name);

            // Check argument count first
            let argv = exec.client.argv();
            if !exec.cmd.check_arg(argv.len()) {
                exec.client.set_reply(RedisErrorRenderer::render_command(
                    &CommandError::WrongArity {
                        command: exec.cmd.name().to_string(),
                    },
                ));
                return Ok(());
            }

            // Cluster-mode leader gate: reject writes on non-leaders before any
            // command-specific setup runs.
            if let Some(gate) = exec.leader_gate.as_ref() {
                if exec.cmd.has_flag(CmdFlags::WRITE) && !gate.is_leader() {
                    // Simplified redirect: Kiwi returns "MOVED <addr>" (no hash slot,
                    // unlike Redis Cluster's "MOVED <slot> <ip:port>"). Clients are
                    // expected to reconnect to the returned leader address directly.
                    let reply = match gate.leader_resp_addr() {
                        Some(addr) => format!("MOVED {addr}"),
                        None => error_catalog::NOT_LEADER.to_string(),
                    };
                    exec.client.set_error(reply);
                    return Ok(());
                }
            }

            if !exec.cmd.uses_typed_execution() {
                error!("Command '{}' is missing typed execution", cmd_name);
                exec.client
                    .set_reply(RedisErrorRenderer::render_command(&CommandError::Internal));
                return Ok(());
            }

            // Route connection-local commands locally and send all other
            // storage-backed commands through the generic Execute path.
            match cmd_name.as_str() {
                "ping" | "auth" | "client" | "hello" => {
                    execute_local_command(&exec).await?;
                }
                _ => {
                    execute_generic_command(&exec).await?;
                }
            }

            Ok(())
        })
    }
}

/// Shared dummy storage for connection-local commands that do not touch real
/// storage (ping/auth/client/hello). Reused across calls to avoid allocating a
/// throwaway `Storage` on every hot-path invocation.
static LOCAL_DUMMY_STORAGE: LazyLock<Arc<Storage>> = LazyLock::new(|| Arc::new(Storage::new(1, 0)));

async fn execute_local_command(exec: &NetworkCmdExecution) -> Result<(), DualRuntimeError> {
    if let Some(result) = exec
        .cmd
        .execute_typed(exec.client.as_ref(), Arc::clone(&LOCAL_DUMMY_STORAGE))
    {
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => RedisErrorRenderer::render_command(&error),
        };
        exec.client.set_reply(reply);
    } else {
        exec.client
            .set_reply(RedisErrorRenderer::render_command(&CommandError::Internal));
    }
    Ok(())
}

/// Execute a command against a directly-owned [`Storage`] instance.
///
/// This is used only by the legacy single-runtime TCP, Unix, and optimized
/// handlers. It keeps those compatibility entry points on the same typed
/// command/error path as the dual-runtime server.
pub(crate) fn execute_direct_typed_command(
    client: &Client,
    storage: Arc<Storage>,
    command: &dyn Cmd,
) {
    let result = if command.check_arg(client.argv().len()) {
        command
            .execute_typed(client, storage)
            .unwrap_or(Err(CommandError::Internal))
    } else {
        Err(CommandError::WrongArity {
            command: command.name().to_string(),
        })
    };

    client.set_reply(match result {
        Ok(reply) => reply,
        Err(error) => RedisErrorRenderer::render_command(&error),
    });
}

async fn execute_generic_command(exec: &NetworkCmdExecution) -> Result<(), DualRuntimeError> {
    let cmd_name = exec.client.cmd_name();
    let argv = exec.client.argv();
    let cmd_name_str = String::from_utf8_lossy(cmd_name.as_slice());

    match exec
        .storage_client
        .execute_command(cmd_name.as_slice(), &argv)
        .await
    {
        Ok(Ok(response)) => exec.client.set_reply(response),
        Ok(Err(error)) => {
            error!(
                "Generic command execution failed for '{}': {error:?}",
                cmd_name_str
            );
            exec.client
                .set_reply(RedisErrorRenderer::render_command(&error));
        }
        Err(e) => {
            error!(
                "Generic command execution failed for '{}': {}",
                cmd_name_str, e
            );
            exec.client
                .set_reply(RedisErrorRenderer::render_execution(&e));
        }
    }

    Ok(())
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::execute_direct_typed_command;
    use std::sync::Arc;

    use client::{Client, StreamTrait};
    use cmd::set::SetCmd;
    use storage::storage::Storage;

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
    fn direct_compatibility_path_renders_typed_errors() {
        let client = Client::new(Box::new(TestStream));
        client.set_argv(&[b"set".to_vec(), b"key".to_vec()]);

        execute_direct_typed_command(&client, Arc::new(Storage::new(1, 0)), &SetCmd::new());

        assert_eq!(
            client.take_reply().as_string().expect("error string"),
            error_catalog::wrong_number("set")
        );
    }
}
