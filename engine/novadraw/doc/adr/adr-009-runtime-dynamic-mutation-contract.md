# ADR-009：Runtime 动态 Mutation 事务

类型：`architecture-decision`

能力归属：Core Runtime。

状态：已接受，2026-09-08；既有 K01 增量效力保持。

## 问题

挂载后的树结构、布局约束和样式若能被任意入口直接修改，失效、通知与身份会脱离
同一个提交点；callback 内立即修改还会使正在遍历的结构失效。

## 决策

Runtime 是唯一运行期提交权威。所有写入先验证、再提交，再发布失效与通知。
callback mutation 按 FIFO 延迟处理，失败通过原错误通道返回。
child order、size override 与 clipping 使用节点状态作为唯一真源；
外部 LayoutManager 在提交前校验约束。

## 替代与代价

不公开 `tree_mut()`，不在 callback 内立即提交，也不让具体 Figure 私藏 clipping
权威。这些路径虽便于局部调用，却绕过一致性边界。采用受检事务需承担准备成本，
且原子性只覆盖引擎受控状态，不承诺回滚外部回调私有副作用。

## 关联设计

[Mutation 与子树准入](../design/architecture/runtime-mutation-and-subtree-admission.md)
定义写入、回执、完整 contents 替换与构造期连接绑定。
[ADR-033](adr-033-runtime-subsystem-facade.md)细化统一 mutation 调用面。
