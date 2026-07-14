# Kiwi Error 处理框架理想设计

日期：2026-07-13  
状态：设计讨论稿  
设计前提：假设从零设计 Kiwi，不考虑当前代码的迁移成本。

## 1. 设计目标

这套设计希望解决以下问题：

1. 开发者看到函数返回的 Error 类型，就知道错误属于哪一层；
2. 开发者能够快速判断一个失败是否可以返回客户端；
3. Error 类型和枚举值尽可能少，但不能少到失去类型信息；
4. 不通过错误字符串判断错误类别；
5. 底层 source 在跨层传播时不会丢失；
6. 客户端文案、程序控制逻辑和服务端日志互相分离；
7. 普通业务结果不被滥用成 Error；
8. 每种 Error 都只有一个清晰职责。

## 2. 总体方案

采用“每层一个小 Error，错误只向上转换”的设计。

| 层次 | Error 类型 | 职责 |
|---|---|---|
| RESP | `ParseError` | RESP 字节解析失败 |
| Storage | `StorageError` | RocksDB、IO、内部数据和状态错误 |
| Command | `CommandError` | 命令执行和 Redis 语义错误 |
| Runtime | `ExecutionError` | timeout、channel、worker、过载和命令执行结果 |
| Raft | `RaftError` | 共识、leader、Raft transport 和 log store 错误 |
| Server | `StartupError` | 配置、bind、runtime 创建和启动失败 |

错误传播方向固定为：

```text
rocksdb::Error
    ↓
StorageError
    ↓
CommandError
    ↓
ExecutionError
    ↓
Network error renderer
    ↓
RESP error response
```

禁止反向依赖，也禁止将 typed error 转成字符串后重新分类。

## 3. Command 层：唯一的客户端业务错误边界

### 3.1 命令接口

命令不再通过修改 `Client` 的 reply 字段表达成功或失败，而是统一返回 `Result`：

```rust
pub trait Command {
    fn execute(
        &self,
        ctx: &mut CommandContext,
        storage: &Storage,
    ) -> Result<Reply, CommandError>;
}
```

执行结果只有两种：

```text
Ok(Reply)          命令成功
Err(CommandError)  命令失败
```

不再使用以下模式：

```rust
client.set_error(...);
return;

client.set_storage_error(...);
return;

Ok(RespData::Error(...))
```

### 3.2 CommandError

`CommandError` 保存错误语义，不保存最终 Redis 错误字符串：

```rust
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("wrong key type")]
    WrongType,

    #[error("wrong number of arguments for {command}")]
    WrongArity {
        command: &'static str,
    },

    #[error("invalid command argument")]
    InvalidArgument {
        kind: ArgumentError,
    },

    #[error("authentication failed")]
    Authentication {
        kind: AuthenticationError,
    },

    #[error("unknown command: {command}")]
    UnknownCommand {
        command: String,
    },

    #[error("not leader")]
    NotLeader {
        leader: Option<String>,
    },

    #[error(transparent)]
    Storage(#[from] StorageError),

    #[error(transparent)]
    Raft(#[from] RaftError),
}
```

可复用的小分类：

```rust
pub enum ArgumentError {
    Syntax,
    NotInteger,
    NotFloat,
    OutOfRange,
    InvalidCursor,
}

pub enum AuthenticationError {
    Required,
    WrongPassword,
    PasswordNotConfigured,
}
```

### 3.3 控制枚举数量

不是每条 Redis 文案都创建顶层 variant，而是按照程序需要采取的行为分组：

```text
CommandError
├── WrongType
├── WrongArity
├── InvalidArgument(ArgumentError)
├── Authentication(AuthenticationError)
├── UnknownCommand
├── NotLeader
├── Storage(StorageError)
└── Raft(RaftError)
```

### 3.4 命令代码示例

理想代码：

```rust
fn execute(...) -> Result<Reply, CommandError> {
    let value = storage.get(key)?;
    Ok(Reply::BulkString(value))
}
```

而不是：

```rust
match storage.get(key) {
    Ok(value) => client.set_reply(...),
    Err(error) => {
        client.set_storage_error(&error);
        return;
    }
}
```

## 4. Storage 层：只描述存储事实

### 4.1 StorageError

Storage 层不知道 RESP、`ERR`、WRONGTYPE 文案或 network runtime。

```rust
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("storage engine failure")]
    Engine {
        #[source]
        source: rocksdb::Error,
    },

    #[error("I/O failure")]
    Io {
        #[source]
        source: std::io::Error,
    },

    #[error("stored data is corrupted during {operation}")]
    Corruption {
        operation: &'static str,
        detail: String,
    },

    #[error("invalid storage state during {operation}")]
    InvalidState {
        operation: &'static str,
        detail: String,
    },

    #[error("expected {expected:?}, found {actual:?}")]
    WrongType {
        expected: DataType,
        actual: DataType,
    },
}
```

