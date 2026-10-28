# suites 配置与单元测试的职责边界

类型：`architecture-review`

评估日期：2026-10-16

## 结论

`suites` 和单元测试解决的是不同层次的问题：

- 单元测试证明一段代码在给定输入下是否符合行为预期；
- suite 证明一个可交付功能切片所需的测试、构建、静态检查、应用验证和平台证据是否按
  约定组合执行。

suite 不提供新的断言能力，也不应替代单元测试。它的核心价值是把分散在不同 crate、
target、脚本和宿主环境中的验证动作，收口为一个稳定的验收身份。文档和评审只需引用
suite ID，不必复制容易漂移的命令序列。

从技术上说，任何 suite 最终仍然只是运行 command。直接执行同一组命令可以得到相同的
进程结果。从本项目治理上说，以下情况必须登记 suite：

- 交付或评审结论对应一个 milestone、API family 或产品能力；
- 验证需要两个以上 command，或者跨 crate、feature、target、平台；
- 验证包含 Cargo test 之外的静态检查、应用 replay、像素检查、性能报告或人工验收；
- 需要让后续文档、CI 或维护者通过稳定 ID 重放同一验收切片。

纯函数、局部算法、单模块状态转换等问题，如果一个精确定向测试已经完整覆盖失败模式，
仅使用单元测试更合适。为每个测试函数建立 suite 会增加 manifest 维护成本，却没有增加
验证能力。

## 当前实现

评估时的 `tests/suites.toml` 包含 106 个 command、
2 个 profile 和 59 个 suite。suite 类型分布为：

| 类型 | 数量 | 主要用途 |
|---|---:|---|
| `contract` | 31 | API、行为、跨模块和扩展消费者契约 |
| `application` | 18 | 应用路径、replay、视觉和平台行为 |
| `quality` | 9 | 工作区质量、性能、体积和证据采集 |
| `build` | 1 | Wasm 与浏览器验证构建 |

17 个 suite 带人工入口，25 个 suite 声明预期 artifact。这些数字只说明配置规模，不能
直接代表验证强度。suite 的实际价值取决于 `commands` 是否真正执行有意义的断言或检查。

## 配置结构

一个 suite 的完整结构如下：

```toml
[[suites]]
id = "core.p2-s07-layout-query"
title = "Checked layout queries and measurement admission"
milestone = "P2-S07"
kind = "contract"
api_semantics = [
  "layout.manager",
  "figure.box.client_area",
  "border.protocol",
]
layers = ["contract"]
platforms = ["headless"]
commands = [
  "test.p2-s07-layout-query",
  "test.p2-s07-layout-query-internal",
]
paths = [
  "crates/novadraw/src/graph/layout_measurement.rs",
  "crates/novadraw/src/layout",
  "crates/novadraw/tests/p2_query_failure_semantics.rs",
]
documents = [
  "doc/adr/adr-038-query-identity-and-failure-semantics.md",
  "doc/verification/plans/p2-s07-query-identity-and-failure-semantics.md",
]
```

字段职责如下：

| 字段 | 当前实现中的作用 | 是否影响执行 |
|---|---|---|
| `id` | 稳定选择器，支持完整 ID 和无歧义点分前缀 | 是 |
| `title` | `list` 输出中的可读名称 | 否 |
| `milestone` | `verify` 开始时打印交付归属 | 否 |
| `kind` | 分类为 quality、contract、application 或 build | 部分 |
| `api_semantics` | 把 contract/application suite 绑定到 parity API family | 部分 |
| `layers` | 声明 static、contract、headless、visual、platform 层 | 否 |
| `platforms` | 声明宿主和目标平台 | 否 |
| `commands` | 决定自动执行的 command 及顺序 | 是 |
| `paths` | 记录受影响源码范围 | 否 |
| `documents` | 记录设计、计划和验收文档，并要求文件存在 | 部分 |
| `artifacts` | 声明预期输出路径 | 否 |
| `manual` | 绑定人工手册和启动 command | 仅由 `manual` 子命令执行 |

这里的“否”表示当前 xtask 不消费该字段做调度或结果判定，不表示字段没有治理价值。
`paths`、`layers`、`platforms` 和 `artifacts` 可以帮助维护者理解覆盖面，也为未来的变更
选 suite、平台矩阵和产物校验提供结构化基础。

