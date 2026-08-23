//! Border 布局器
//!
//! 参考 draw2d: BorderLayout
//! 将容器划分为北、南、东、西、中五个区域。

use super::LayoutContext;
use super::LayoutManager;
use crate::graph::BlockId;
use novadraw_geometry::Rectangle;

/// Border 布局区域
///
/// 对应 draw2d: BorderLayout.CENTER, NORTH, SOUTH, EAST, WEST
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum BorderRegion {
    /// 中心区域（默认）
    #[default]
    Center,
    /// 北部（顶部）
    North,
    /// 南部（底部）
    South,
    /// 东部（右侧）
    East,
    /// 西部（左侧）
    West,
}

impl BorderRegion {
    /// 从字符串解析区域
    pub fn from_name(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "north" | "n" => BorderRegion::North,
            "south" | "s" => BorderRegion::South,
            "east" | "e" => BorderRegion::East,
            "west" | "w" => BorderRegion::West,
            _ => BorderRegion::Center,
        }
    }
}

/// BorderLayout 为直接子节点定义的区域约束。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderConstraint {
    pub region: BorderRegion,
    pub size: Option<f64>,
}

impl BorderConstraint {
    pub fn new(region: BorderRegion) -> Self {
        Self { region, size: None }
    }

    pub fn with_size(region: BorderRegion, size: f64) -> Self {
        Self {
            region,
            size: Some(size.max(0.0)),
        }
    }
}

/// Border 布局器
///
/// 将容器划分为五个区域：北、南、东、西、中。
/// 子元素通过约束指定要放置的区域。
///
/// # 使用方式
///
/// 约束的 x, y 字段指定区域：
/// - x = 0, y = 0 → Center
/// - x = 0, y = -1 → North
/// - x = 0, y = 1 → South
/// - x = 1, y = 0 → East
/// - x = -1, y = 0 → West
///
/// 约束的 width, height 指定该区域的尺寸（可选）
#[derive(Debug, Clone)]
pub struct BorderLayout {
    /// 各个区域的默认尺寸
    north_height: f64,
    south_height: f64,
    west_width: f64,
    east_width: f64,
}

impl BorderLayout {
    /// 创建新的 BorderLayout
    pub fn new() -> Self {
        Self {
            north_height: 50.0,
            south_height: 50.0,
            west_width: 50.0,
            east_width: 50.0,
        }
    }

    /// 创建带有默认尺寸的 BorderLayout
    pub fn with_sizes(north: f64, south: f64, west: f64, east: f64) -> Self {
        Self {
            north_height: north,
            south_height: south,
            west_width: west,
            east_width: east,
        }
    }

    /// 从约束解析区域
    fn get_region(constraint: &Rectangle) -> BorderRegion {
        // 使用 width/height 作为区域标识
        // width < 0 → West, width > 0 → East
        // height < 0 → North, height > 0 → South
        // 默认为 Center
        if constraint.height < 0.0 {
            BorderRegion::North
        } else if constraint.height > 0.0 {
            BorderRegion::South
        } else if constraint.width < 0.0 {
            BorderRegion::West
        } else if constraint.width > 0.0 {
            BorderRegion::East
        } else {
            BorderRegion::Center
        }
    }
}

impl Default for BorderLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutManager for BorderLayout {
    fn get_preferred_size(
        &self,
        _container: BlockId,
        _w_hint: f64,
        _h_hint: f64,
        _ctx: &dyn LayoutContext,
    ) -> (f64, f64) {
        // 默认尺寸
        (
            self.west_width + self.east_width + 100.0,
            self.north_height + self.south_height + 100.0,
        )
    }

    fn get_minimum_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64) {
        self.get_preferred_size(container, w_hint, h_hint, ctx)
    }

    fn layout(&self, container: BlockId, ctx: &mut dyn LayoutContext) {
        let children = ctx.get_children(container);
        if children.is_empty() {
            return;
        }

        // 获取容器的 bounds
        let container_bounds = ctx.get_container_bounds(container);
        let cx = container_bounds.x;
        let cy = container_bounds.y;
        let cw = container_bounds.width;
        let ch = container_bounds.height;

        // 默认区域尺寸
        let north_h = self.north_height.min(ch * 0.3);
        let south_h = self.south_height.min(ch * 0.3);
        let west_w = self.west_width.min(cw * 0.3);
        let east_w = self.east_width.min(cw * 0.3);

        // 计算中心区域
        let center_x = cx + west_w;
        let center_y = cy + north_h;
        let center_w = cw - west_w - east_w;
        let center_h = ch - north_h - south_h;

        // 用于跟踪已分配给各个区域的元素
        let mut allocated_regions = [false; 5];

        // 第一次遍历：处理有明确约束的元素
        for (child_id, _current_bounds) in &children {
            if let Some((region, requested_size)) = border_constraint(ctx, *child_id) {
                let (x, y, w, h) = match region {
                    BorderRegion::North => {
                        allocated_regions[1] = true;
                        let h = requested_size.unwrap_or(north_h).min(ch * 0.5);
                        (cx, cy, cw, h)
                    }
                    BorderRegion::South => {
                        allocated_regions[2] = true;
                        let h = requested_size.unwrap_or(south_h).min(ch * 0.5);
                        (cx, cy + ch - h, cw, h)
                    }
                    BorderRegion::East => {
                        allocated_regions[3] = true;
                        let w = requested_size.unwrap_or(east_w).min(cw * 0.5);
                        (cx + cw - w, center_y, w, center_h)
                    }
                    BorderRegion::West => {
                        allocated_regions[4] = true;
                        let w = requested_size.unwrap_or(west_w).min(cw * 0.5);
                        (cx, center_y, w, center_h)
                    }
                    BorderRegion::Center => {
                        allocated_regions[0] = true;
                        (center_x, center_y, center_w, center_h)
                    }
                };

                ctx.set_child_bounds(*child_id, Rectangle::new(x, y, w, h));
            }
        }

        // 第二次遍历：处理没有约束的元素，放置到剩余区域
        let mut remaining_regions = Vec::new();
        for (i, allocated) in allocated_regions.iter().enumerate() {
            if !*allocated {
                remaining_regions.push(match i {
                    0 => BorderRegion::Center,
                    1 => BorderRegion::North,
                    2 => BorderRegion::South,
                    3 => BorderRegion::East,
                    4 => BorderRegion::West,
                    _ => BorderRegion::Center,
                });
            }
        }

        for (child_id, _) in &children {
            if ctx.get_constraint(*child_id).is_none()
                && let Some(region) = remaining_regions.pop()
            {
                let (x, y, w, h) = match region {
                    BorderRegion::North => (cx, cy, cw, north_h),
                    BorderRegion::South => (cx, cy + ch - south_h, cw, south_h),
                    BorderRegion::East => (cx + cw - east_w, center_y, east_w, center_h),
                    BorderRegion::West => (cx, center_y, west_w, center_h),
                    BorderRegion::Center => (center_x, center_y, center_w, center_h),
                };
                ctx.set_child_bounds(*child_id, Rectangle::new(x, y, w, h));
            }
        }
    }
}