只保留五类，是因为调用方对它们确实可能采取不同处理：

| Variant | 含义 | 上层行为 |
|---|---|---|
| `Engine` | RocksDB 执行失败 | 记录 source，客户端返回 internal error |
| `Io` | 文件或磁盘 IO 失败 | 记录 source，客户端返回 internal error |
| `Corruption` | 已存储的数据无法解析 | 高级别报警，客户端返回 internal error |
| `InvalidState` | 程序状态或内部不变量异常 | 记录程序错误，客户端返回 internal error |
| `WrongType` | Redis key 类型不符合命令要求 | 转成客户端 WRONGTYPE |

### 4.2 不再保留的重叠类型

不单独保留：

```text
Encoding
InvalidFormat
OptionNone
System
Unknown
Transaction
Batch
Compaction
Config
KeyNotFound
RedisErr
```

处理原则：

- 有 RocksDB source → `Engine`；
- 有 IO source → `Io`；
- 已存储数据无法解码 → `Corruption`；
- 程序认为不可能为空但为空 → `InvalidState`；
- key 类型错误 → `WrongType`；
- 调用方没有不同处理行为 → 不新增独立 variant。

### 4.3 正常结果不是 Error

key 不存在：

```rust
pub fn get(
    &self,
    key: &[u8],
) -> Result<Option<Bytes>, StorageError>;
```

条件写入结果：

```rust
pub enum SetOutcome {
    Stored,
    AlreadyExists,
}
```

`AlreadyExists`、key 不存在、scan 完成等正常业务状态不应该被放进 Error。

## 5. Runtime 层：只描述执行是否完成

### 5.1 ExecutionError

不设计包含大量字符串 variant 的 `DualRuntimeError`，而是使用：

```rust
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error(transparent)]
    Command(#[from] CommandError),

    #[error("command execution timed out")]
    Timeout,

    #[error("storage runtime is unavailable")]
    Unavailable,

    #[error("storage request queue is full")]
    Overloaded,

    #[error("storage channel is closed")]
    ChannelClosed,

    #[error("server is shutting down")]
    ShuttingDown,

    #[error("command worker stopped unexpectedly")]
    WorkerStopped,
}
```

### 5.2 不单独保留的概念

不再使用：

```text
NetworkRuntime(String)
StorageRuntime(String)
Channel(String)
Lifecycle(String)
HealthCheck(String)
ErrorBoundary(String)
FaultIsolation(String)
RuntimeIsolation(String)
RecoveryFailed(String)
```

如果最终行为相同，就归并到：

- `Unavailable`；
- `Overloaded`；
- `ChannelClosed`；
- `ShuttingDown`；
- `WorkerStopped`。

### 5.3 对外接口

```rust
pub async fn execute(
    &self,
    request: CommandRequest,
) -> Result<Reply, ExecutionError>;
```

Storage runtime 内部执行：

```rust
let result: Result<Reply, CommandError> =
    command.execute(&mut context, &storage);
```

跨 runtime 返回：

```rust
result.map_err(ExecutionError::Command)
```

timeout、channel closed 等错误才由 runtime 本身产生。

## 6. 错误只向上转换

依赖关系固定：

```text
Storage 不知道 CommandError
Storage 不知道 RespData
RESP 不知道 WRONGTYPE
Runtime 不拼接 Redis 错误文本
Network 负责最终日志和客户端响应
```

底层错误向上传递时保留 source：

```text
ExecutionError::Command(
    CommandError::Storage(
        StorageError::Engine { source }
    )
)
```

它表达的是：

```text
一次执行失败
→ 原因来自命令
→ 命令失败原因来自 storage
→ storage 失败原因来自 RocksDB
```

禁止：

```text
StorageError
→ String
→ RuntimeError
→ 搜索 String
→ 猜测原始错误类型
```

## 7. Network 层：唯一客户端错误渲染入口

### 7.1 RedisErrorRenderer

客户端错误文案只存在于：

```text
src/net/src/error_response.rs
```