## 加载与校验机制

每次运行 xtask 都先完成以下步骤：

1. 从 `tests/suites.toml` 反序列化 manifest；
2. 拒绝未知字段和不支持的 schema version；
3. 校验 command、profile 和 suite ID；
4. 要求 `quick`、`full` profile 存在；
5. 要求 suite ID 唯一，标题、milestone、layers 和 platforms 非空；
6. 要求 contract/application suite 至少映射一个已登记的 API family；
7. 拒绝 suite 内重复或不存在的 command；
8. 要求 `documents` 和人工手册真实存在；
9. 拒绝绝对路径和包含父目录跳转的路径；
10. 拒绝没有被 profile、suite 或 manual 引用的 command。

这意味着 suite 同时承担了一部分 manifest 完整性检查。配置漂移会在执行任何子命令前
失败，不会等到某个特定 suite 被调用时才暴露。

## 执行机制

`cargo xtask verify <suite-id>` 的执行路径是：

```text
加载 manifest
    ↓
完整校验 manifest
    ↓
按完整 ID 或唯一前缀解析 suite
    ↓
按配置顺序执行 commands
    ↓
任一子进程非零则立即停止
```

每个 command 包含 `program`、argv、可选工作目录和环境变量。xtask 使用进程 API 直接
启动命令，不通过 shell 拼接字符串。执行前打印 command ID、说明和实际命令，子进程的
输出直接进入终端。

`verify --all` 会遍历全部 suite，并按 command ID 去重后执行。去重只认 command ID：
两个不同 ID 即使 argv 完全相同，仍会重复执行。

人工步骤不属于普通 `verify`。带 `manual` 的 suite 需要另行运行
`cargo xtask manual <suite-id>`，该命令只打印手册路径并启动对应程序，不会记录人工验收
结论。

## 当前未实现的能力

现有 suite 不能自动完成以下工作：

- 根据 Git diff 和 `paths` 选择受影响 suite；
- 根据当前宿主过滤 `platforms` 不匹配的 command；
- 根据 `layers` 自动补齐缺失的验证层；
- 在执行后检查 `artifacts` 是否生成、是否为本次新产物、schema 是否正确；
- 计算语句、分支或函数覆盖率；
- 判断测试集合是否完整覆盖了 `api_semantics`；
- 比较性能和二进制体积是否超过回归阈值；
- 把人工操作结果写入结构化证据。

因此，suite 是验收编排和可追溯性机制，不是自动覆盖率证明系统。

## 单元测试能证明什么

常规单元测试最适合以下问题：

- 纯函数的输入输出；
- 算法边界和错误分类；
- 单个模块内部的不变量；
- 小型状态机的确定性转换；
- 无平台依赖的数据结构行为；
- 一个失败可以直接定位到少量代码的局部契约。

它的优势来自测试和实现距离很近：

- 启动成本低；
- 可以并行执行大量 case；
- fixture 小，失败定位直接；
- 不依赖外部程序、窗口、GPU 或网络；
- 重构实现时容易同步维护。

Rust 的 crate 内单元测试还能访问私有实现，适合验证内部数值算法和不可公开的辅助状态。
本项目的 `geometry::checked_inverse::s07_checked_inverse_tests` 就属于这种情况。

## 仅使用单元测试的局限

### 无法自然表达跨命令验收

一个公共能力可能需要同时证明：

- 内部算法正确；
- 外部消费者可以编译；
- 负向 API 示例必须编译失败；
- Native 和 Wasm feature 都能构建；
- 依赖图没有穿透 Core；
- 应用路径产生预期报告。

这些检查可以被强行塞进一个测试进程，但会让单元测试负责启动 Cargo、rustdoc、脚本和
窗口程序。测试会变慢、环境依赖加重，失败定位也会变差。suite 把这些动作保留为独立
command，再以验收切片组合，职责更清楚。

### 不覆盖构建矩阵

默认 `cargo test` 只覆盖当前宿主和当前启用 feature。它不能自动证明：

