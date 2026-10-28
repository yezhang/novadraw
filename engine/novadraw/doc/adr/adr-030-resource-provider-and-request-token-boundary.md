# ADR-030：ResourceProvider 与资源请求 Token 边界

类型：`architecture-decision`

状态：`accepted`

日期：2026-10-09

## 背景

Core 当前同时提供 `ImageData::decode_file`、`Runtime::complete_image_file` 和
`Runtime::complete_image_bytes`。这造成三个问题：

1. backend-neutral Core 直接依赖文件系统，并通过 `cfg` 形成 Native/Web 不同表面；
2. 文件读取、图片解码和 Runtime resource mutation 失败被压缩进
   `ImageDecodeError`；
3. 未来 Web fetch 或 worker 的迟到完成没有 request identity，可能覆盖更新请求。

ADR-012 已规定异步 loader 必须携带 resource-local request token，但此前没有产品路径，
因此只记录了合同。本次 ResourceProvider 边界开始承载 Native/Web 加载，必须同时实现
该 token 合同。

## 决策

### 1. Runtime 不拥有 ResourceProvider

规范流向为：

```text
application
  -> Native FileResourceProvider / Web FetchResourceProvider
  -> bytes
  -> Core ImageData::decode
  -> Runtime resource completion
```

Runtime 不保存 provider，不主动读取 Path/URL，也不轮询 future。应用仍是 composition
root，负责选择 provider、调度异步任务并把结果提交给 Runtime。

不在 Core 定义统一 `ResourceProvider` trait。文件 Path、URL、同步 I/O、Web Promise 和
平台错误不是同一稳定语义；强行统一会引入 boxed future、字符串错误或平台 source
枚举。Native/Web 分别提供具体 provider，复用的是 bytes 与 request token 合同。

### 2. Core 只保留纯解码

`ImageData` 保留：

```text
decode(bytes, scale)
decode_png(bytes, scale)
decode_svg(bytes, scale)
```

删除 `decode_file`。`ImageDecodeError` 只表达解码错误，删除文件 I/O 和 Runtime update
占位变体。

Runtime 删除 `complete_image_file`。bytes 完成使用
`complete_image_request_bytes`，其错误明确分为：

```text
ImageCompletionError::Decode(ImageDecodeError)
ImageCompletionError::Resource(ResourceError)
```

provider 读取失败保持 provider 自己的错误类型；应用可用
`fail_resource_request(token, diagnostic)` 把稳定诊断写入 Runtime，但原始错误仍由调用者
持有。

### 3. 每次异步请求必须有 Token

Core 提供：

```text
begin_resource_request(ResourceId) -> ResourceRequestToken
complete_image_request(token, ImageData)
complete_image_request_bytes(token, bytes, scale)
complete_font_request(token, FontData)
fail_resource_request(token, diagnostic)
```

每个 ResourceId 同时只有一个 active request generation。开始新请求会使旧 token
失效；重复、迟到或已被直接 mutation 取代的完成返回
`ResourceError::StaleRequest`，且不得改变：

- resource status 或 revision；
- pending ResourceDelta；
- dependent Figure invalidation。

直接 `complete_image`、`complete_font`、`fail_resource` 和 `remove_resource` 是同步
应用 mutation，会使 active request 失效。Ready 资源开始 replacement request 时继续
保留旧 Ready payload，直到新请求成功或显式失败。

### 4. Platform provider

Native platform package 提供 `FileResourceProvider`：

```text
load(path) -> io::Result<Vec<u8>>
```

Web platform package在 `resource` feature 下提供 `FetchResourceProvider`：

```text
load(url).await -> Result<Vec<u8>, WebResourceError>
```

Web provider 区分 window unavailable、fetch rejection、非成功 HTTP status、response
类型错误和 body 读取失败。两种 provider 都不依赖 Runtime，不解码图片，也不提交资源。

### 5. 注册 API 只保留 fallible 权威入口

`Runtime::register_image` 与 `Runtime::register_font` 直接返回 `Result`。删除
`try_register_*` 和 panic convenience，避免 faulted Runtime 存在两套注册语义。

底层 `ResourceRegistry` 的本地 slot 分配仍可保持 infallible；公开 Runtime 边界负责
fault 检查。

## 兼容与后果

- 本次属于 0.1 breaking migration，不保留文件 API 或 panic alias；
- Core Native/Web 公共表面一致，不再出现资源文件 I/O 的 target `cfg`；
- provider error、decode error 与 Runtime mutation error 可分别处理；
- token 只保护 Engine 内资源发布，不声称取消平台 I/O；
- provider cache、HTTP cache、文件 watcher、worker pool 和资源预算仍归应用/平台，
  不进入 Core。

## 验证

- Core source gate 拒绝 `std::fs`、Path 和 `decode_file` 重新进入 render/runtime；
- request replacement、duplicate completion、direct mutation invalidation 和 foreign token
  均有合同测试；
- decode 与 resource mutation 错误保持不同 variant；
- Native 文件 provider 读取真实 fixture 后经 Core decode 和 Runtime completion；
- Web fetch provider wasm32 feature 编译；
- root/module API snapshot、docs、Clippy、quick 与 full gate。

## 关系

- 实现 ADR-012 第 6 节和资源生命周期设计第 11 节的 request token；
- 延续 ADR-017 的结构化失败合同；
- 延续 ADR-023 的 Core/platform package 边界；
- 关闭临时概念设计审计 TC-11 与 API-F09 的资源边界部分。