```rust
pub struct RedisErrorRenderer;

impl RedisErrorRenderer {
    pub fn render(error: &ExecutionError) -> RespData {
        RespData::error(Self::message(error))
    }

    fn message(error: &ExecutionError) -> String {
        match error {
            ExecutionError::Command(error) => {
                Self::command_message(error)
            }
            ExecutionError::Timeout => {
                "ERR command timeout".to_string()
            }
            ExecutionError::Overloaded => {
                "ERR server overloaded".to_string()
            }
            ExecutionError::Unavailable
            | ExecutionError::ChannelClosed
            | ExecutionError::ShuttingDown
            | ExecutionError::WorkerStopped => {
                "ERR internal server error".to_string()
            }
        }
    }
}
```

### 7.2 CommandError 渲染

```rust
fn command_message(error: &CommandError) -> String {
    match error {
        CommandError::WrongType => {
            "WRONGTYPE Operation against a key holding the wrong kind of value"
                .to_string()
        }

        CommandError::WrongArity { command } => {
            format!(
                "ERR wrong number of arguments for '{command}' command"
            )
        }

        CommandError::InvalidArgument {
            kind: ArgumentError::Syntax,
        } => {
            "ERR syntax error".to_string()
        }

        CommandError::Storage(
            StorageError::WrongType { .. }
        ) => {
            "WRONGTYPE Operation against a key holding the wrong kind of value"
                .to_string()
        }

        CommandError::Storage(_) => {
            "ERR internal server error".to_string()
        }

        // 其他稳定映射省略
    }
}
```

Command 和 Storage 代码不包含 `ERR`、WRONGTYPE、NOAUTH 等协议文案。

### 7.3 静态检查

CI 规则：

> 除 `error_response.rs` 及测试外，生产代码禁止出现 Redis error class 文案。

## 8. 日志与客户端响应分离

network boundary 统一处理：

```rust
match executor.execute(request).await {
    Ok(reply) => reply,

    Err(error) => {
        log_execution_error(&context, &error);
        RedisErrorRenderer::render(&error)
    }
}
```

服务端结构化日志：

```rust
tracing::error!(
    request_id = %context.request_id,
    command = %context.command,
    client_addr = %context.client_addr,
    error = ?error,
    "command execution failed"
);
```

客户端只收到：

```text
ERR internal server error
```

服务端可以看到完整错误链：

```text
ExecutionError::Command(
  CommandError::Storage(
    StorageError::Corruption {
      operation: "decode list metadata",
      detail: "expected 16 bytes, got 7"
    }
  )
)
```

### 8.1 请求上下文不放进 Error

```rust
pub struct RequestContext {
    pub request_id: RequestId,
    pub command: CommandName,
    pub client_addr: SocketAddr,
}
```

request ID、客户端地址和命令名属于请求上下文。Error 只描述失败原因，避免每个 Error variant 携带相同字段。

不需要复杂的全局内存型 `ErrorLogger` 才能实现错误关联。

## 9. 重试策略不属于 Error

不提供通用的：

```rust
error.is_recoverable()
```

能否重试取决于：

- 命令是读还是写；
- 请求是否已经进入 storage runtime；
- storage 是否可能已经提交；
- 命令是否幂等；
- Raft 是否已经写入日志；
- 客户端是否允许重试。

使用独立策略：

```rust
RetryPolicy::decide(
    &command_metadata,
    &execution_stage,
    &error,
)
```

Error 只报告事实，Policy 决定行为。

禁止：

```rust
if error.to_string().contains("timeout") {
    retry();
}
```

## 10. RaftError 独立设计

Raft 是独立子系统：

```rust
#[derive(Debug, thiserror::Error)]
pub enum RaftError {
    #[error("not leader")]
    NotLeader {
        leader: Option<String>,
    },

    #[error("raft quorum unavailable")]
    QuorumUnavailable,

    #[error("raft request rejected")]
    Rejected,

    #[error("raft log store failure")]
    LogStore {
        #[source]
        source: StorageError,
    },

    #[error("raft transport failure")]
    Transport {
        #[source]
        source: tonic::Status,
    },

    #[error("raft internal failure during {operation}")]
    Internal {
        operation: &'static str,
        detail: String,
    },
}
```

Command 层只转换需要理解的语义：

```rust
impl From<RaftError> for CommandError {
    fn from(error: RaftError) -> Self {
        match error {
            RaftError::NotLeader { leader } => {
                CommandError::NotLeader { leader }
            }
            other => CommandError::Raft(other),
        }
    }
}
```

客户端映射：

```text
NotLeader + leader address → MOVED ...
NotLeader + no address     → ERR not leader
其他 RaftError             → ERR internal server error
```

## 11. StartupError 独立设计

启动失败不进入命令错误链：