- `--no-default-features` 是否可编译；
- Wasm target 是否可编译；
- Native/Web backend feature 是否互相独立；
- Windows/Linux 目标是否满足依赖和条件编译；
- release profile 是否能构建。

这些属于构建资格，不是运行期单元行为。

### 不覆盖源码与依赖边界

热路径禁止日志、Core 禁止平台 I/O、公开 API 禁止第三方类型、crate root 只能导出领域
模块等规则，通常需要源码扫描、rustdoc 分析、依赖图检查或公开符号快照。单元测试只有
在违规代码被实际调用并表现出可观察错误时才可能发现问题，无法可靠约束这类结构规则。

### 不覆盖真实应用链路

单元测试可以用 fake 验证局部协议，但不能证明真实 Tool、Request、Command、模型通知、
Viewer 投影和 overlay 是否按生产路径连在一起。应用 replay 复用真实 harness，能验证
多阶段因果顺序和故障前缀，这是局部 mock 测试无法等价证明的。

### 不覆盖平台和视觉事实

真实 GPU 提交、窗口可见性、WindowServer 呈现、浏览器 WebGPU、DPI 像素和输入到呈现
链路依赖实际宿主资源。单元测试最多证明平台适配器对 mock 的处理，不能证明真实平台
行为。

### 缺少稳定的交付身份

测试文件名和过滤器会随重构变化。milestone、API family 和评审文档需要比具体 test
target 更稳定的引用。suite ID 提供这一层长期身份，让验收记录可以写
`core.p2-s07-coordinate-route-query`，无需固化五条底层命令。

## 覆盖范围对比

| 验证对象 | 单元测试 | suite |
|---|---|---|
| 函数和算法行为 | 强 | 通过 command 间接覆盖 |
| 私有实现不变量 | 强 | 不直接提供 |
| 单 crate 公共 API | 可覆盖 | 可组合 integration/doc test |
| 跨 crate 消费者 | 较弱 | 适合 |
| feature/target 构建 | 不自然 | 适合 |
| Clippy、fmt、源码扫描 | 不覆盖 | 可组合 |
| 依赖图和公开 API 快照 | 不覆盖 | 可组合 |
| 应用 replay | 不适合 | 适合 |
| GPU、浏览器、真实窗口 | 不覆盖 | 可绑定平台 command |
| 性能和体积报告 | 不适合 | 可编排 |
| 人工验收手册 | 不表达 | 可绑定 |
| milestone/API family 追踪 | 不表达 | 适合 |
| artifact 声明 | 不表达 | 当前可声明但不校验 |

suite 的覆盖能力来自 command 组合，不来自 `[[suites]]` 配置本身。一个只运行空脚本的
suite 仍然没有价值；一个精确单元测试可以比配置庞大但没有断言的 suite 更可靠。

## 执行效率对比

| 维度 | 单元测试 | suite |
|---|---|---|
| 单次反馈速度 | 通常最快 | 取决于 command 数量和平台成本 |
| 编译复用 | 同一 test target 内较好 | 多个 Cargo/rustdoc command 可能重复编译 |
| 并行能力 | Rust test case 默认并行 | suite command 当前串行 |
| 失败定位 | 通常精确到 case | 先定位 command，再查看内部 case 或报告 |
| 选择粒度 | test 名、module、target | 稳定功能切片 |
| 环境成本 | 通常低 | 可高到需要 GPU、浏览器或不同 OS |
| 重放一致性 | 依赖调用者记住参数 | suite 固化命令和顺序 |
| 全量成本 | 单 crate 可控 | `verify --all` 可能跨平台且非常昂贵 |

最有效的工作流采用分层方式：

1. 编辑内环运行一个精确单元测试；
2. 局部实现稳定后运行受影响的 integration test；
3. 功能切片完成后运行对应 suite；
4. 批次边界运行 quick profile；
5. 最终提交边界运行 full profile和必要的平台 suite。

这样既保留单元测试的快速反馈，也避免交付时漏掉跨边界证据。

## 典型 suite 的价值

### 纯测试组合

`core.p2-s07-layout-query` 只包含两个 Cargo test command：

- 一个外部 integration test；
- 一个过滤后的 crate 内部单元测试。

