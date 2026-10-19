# Java 到 Rust 的语义迁移

类型：`migration-guide`

本文定义迁移方法；语言特性速查见 [OO 映射](oo-mapping.md)。
示例用于解释 Rust 规则，Novadraw 当前 API 以
[公共契约](../../design/architecture/public-api-contract.md)为准。

## 先固定可观察行为

迁移对象是行为及扩展契约。对 Draw2D/GEF，先列出有序树、坐标转换、状态隔离、
validation/repair、连接依赖、输入仲裁、模型投影与命令历史。
Java 类层级、方法拼写、对象引用和 SWT 类型分别评估，不能作为行为等价的证明。

每项契约记录输入、结果、所有者、变化顺序、失败后状态、外部替换点及验证证据。
源码结论绑定仓库、提交与 class/method；历史报告只证明对应快照。

## 从所有权推导数据结构

Java 的 `extends` 同时复用字段、实现和子类型关系，Rust 应分别处理：

| Java 关系 | Rust 处理 |
|---|---|
| 对象持有字段 | struct 明确持有数据 |
| 可替换行为 | 小 trait；按需求采用泛型或 trait object |
| 默认算法复用 | trait 默认方法、组合 helper 或普通函数 |
| parent/child 对象引用 | 明确 owner；需要跨对象引用时使用受检身份 |
| 框架回调改变对象 | 阶段上下文与受控输出，由 owner 校验提交 |
| 生命周期与订阅 | 明确注册、解除、退休与迟到事件责任 |

supertrait 表达能力要求，不继承字段。不要按 Java 继承深度机械生成 trait 链。
Novadraw 的节点几何、拓扑和提交权限分别由节点、树和 Runtime 承载；具体 Figure
保存私有内容和行为。不能把 Java `setBounds` 直接迁为可绕过 Runtime 的 Figure setter。

## 默认实现不能与 blanket impl 混淆

下面是普通 Rust 教学示例：

```rust
trait Caption {
    fn text(&self) -> &str;
    fn display(&self) -> String {
        self.text().to_owned()
    }
}

struct Badge(String);

impl Caption for Badge {
    fn text(&self) -> &str { &self.0 }
    fn display(&self) -> String { format!("[{}]", self.text()) }
}
```

具体 impl 可以覆盖 trait 的默认方法。如果已有 `impl<T: SomeTrait> Caption for T`，
就不能再为满足该约束的 `Badge` 写一个重叠的 `impl Caption`。
稳定 Rust 不提供 Java 式“覆盖 blanket impl”。默认绘制复用应采用 helper 或组合，
保留第三方类型自己实现协议的空间。

## 选择分发方式

异构存储可用 `Box<dyn Trait>`；临时调用可用 `&dyn Trait`；已知类型的算法可用
`T: Trait`。泛型与动态分发都需要按真实消费者选择，不能据语法直接宣称更快。
堆分配来自 `Box` 等存储选择，trait object 引用本身不要求堆分配。

类型检查需要 `Any` 时，只能转回实际 concrete type；`&T` 不能据此调用需要
`&mut T` 的方法。框架消费者若需要跨类型行为，应优先定义 typed capability，
避免让每个调用方维护 concrete downcast 分支。

## 按依赖顺序迁移

1. 固定源码基线、行为清单、反例和失败后状态。
2. 定义状态 owner、身份域及扩展输入/输出。
3. 先写最窄契约及测试，再迁移实现和消费者。
4. 将缺陷、主动收窄、后置能力分别登记到 parity 与 backlog。
5. 更新规范、链接和验证证据，再关闭对应切片。

现行 Novadraw 内置与扩展共用 typed identity、payload、registry 和提交语义。
不能把“内置 enum + 外部 envelope”当成默认迁移路线。
Editor 产品 schema、serializer 和产品级入口由独立产品包负责。

## 验证等价与恢复

单元或契约测试验证具体反例、顺序、非法输入及恢复；独立消费者证明扩展无需修改
Core；Headless 验证确定性事务；Native/Web 验证设备输入、IME、DPI 和真实像素。
命令数量、已有类型或一个可运行 demo 都不足以证明语义对等。

布局、路由、paint 与 damage 必须用同一几何事实；离屏完整重绘不能替代 surface
局部修复验收。错误、合法缺失和合法不变分别表达；panic/fault 不冒充业务拒绝，
也不承诺回滚任意外部副作用。

当前分层验证命令见 [仓库规则](../../../CLAUDE.md)与
[验证入口](../../../verification/README.md)。历史方法示例不提供另一套执行命令。
