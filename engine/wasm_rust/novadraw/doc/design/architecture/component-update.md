# Figure 组件更新协议

类型：`normative-design`

状态：`accepted / initial-contract-implemented`

范围：ADR-014 D4.4 `typed component update`。

## 1. 问题

Runtime 必须继续作为 source mutation 的唯一提交边界，但不能为每一种第三方 Figure
新增 downcast 分支。直接暴露 `&mut dyn Figure` 或接收任意 mutation closure 会绕过
revision、damage、layout invalidation 和 panic/fault 边界。

本协议只更新单个 Figure 的私有组件状态，不修改 topology、Runtime registry、
LayoutManager 或其他 Figure。跨对象关系更新仍使用各自的 Runtime typed API。

## 2. 公共接口

```rust
pub trait FigureComponentUpdate {
    type Figure: Figure;
    type Prepared;
    type Error;

    fn prepare(
        self,
        current: &Self::Figure,
        context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error>;

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure);
}

pub struct FigureComponentContext {
    pub figure_id: FigureId,
    pub component_revision: u64,
    pub bounds: Rectangle,
}

pub struct PreparedFigureUpdate<T> {
    value: T,
    invalidation: ComponentInvalidation,
}

pub enum ComponentInvalidation {
    Paint,
    GeometryAndPaint,
    LayoutGeometryAndPaint,
}

pub enum ComponentUpdateError<E> {
    Runtime(RuntimeMutationError),
    WrongFigureType {
        figure: FigureId,
        expected: &'static str,
        actual: &'static str,
    },
    RevisionExhausted(FigureId),
    Rejected(E),
}

impl FigureEditor<'_> {
    pub fn update_component<U>(
        &mut self,
        update: U,
    ) -> Result<ComponentUpdateReceipt, ComponentUpdateError<U::Error>>
    where
        U: FigureComponentUpdate;
}
```

`PreparedFigureUpdate::new` 默认采用
`LayoutGeometryAndPaint`；调用方必须显式选择更窄的 invalidation，不能默认
“无影响”。`paint` 和 `geometry_and_paint` 是显式窄化构造器，本期不提供 `None`。
`ComponentUpdateReceipt` 返回 Figure、提交前后 revision 和实际 invalidation。
`FigureTree::component_revision` 提供只读 revision 查询，不暴露可变 Figure。
`FigureEditor` 只借用 Runtime 并转交目标身份，不拥有另一份状态或提交协议。

## 3. 执行顺序

```text
validate Runtime/attached/type/current revision capacity
-> capture old visual envelope and immutable context
-> U::prepare(&typed Figure, context) and validate owned candidate
-> U::commit(prepared, &mut typed Figure)
-> increment component revision
-> enqueue declared invalidation and old/new damage
-> return committed receipt
```

`prepare` 失败时，Runtime、Figure 与 revision 不变，owned update 及候选值正常释放。
`commit` 没有 Result；prepared value 必须已完整验证。`prepare` 或 `commit` panic 时
Runtime 标记 faulted，不承诺回滚 interior mutation 或外部副作用。

Runtime 能统一验证 target、namespace、revision 和 NodeState 几何；第三方私有候选值
的有限几何及引用合法性由 `prepare` 负责，因为引擎不能枚举其内部表示。

## 4. 私有派生快照

第三方 Figure 可把派生快照保存在自身具体类型中，由 prepared value 原子替换。
Runtime 不枚举快照类型，也不提供通用 `Any` registry。若派生计算依赖布局后的 bounds，
source update 只发布输入并声明 layout invalidation；稳定化阶段使用后续独立的 typed
derived component 协议，不能在 source commit 中读取未来布局结果。

## 5. 首个验收用例

在独立 integration test 中定义外部 `BadgeFigure`：

- 私有字段为 `text`、`measured_width` 和不可变 paint snapshot；
- `SetBadgeText` 拥有新 String，prepare 拒绝空文本并生成候选；
- commit 不需要修改 Runtime 分支；
- 成功后 revision 单调增加并产生 layout + paint；
- prepare 失败保持旧文本、revision 和 pending work 不变；
- foreign/disposed/wrong type 返回结构化错误；
- commit panic 后 Runtime faulted，后续 frame 返回 `FramePreparation::Error(Faulted)`。

该用例已落地于 `novadraw-scene/tests/d4_component_update.rs`。测试还验证保守更新实际
提升 layout generation、产生非空 damage，且完整 Runtime 移动之外不存在可变
Figure 逃逸。

## 6. 暂不包含

- 任意 closure mutation；
- 跨 Figure 原子事务；
- topology 或 registry 更新；
- 动态 plugin registry；
- 跨 Runtime prepared value 重放；
- 通用派生 DAG。