它没有增加测试能力，但仍有治理价值：它把外部失败语义、内部 identity 语义、三个 API
family、ADR 和验证计划绑定成一个 P2-S07 验收入口。

如果这两个测试只服务日常局部开发，不对应正式交付切片，直接运行测试即可，无需 suite。

### 跨模块契约组合

`core.p2-s07-coordinate-route-query` 组合五个 command，覆盖：

- 外部坐标查询错误；
- 内部精确奇异和计算范围；
- Route adapter identity 与依赖指纹；
- M4 坐标基线；
- Connection 公共契约。

单个单元测试无法代表这个切片的完整回归面。这里使用 suite 是必要的。

### 架构边界组合

`core.facade` 组合无默认 feature 编译、doctest、依赖图、第三方类型、Kurbo 版本和
Native/Web backend 编译。它证明的是 package 边界和公开 facade 资格，常规单元测试无法
完整替代。

### 应用链路组合

`g3.selection-targeting` 组合 Core dispatch test、Editor selection test、Viewer
interaction test 和真实 EditorHarness replay，并绑定 JSON artifact 与 Native 人工手册。
它覆盖的是从输入仲裁到投影结果的完整交互切片，必须使用 suite。

### 应用级验证

`core.runtime` 不以常规 test target 为主，而是运行 update-app 和 event-app 的确定性
验证模式。程序在失败时返回非零，并写结构化报告。suite 为这些应用探针提供统一入口和
artifact 归属。

### 不应机械保留的 suite

`workspace.quality` 只组合 fmt、Clippy 和 workspace test，与 full profile 的职责重叠，
且声明的 artifact 当前没有生产者。这个例子说明 suite 数量不是质量指标。无法形成独立
功能切片、没有稳定交付身份、只是重复 profile 的配置，应合并或移除。

## 必须使用 suite 的判断

在本项目中，满足以下任一条件就应配置 suite：

| 条件 | 原因 |
|---|---|
| 一个验收结论需要两个以上 command | 防止调用者遗漏步骤或改变顺序 |
| 跨 crate、feature、target 或 OS | 单一 test target 无法代表构建矩阵 |
| 包含脚本、rustdoc、依赖图或源码扫描 | 已超出单元测试职责 |
| 需要运行应用 replay、示例 verify 或 benchmark | 需要统一非 test 二进制入口 |
| 需要 GPU、浏览器、窗口或真实像素 | 需要平台和视觉验证层 |
| 需要生成或引用 artifact | 需要证据归属和固定输出约定 |
| 存在人工验收步骤 | 需要绑定手册与启动入口 |
| 对应 milestone 或正式评审 | 需要稳定、可追溯的验收身份 |
| 对应一个或多个 parity API family | 需要把语义声明与可执行证据关联 |
| 后续 CI 需要按功能切片重放 | 需要稳定选择器 |

这里的“必须”是工程治理要求。底层命令当然可以手工逐条运行，但无法保证每次交付都使用
相同集合，也无法让评审文档稳定引用。

## 仅单元测试即可的判断

以下条件全部满足时，通常只需要单元测试：

- 行为局限在一个函数、模块或 crate 内部；
- 输入输出确定，不依赖真实时钟、文件系统、GPU、窗口、浏览器或网络；
- 不涉及 feature、target 或平台条件编译；
- 不需要验证公共 package 边界和第三方消费者；
- 不需要额外静态检查、构建检查、报告、截图或人工步骤；
- 一个精确定向测试目标足以完整表达成功和失败条件；
- 该行为不需要独立的 milestone 或长期验收身份。

典型例子包括：

- 几何算法的边界输入；
- parser 的合法和非法 token；
- 小型 cache 的命中、失效和淘汰；
- 单个状态机的状态迁移；
- 错误枚举的分类；
- 纯数据结构的不变量。

如果需求后来扩展为“外部 crate 能否调用”“Wasm 是否可编译”或“真实应用是否按同一路径
工作”，应保留原单元测试，并在其上增加 suite。

## 不同项目的选择标准

