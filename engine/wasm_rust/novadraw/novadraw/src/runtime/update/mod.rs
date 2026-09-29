//! Update Manager - 更新管理
//!
//! 管理场景图的更新流程，包括：
//! - 脏区域（dirty region）跟踪：记录需要重绘的区域
//! - 失效块（invalid block）队列：记录需要重新布局的块
//! - 两阶段更新：先布局（validation），再重绘（repaint）
//!
//! 参考 Eclipse Draw2D 的 DeferredUpdateManager 设计。
//!
//! # 更新流程
//!
//! ```text
//! repaint() ──────► add_dirty_region() ──► 脏区域队列
//!                                                          │
//! revalidate() ──► add_invalid_figure() ──► 失效块队列   │
//!                                                          ▼
//!                                             perform_update()
//!                                                  │
//!                    ┌──────────────────────────────┼──────────────────────────────┐
//!                    ▼                              ▼                              ▼
//!              Phase 1: Layout            Phase 2: Union Dirty Regions    Phase 3: Repaint
//!              (布局失效的块)              (合并所有脏区域)                (重绘脏区域)
//! ```
//!
//! # 架构设计
//!
//! [`UpdateManager`] 是 Runtime 内部的具体事务组件，负责跟踪失效与脏区，
//! 并驱动 FigureTree 完成 validation 和 recording。
//! `repair` 模块负责 DamageSet 写入与 repair phase 的脏区处理逻辑。

mod deferred;
mod listener;
mod repair;

pub use deferred::UpdateManager;
pub use listener::{
    ActionEvent, ActionListener, AncestorEvent, AncestorEventKind, AncestorListener,
    CoordinateListener, FigureEvent, FigureListener, LayoutEvent, LayoutEventKind, LayoutListener,
    ListenerDirective, ListenerId, ListenerScope, NotificationEffect, NotificationQueue,
    NotificationRecord, ObservationListener, PropertyChangeEvent, PropertyChangeListener,
    PropertyValue, StableQueryError, StableSceneQuery, UpdateEvent, UpdateListener,
    ValidatingListener,
};