```rust
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("configuration error")]
    Config {
        #[source]
        source: ConfigError,
    },

    #[error("failed to create runtime")]
    Runtime {
        #[source]
        source: RuntimeBuildError,
    },

    #[error("failed to open storage at {path:?}")]
    StorageOpen {
        path: PathBuf,
        #[source]
        source: StorageError,
    },

    #[error("failed to bind {address}")]
    Bind {
        address: SocketAddr,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to initialize raft")]
    Raft {
        #[source]
        source: RaftError,
    },
}
```

`main()`：

```rust
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,

        Err(error) => {
            tracing::error!(
                error = ?error,
                "Kiwi failed to start"
            );
            ExitCode::FAILURE
        }
    }
}
```

## 12. 统一错误库和构造规则

理想情况下全项目统一使用：

```text
thiserror
```

用途：

- `#[source]` 保留错误链；
- `#[from]` 处理无歧义的转换；
- Display 只用于日志，不用于程序分类。

不混用：

```text
Snafu selector
thiserror
自由 String
大量手写构造函数
format! 包装 source
```

### 12.1 可以自动转换的情况

无歧义的底层错误：

```rust
#[from]
source: rocksdb::Error
```

### 12.2 应该显式转换的情况

具有语义选择的跨层转换：

```rust
storage
    .get(key)
    .map_err(CommandError::Storage)?;
```

显式写出 `Storage` 能帮助阅读者理解错误跨越了哪一层。

不使用只把 `String` 塞进同名 variant 的构造方法。

## 13. 文件布局

```text
src/resp/src/error.rs
  ParseError

src/storage/src/error.rs
  StorageError

src/cmd/src/error.rs
  CommandError
  ArgumentError
  AuthenticationError

src/common/runtime/error.rs
  ExecutionError

src/raft/src/error.rs
  RaftError

src/server/src/error.rs
  StartupError

src/net/src/error_response.rs
  RedisErrorRenderer
  全部客户端错误文案
```

每个模块通常只需要理解自己的 Error 和直接相邻层的转换边界。

## 14. 开发者使用规则

| 当前场景 | 应该使用 |
|---|---|
| RESP 字节无法解析 | `ParseError` |
| 参数或 Redis 命令语义错误 | `CommandError` |
| RocksDB、IO、数据损坏、内部状态 | `StorageError` |
| timeout、channel、worker、过载 | `ExecutionError` |
| Raft 共识或通信失败 | `RaftError` |
| 配置、bind、启动失败 | `StartupError` |
| key 不存在、条件不满足 | 正常返回值，不使用 Error |

新增 variant 前只问两个问题：

1. 调用方是否需要采用不同的处理行为？
2. 现有 variant 是否已经能够准确表达？

如果第一题答案为否，就不新增 variant。

## 15. 测试策略

| 测试 | 验证内容 |
|---|---|
| Error 单元测试 | source chain 和类型转换没有丢失 |
| Renderer 快照测试 | Redis 错误文案和 RESP 字节完全正确 |
| Command 测试 | 成功返回 `Reply`，失败返回 `CommandError` |
| Storage 测试 | key 不存在是 `Ok(None)`，数据损坏才是 Error |
| Runtime 测试 | timeout、channel closed、worker stopped 分类准确 |
| 安全测试 | RocksDB 路径、内部格式、source 文案不会返回客户端 |
| 静态检查 | renderer 之外禁止出现客户端错误文案 |

## 16. 完整示例

### 16.1 正常命令

```rust
fn execute(...) -> Result<Reply, CommandError> {
    let value = storage.get(key)?;
    Ok(Reply::BulkString(value))
}
```

### 16.2 RocksDB 错误

```text
rocksdb::Error
→ StorageError::Engine
→ CommandError::Storage
→ ExecutionError::Command
→ 服务端记录完整 source
→ 客户端收到 ERR internal server error
```

### 16.3 WRONGTYPE

```text
StorageError::WrongType
→ CommandError::Storage
→ ExecutionError::Command
→ RedisErrorRenderer
→ 客户端收到固定 WRONGTYPE
```

### 16.4 Timeout

```text
runtime 等待响应超时
→ ExecutionError::Timeout
→ 服务端记录 request context
→ RedisErrorRenderer
→ 客户端收到 ERR command timeout
```

## 17. 最终效果

这套理想框架并不追求全项目只有一个 Error。它追求的是：

- 每层最多一个 Error；
- 每个 Error 都有唯一职责；
- 枚举值只对应真正不同的处理行为；
- 构造方法很少；
- 错误只向上传播一次；
- source 不会变成字符串而丢失；
- 客户端文案只存在于 renderer；
- 服务端日志保留完整错误链；
- 普通业务结果不被当成 Error；
- 开发者能通过一张表确定应该使用哪个 Error。