fn border_constraint(
    ctx: &dyn LayoutContext,
    child_id: BlockId,
) -> Option<(BorderRegion, Option<f64>)> {
    let constraint = ctx.get_constraint(child_id)?;
    if let Some(constraint) = constraint.as_any().downcast_ref::<BorderConstraint>() {
        return Some((constraint.region, constraint.size));
    }
    constraint.as_any().downcast_ref::<Rectangle>().map(|rect| {
        let region = BorderLayout::get_region(rect);
        let size = match region {
            BorderRegion::North => (-rect.height).is_sign_positive().then_some(-rect.height),
            BorderRegion::South => (rect.height > 0.0).then_some(rect.height),
            BorderRegion::East => (rect.width > 0.0).then_some(rect.width),
            BorderRegion::West => (rect.width < 0.0).then_some(-rect.width),
            BorderRegion::Center => None,
        };
        (region, size)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_border_region_from_str() {
        assert_eq!(BorderRegion::from_name("north"), BorderRegion::North);
        assert_eq!(BorderRegion::from_name("N"), BorderRegion::North);
        assert_eq!(BorderRegion::from_name("south"), BorderRegion::South);
        assert_eq!(BorderRegion::from_name("S"), BorderRegion::South);
        assert_eq!(BorderRegion::from_name("east"), BorderRegion::East);
        assert_eq!(BorderRegion::from_name("E"), BorderRegion::East);
        assert_eq!(BorderRegion::from_name("west"), BorderRegion::West);
        assert_eq!(BorderRegion::from_name("W"), BorderRegion::West);
        assert_eq!(BorderRegion::from_name("center"), BorderRegion::Center);
        assert_eq!(BorderRegion::from_name("unknown"), BorderRegion::Center);
    }

    #[test]
    fn test_border_layout_creation() {
        let layout = BorderLayout::new();
        // 默认尺寸应该设置正确
        let (w, h) = layout.get_preferred_size(
            BlockId::from(slotmap::KeyData::from_ffi(0)),
            800.0,
            600.0,
            &MockLayoutContext::new(),
        );
        // 默认尺寸 = west + east + 100 = 50 + 50 + 100 = 200
        // 高度 = north + south + 100 = 50 + 50 + 100 = 200
        assert!(w > 0.0);
        assert!(h > 0.0);
    }

    #[test]
    fn test_border_layout_with_sizes() {
        let layout = BorderLayout::with_sizes(80.0, 80.0, 100.0, 100.0);
        assert_eq!(layout.north_height, 80.0);
        assert_eq!(layout.south_height, 80.0);
        assert_eq!(layout.west_width, 100.0);
        assert_eq!(layout.east_width, 100.0);
    }
}

/// Mock LayoutContext for testing
#[cfg(test)]
struct MockLayoutContext {
    children: Vec<(BlockId, Rectangle)>,
    container_bounds: Rectangle,
}

#[cfg(test)]
impl MockLayoutContext {
    fn new() -> Self {
        Self {
            children: Vec::new(),
            container_bounds: Rectangle::new(0.0, 0.0, 800.0, 600.0),
        }
    }
}

#[cfg(test)]
impl super::LayoutContext for MockLayoutContext {
    fn get_children(&self, _parent_id: BlockId) -> Vec<(BlockId, Rectangle)> {
        self.children.clone()
    }

    fn get_constraint(&self, _child_id: BlockId) -> Option<&dyn super::LayoutConstraint> {
        None
    }

    fn get_preferred_size(&self, _block_id: BlockId, _w_hint: f64, _h_hint: f64) -> (f64, f64) {
        (100.0, 100.0)
    }

    fn set_child_bounds(&mut self, _child_id: BlockId, _bounds: Rectangle) {
        // No-op for testing
    }

    fn get_container_bounds(&self, _container_id: BlockId) -> Rectangle {
        self.container_bounds
    }
}
