# TC-04 类型化属性身份收口

类型：`implementation-verification`

日期：2026-10-08

状态：`complete`

## 1. 关闭范围

本批次关闭临时概念审计 TC-04：

- 属性身份和值类型不再由字符串与 `PropertyValue` 运行时约定关联；
- Animation Trigger 不再接受 property name；
- coalescing 和 shown/hidden 映射不再比较显示字符串；
- Listener/Inspector 风格消费者仍可读取稳定名称和结构化诊断值。

## 2. 实现

依据 [ADR-028](../../adr/adr-028-typed-property-identity.md) 新增：

```text
PropertyKey<V>
TypedPropertyChange<V>
  -> ErasedPropertyKey
  -> PropertyChangeEvent
  -> heterogeneous NotificationEffect journal
```

`PropertyChangeEvent` 字段已私有化，只能由 typed change 擦除构造。擦除 key 保留：

- namespace；
- stable name；
- Rust `TypeId`；
- diagnostic type name。

事件还私有保存 erased typed old/new value 与 equality function；coalescing 不比较
`PropertyValue` 诊断投影。

内置 Figure、style、viewport、scroll、scale、freeform、label、image、point-list 和
text-flow 属性已迁移到标准 typed key。领域枚举 key 直接绑定真实 Rust 类型，不以
`String` 冒充行为类型；`PropertyValue::Text` 仅是异构 journal 的诊断投影。

Animation 提供：

```text
AnimationTrigger::property_changed(PropertyKey<V>)
AnimationTrigger::state_changed(PropertyKey<V>)
```

后者要求 `V: DiscretePropertyValue`。`AnimationFact`、coalescing 和生命周期映射使用
`ErasedPropertyKey`；同名不同 namespace 或不同值类型不会匹配或合并。

## 3. 外部消费者证据

`tc04_typed_property_identity.rs` 定义仓库外语义的 `Mode`、`PropertyKey<Mode>` 和
`PropertyValueType` / `DiscretePropertyValue` 实现，通过 Figure input callback 发出
typed change，验证：

- custom property Behavior 无需修改 Core 枚举；
- property/state Trigger 使用同一 typed identity；
- 同名不同 namespace 保持两个 fact；
- 同 namespace/name 不同 Rust 类型身份不同；
- 多次变化按第一 old / 最后 new 合并；
- old/new 即使投影成相同诊断文本，真实 typed change 也不会被错误抵消；
- Listener 可读 namespace/name/type 与结构化值；
- shown/hidden 只由标准 `VISIBLE` key 映射。

README compile-fail 同时证明 `PropertyKey<bool>` 不能与 `f64` new value 组合。

## 4. 验证

- `cargo check --workspace`：通过；
- `cargo xtask verify core.tc04-typed-property-identity`：通过；
- `cargo xtask docs`：通过；
- `cargo clippy -p novadraw --lib -- -D warnings`：通过；
- `cargo xtask check --quick`：通过；
- `scripts/check_public_api_surface.sh`：通过；
- `git diff --check`：通过。

本批次不重复运行 full gate；当前 workspace test 的 Builder `Result` 迁移阻断仍按既有
记录处理，不属于 TC-04。