| 项目类型 | 建议方案 | 何时引入 suite |
|---|---|---|
| 小型纯算法库 | 单元测试为主 | 出现 feature matrix、公开 API 快照或正式发布门禁时 |
| 单 crate CLI | 单元测试 + 少量 integration test | 需要打包、跨平台、真实文件/进程 smoke 时 |
| Web/API 服务 | 单元测试 + integration/contract test | 需要数据库迁移、容器、端到端 API、部署 smoke 时 |
| 多 crate SDK 或框架 | 单元测试和 suite 并用 | 从一开始就为 public facade、consumer、feature/target 建 suite |
| GUI/图形引擎 | suite 是必要基础设施 | 涉及像素、GPU、窗口、输入、DPI、平台适配时 |
| 插件平台 | suite 是必要基础设施 | 需要外部消费者证明、开放协议和 ABI/API 边界时 |
| Monorepo | 单元测试负责局部，suite 负责产品切片 | 一个交付跨多个 package 或团队边界时 |
| 原型和一次性工具 | 单元测试可选，通常不建 suite | 准备长期维护、发布或形成多人协作时 |
| 合规或高审计项目 | 测试与 suite 都必须 | suite 需要进一步校验 artifact、revision 和审批证据 |

项目规模不是唯一标准。一个代码量很小但需要跨三种平台发布的工具，比一个大型纯算法
crate 更需要 suite。判断依据应是验收边界和证据种类。

## 配置建议

### 先写测试，再登记 command，最后建立 suite

推荐顺序：

1. 用精确测试表达最小行为契约；
2. 把稳定的执行方式登记为 command；
3. 当交付结论需要组合多个 command 或稳定身份时，再建立 suite。

suite 不应成为“还没想清楚断言时先占一个配置”的容器。

### 一个 suite 只证明一个交付切片

suite 应围绕行为或交付能力命名，避免按文件名或临时任务命名。命令集合应满足：

- 每条 command 都能解释其对结论的贡献；
- 删除任一 command 都会形成明确证据缺口；
- 不夹带与该切片无关的 workspace 全量检查；
- 不重复执行相同 argv 的别名 command；
- command 顺序按低成本到高成本排列，尽早失败。

### 不为每个单元测试建 suite

如果 suite 只有一个普通 `cargo test` command，且没有 milestone、API family、文档引用或
长期重放需求，直接运行该测试即可。无意义的 suite 会让 `list` 变长，也会增加 ID 和
文档维护成本。

### 区分 suite 与 profile

- suite 回答“某个功能切片是否通过”；
- profile 回答“当前工作树是否达到某个通用质量门槛”。

fmt、Clippy、workspace test 等通用门禁更适合 profile。只有当它们确实是某个能力的必要
证据时，才应被 suite 引用。

### 让元数据保持真实

虽然当前 `paths`、`platforms`、`layers` 和 `artifacts` 尚未参与调度，仍应准确维护：

- `paths` 只列真正影响该切片的代码；
- `platforms` 不写未实际执行的平台；
- `layers` 不把 headless test 标成 visual；
- `artifacts` 只声明 command 确实生成的产物；
- `manual` 只绑定无法自动化且有明确检查项的步骤。

这些字段未来一旦进入自动选择和结果校验，错误元数据会直接造成漏跑或误判。

## 取舍原则

最终可以用一句话判断：

> 局部代码行为用测试证明，交付切片用 suite 证明。

落到执行上：

- 能由一个确定性的精确测试完整证明，就只写测试；
- 需要组合不同测试目标，建立 suite；
- 需要构建、静态边界、应用路径或平台事实，必须建立 suite；
- 需要人工判断，suite 绑定手册，但不能把“程序已打开”记为验收通过；
- 需要长期审计，suite 还应补 artifact 新鲜度、revision 和 schema 校验；
- suite 没有真实断言或检查时，配置本身没有价值。

对 Novadraw 这类多 crate、跨 Native/Web、包含 GPU 和 Editor 应用链路的框架，完全退回
“只有单元测试”的方案会丢失公共 API、构建矩阵、真实应用和平台资格证据。反过来，把每个
局部 case 都提升为 suite 也会拖慢反馈并污染验收目录。当前合理方向是继续保留两层体系：
单元测试负责快速、精确的实现反馈，suite 负责稳定、可追溯的功能切片验收。
