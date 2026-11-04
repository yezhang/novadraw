# ADR-028：类型化属性身份与异构通知日志

类型：`architecture-decision`

状态：`accepted`

日期：2026-10-08

## 背景

`PropertyChangeEvent` 目前把 `&'static str` 名称与 `PropertyValue` 并列存储。名称既是
诊断文本，又被 Animation Behavior 用作匹配和 coalescing 身份，类型系统无法证明
`"enabled"` 对应 `bool`，也无法阻止同名属性用不同值类型进入同一事务。

Animation 总体设计已经禁止字符串 property path，因此 Behavior 专题中的
`PropertyChanged(name)` 只是阶段性接口，不是长期合同。

## 决策

### 1. 类型化源事件

属性源必须先构造：

```text
PropertyKey<V>
TypedPropertyChange<V>
```

`PropertyKey<V>` 由 namespace、稳定名称和值类型组成。namespace/name 用于稳定诊断，
Rust `TypeId` 用于进程内身份比较。空 namespace/name 在常量构造时拒绝。

`TypedPropertyChange<V>` 同时持有 key、old value 和 new value，因此非法的 key/value
组合不能构造。

### 2. 异构 journal

Runtime effect queue 保存擦除后的 `PropertyChangeEvent`：

```text
TypedPropertyChange<V>
  -> ErasedPropertyKey
  -> PropertyValue diagnostic payload
  -> PropertyChangeEvent
```

`PropertyChangeEvent` 字段私有，只能由 typed change 擦除得到。Listener、Inspector 和
异构 journal 可以读取：

- Figure identity；
- erased property identity；
- namespace/name/type name；
- 结构化 old/new diagnostic value。

事件同时私有保留 erased typed old/new value 与对应 equality function，供跨多次事件的
coalescing 判断第一 old 与最后 new 是否相等。诊断值不参与身份、匹配或等价判定。

### 3. Behavior 与 coalescing

Animation Trigger 通过 typed key 构造 property selector：

```text
AnimationTrigger::property_changed(ENABLED)
AnimationTrigger::state_changed(ENABLED)
```

`state_changed` 只接受实现 `DiscretePropertyValue` 的值类型。Behavior、shown/hidden
映射和 coalescing 使用 `ErasedPropertyKey`，不得读取属性显示名称。

同一稳定事务的 property fact key 为 `(FigureId, ErasedPropertyKey)`。同名不同类型、
同名不同 namespace 都是不同属性。

### 4. 扩展边界

外部 Figure 可以：

1. 声明自己的 `PropertyKey<V>` 常量；
2. 为本地值类型实现 `PropertyValueType`；
3. 通过 `EventContext::emit_property_change` 发出 typed change；
4. 用同一 key 安装 Animation Behavior。

Core 不维护全局 property registry，不使用字符串反射，也不要求修改中心枚举。

## 失败与兼容

- 本次属于 0.1 breaking migration，不保留字符串 Trigger 或公开字段转发壳；
- 相同 namespace/name 但不同 Rust 类型不会匹配或合并；
- `PropertyValue` 只承担诊断/观察，不恢复成行为裁判；
- `TypeId` 不用于持久化或跨进程协议；持久化诊断使用 namespace/name/type name；
- 旧字符串常量改为 typed key，调用方必须显式擦除或使用 typed API。

## 验证

- compile/runtime contract：同 key typed 构造与外部 key consumer；
- negative compile contract：key/value 类型不匹配不能编译；
- coalescing：同 identity 合并，不同 namespace/type 不合并；
- Behavior：不比较显示字符串，shown/hidden 使用 `VISIBLE` key；
- Listener/Inspector：可读取稳定显示信息和结构化值；
- `core.tc04-typed-property-identity`、docs、Clippy 与 quick gate。
