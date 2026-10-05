//! 渲染上下文
//!
//! 参考 HTML5 Canvas API 设计，生成可重放的渲染命令。

use crate::Color;
use crate::geometry::{Affine2D, Point, PointList, Rectangle};

use super::stroke::{DashPattern, GraphicsInputError, StrokeStyle};
use super::{ClipPath, FillRule, Paint};
use crate::render::command::{
    ImageDrawDisposition, ImageDrawError, Path, RenderCommand, RenderCommandKind,
    validate_image_draw_geometry,
};
use crate::render::submission::{DamageSet, RenderSubmission};
use crate::render::text::TextLayout;

#[derive(Clone, Debug)]
struct GraphicsState {
    font: crate::text::FontDescriptor,
    fill_paint: Option<Paint>,
    stroke_paint: Option<Paint>,
    stroke: StrokeStyle,
    fill_rule: FillRule,
    global_alpha: f64,
    transform: Affine2D,
    clip_depth: usize,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            font: crate::text::FontDescriptor::default(),
            fill_paint: None,
            stroke_paint: None,
            stroke: StrokeStyle::default(),
            fill_rule: FillRule::default(),
            global_alpha: 1.0,
            transform: Affine2D::IDENTITY,
            clip_depth: 0,
        }
    }
}

pub struct NdCanvas {
    damage: DamageSet,
    commands: Vec<RenderCommand>,
    /// 当前正在构建的路径（用于 begin_path/fill/stroke 流程）
    current_path: Option<Path>,
    state: GraphicsState,
    state_stack: Vec<GraphicsState>,
    recording_error: Option<crate::graphics::GraphicsError>,
    font_dependencies: Vec<crate::text::FontFaceRef>,
}

impl Default for NdCanvas {
    fn default() -> Self {
        Self::new()
    }
}

impl NdCanvas {
    pub fn new() -> Self {
        Self {
            damage: DamageSet::default(),
            commands: Vec::new(),
            current_path: None,
            state: GraphicsState::default(),
            state_stack: Vec::new(),
            recording_error: None,
            font_dependencies: Vec::new(),
        }
    }

    pub(crate) fn recording_error(&self) -> Option<&crate::graphics::GraphicsError> {
        self.recording_error.as_ref()
    }

    pub(crate) fn state_depth(&self) -> usize {
        self.state_stack.len()
    }

    pub(crate) fn stroke_paint(&self) -> Option<&Paint> {
        self.state.stroke_paint.as_ref()
    }

    pub(crate) fn validate_recording(
        &self,
        resources: &crate::ResourceRegistry,
    ) -> Result<(), crate::graphics::GraphicsError> {
        use crate::graphics::GraphicsError;
        if let Some(error) = &self.recording_error {
            return Err(error.clone());
        }
        for face in &self.font_dependencies {
            let resource = resources
                .snapshot_resource(face.resource_id())
                .map_err(GraphicsError::Resource)?;
            crate::text::outline::FontInstanceRef::new(face, &resource, &[])
                .map_err(GraphicsError::Font)?;
        }
        let mut depth = 0_usize;
        for command in &self.commands {
            match &command.kind {
                RenderCommandKind::PushState => depth += 1,
                RenderCommandKind::RestoreState if depth == 0 => {
                    return Err(GraphicsError::UnbalancedState);
                }
                RenderCommandKind::PopState => {
                    depth = depth.checked_sub(1).ok_or(GraphicsError::UnbalancedState)?;
                }
                RenderCommandKind::DrawGlyphRun { run, .. } => {
                    let resource = resources
                        .snapshot_resource(run.font.resource_id())
                        .map_err(GraphicsError::Resource)?;
                    crate::text::outline::FontInstanceRef::new(
                        &run.font,
                        &resource,
                        &run.normalized_coords,
                    )
                    .map_err(GraphicsError::Font)?;
                }
                _ => {}
            }
        }
        if depth != 0 {
            return Err(GraphicsError::UnbalancedState);
        }
        Ok(())
    }

    pub(crate) fn font(&self) -> &crate::text::FontDescriptor {
        &self.state.font
    }

    pub(crate) fn set_font(&mut self, font: crate::text::FontDescriptor) {
        self.state.font = font;
    }

    pub(crate) fn reject_recording(&mut self, error: crate::graphics::GraphicsError) {
        self.recording_error.get_or_insert(error);
    }

    /// Validates only this call's new commands and rolls back its state on failure.
    pub(crate) fn record_checked(
        &mut self,
        record: impl FnOnce(&mut Self),
    ) -> Result<(), crate::graphics::GraphicsError> {
        let start = self.commands.len();
        let state = self.state.clone();
        let path = self.current_path.clone();
        let damage = self.damage.clone();
        let dependencies = self.font_dependencies.len();
        record(self);
        let mut candidate = Vec::with_capacity(self.commands.len() - start + 1);
        candidate.push(RenderCommand {
            kind: RenderCommandKind::SetTransform {
                matrix: state.transform,
            },
        });
        candidate.extend_from_slice(&self.commands[start..]);
        let result = self.recording_error.clone().map_or_else(
            || {
                super::validate_graphics_input(&candidate, 1.0)
                    .map_err(|error| crate::graphics::GraphicsError::Input(error.reason))
            },
            Err,
        );
        if let Err(error) = result {
            self.commands.truncate(start);
            self.state = state;
            self.current_path = path;
            self.damage = damage;
            self.font_dependencies.truncate(dependencies);
            self.reject_recording(error.clone());
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn record_path(&mut self, path: &Path, stroke: bool) {
        let paint = if stroke {
            &self.state.stroke_paint
        } else {
            &self.state.fill_paint
        };
        if let Some(paint) = paint {
            let paint = paint.with_global_alpha(self.state.global_alpha);
            if paint.is_visible() {
                let kind = if stroke {
                    RenderCommandKind::StrokePath {
                        path: path.clone(),
                        paint,
                        stroke: self.state.stroke.clone(),
                    }
                } else {
                    RenderCommandKind::FillPath {
                        path: path.clone(),
                        paint,
                        rule: self.state.fill_rule,
                    }
                };
                self.create_command(kind);
            }
        }
    }

    fn create_command(&mut self, kind: RenderCommandKind) {
        if self.damage.is_empty() {
            self.damage.set_full();
        }
        let command = RenderCommand { kind };
        self.commands.push(command);
    }

    fn color_with_global_alpha(&self, color: Color) -> Color {
        color.with_alpha((color.alpha() * self.state.global_alpha).clamp(0.0, 1.0))
    }

    /// 保存当前状态（压栈）
    ///
    /// 对应 Draw2D: Graphics.pushState()
    /// 将当前状态复制并压入状态栈
    pub fn push_state(&mut self) {
        self.state_stack.push(self.state.clone());
        self.create_command(RenderCommandKind::PushState);
    }

    /// 恢复到最近一次 pushState 的状态（不弹出栈）
    ///
    /// 对应 Draw2D: Graphics.restoreState()
    /// 用于在 paintFigure 之后、paintChildren 之前恢复裁剪区
    pub fn restore_state(&mut self) {
        if let Some(saved) = self.state_stack.last() {
            self.state = saved.clone();
        }
        self.create_command(RenderCommandKind::RestoreState);
    }

    /// 弹出并恢复状态
    ///
    /// 对应 Draw2D: Graphics.popState()
    /// 用于在所有绘制完成后恢复 pushState 前的状态
    pub fn pop_state(&mut self) {
        if let Some(saved) = self.state_stack.pop() {
            self.state = saved;
        }
        self.create_command(RenderCommandKind::PopState);
    }

    /// 平移
    ///
    /// 生成 ConcatTransform 命令
    pub fn translate(&mut self, x: f64, y: f64) {
        let t = Affine2D::from_translation(x, y);
        self.state.transform = self.state.transform.post_concat(t);
        self.create_command(RenderCommandKind::ConcatTransform { matrix: t });
    }

    /// 按角度旋转
    ///
    /// 与 Draw2D `Graphics::rotate` 一致，参数单位为度。
    /// 生成 ConcatTransform 命令
    pub fn rotate(&mut self, degrees: f64) {
        let t = Affine2D::from_rotation(degrees.to_radians());
        self.state.transform = self.state.transform.post_concat(t);
        self.create_command(RenderCommandKind::ConcatTransform { matrix: t });
    }

    /// 缩放
    ///
    /// 生成 ConcatTransform 命令
    pub fn scale(&mut self, x: f64, y: f64) {
        let t = Affine2D::from_scale(x, y);
        self.state.transform = self.state.transform.post_concat(t);
        self.create_command(RenderCommandKind::ConcatTransform { matrix: t });
    }

    pub fn transform(&mut self, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) {
        let t = Affine2D::new(a, b, c, d, e, f);
        self.state.transform = self.state.transform.post_concat(t);
        self.create_command(RenderCommandKind::ConcatTransform { matrix: t });
    }

    pub fn set_transform(&mut self, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) {
        let t = Affine2D::new(a, b, c, d, e, f);
        self.state.transform = t;
        self.create_command(RenderCommandKind::SetTransform { matrix: t });
    }

    pub fn reset_transform(&mut self) {
        self.state.transform = Affine2D::IDENTITY;
        self.create_command(RenderCommandKind::ResetTransform);
    }

    pub fn clear_rect(&mut self, x: f64, y: f64, width: f64, height: f64, color: Color) {
        let rect = Rectangle::new(x, y, width, height);
        let color = self.color_with_global_alpha(color);
        self.create_command(RenderCommandKind::ClearRect { rect, color });
    }

    pub fn fill_rect_with_color(&mut self, x: f64, y: f64, width: f64, height: f64, color: Color) {
        self.fill_rect_with_paint(x, y, width, height, color.into(), FillRule::NonZero);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fill_rect_with_paint(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        paint: Paint,
        rule: FillRule,
    ) {
        let rect = Rectangle::new(x, y, width, height);
        let paint = paint.with_global_alpha(self.state.global_alpha);
        self.create_command(RenderCommandKind::FillRect { rect, paint, rule });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn stroke_rect_with_style(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        paint: impl Into<Paint>,
        stroke: StrokeStyle,
    ) {
        let rect = Rectangle::new(x, y, width, height);
        let paint = paint.into().with_global_alpha(self.state.global_alpha);
        self.create_command(RenderCommandKind::StrokeRect {
            rect,
            paint,
            stroke,
        });
    }

    pub fn fill_rectangle(&mut self, x: f64, y: f64, width: f64, height: f64) {
        if let Some(paint) = self.state.fill_paint.clone() {
            self.fill_rect_with_paint(x, y, width, height, paint, self.state.fill_rule);
        }
    }

    pub fn draw_rectangle(&mut self, x: f64, y: f64, width: f64, height: f64) {
        if let Some(paint) = self.state.stroke_paint.clone() {
            self.stroke_rect_with_style(x, y, width, height, paint, self.state.stroke.clone());
        }
    }

    /// 绘制椭圆
    ///
    /// 椭圆中心为 (cx, cy)，x 轴半径 rx，y 轴半径 ry
    #[allow(clippy::too_many_arguments)]
    pub fn ellipse_with_style(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        fill_color: Option<Color>,
        stroke_color: Option<Color>,
        stroke: StrokeStyle,
    ) {
        self.ellipse_with_paints(
            cx,
            cy,
            rx,
            ry,
            fill_color.map(Paint::from),
            stroke_color.map(Paint::from),
            stroke,
            FillRule::NonZero,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ellipse_with_paints(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        fill_paint: Option<Paint>,
        stroke_paint: Option<Paint>,
        stroke: StrokeStyle,
        rule: FillRule,
    ) {
        let fill_paint = fill_paint.map(|paint| paint.with_global_alpha(self.state.global_alpha));
        let stroke_paint =
            stroke_paint.map(|paint| paint.with_global_alpha(self.state.global_alpha));
        self.create_command(RenderCommandKind::Ellipse {
            cx,
            cy,
            rx,
            ry,
            fill_paint,
            stroke_paint,
            stroke,
            rule,
        });
    }

    pub fn fill_oval(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.ellipse_with_paints(
            x + width / 2.0,
            y + height / 2.0,
            width / 2.0,
            height / 2.0,
            self.state.fill_paint.clone(),
            None,
            self.state.stroke.clone(),
            self.state.fill_rule,
        );
    }

    pub fn draw_oval(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.ellipse_with_paints(
            x + width / 2.0,
            y + height / 2.0,
            width / 2.0,
            height / 2.0,
            None,
            self.state.stroke_paint.clone(),
            self.state.stroke.clone(),
            self.state.fill_rule,
        );
    }

    /// 绘制直线
    ///
    /// 从 p1 到 p2 的直线
    pub fn line_with_style(
        &mut self,
        p1: Point,
        p2: Point,
        paint: impl Into<Paint>,
        stroke: StrokeStyle,
    ) {
        let paint = paint.into().with_global_alpha(self.state.global_alpha);
        self.create_command(RenderCommandKind::Line {
            p1,
            p2,
            paint,
            stroke,
        });
    }

    /// 绘制折线
    ///
    /// 从 `points[0]` 到 `points[1]` ... 到 `points[n]` 的折线
    pub fn polyline_with_style(
        &mut self,
        points: &[Point],
        paint: impl Into<Paint>,
        stroke: StrokeStyle,
    ) {
        if points.len() < 2 {
            return;
        }
        let paint = paint.into().with_global_alpha(self.state.global_alpha);
        self.create_command(RenderCommandKind::Polyline {
            points: PointList::from_points(points.to_vec()),
            paint,
            stroke,
        });
    }

    pub fn draw_polygon(&mut self, points: &[Point]) {
        if points.len() < 2 {
            return;
        }
        let Some(paint) = self.state.stroke_paint.clone() else {
            return;
        };
        let mut closed = points.to_vec();
        if points.first() != points.last() {
            closed.push(points[0]);
        }
        self.polyline_with_style(&closed, paint, self.state.stroke.clone());
    }

    pub fn fill_polygon(&mut self, points: &[Point]) {
        if points.len() < 3 {
            return;
        }
        self.begin_path();
        self.move_to(points[0].x(), points[0].y());
        for point in &points[1..] {
            self.line_to(point.x(), point.y());
        }
        self.close_path();
        self.fill();
    }

    /// 开始构建路径
    pub fn begin_path(&mut self) {
        self.current_path = Some(Path::new());
    }

    /// 闭合路径
    pub fn close_path(&mut self) {
        if let Some(ref mut path) = self.current_path {
            path.close();
        }
    }

    /// 移动到指定点（路径起点）
    pub fn move_to(&mut self, x: f64, y: f64) {
        if let Some(ref mut path) = self.current_path {
            path.move_to(x, y);
        }
    }

    /// 直线连接到指定点
    pub fn line_to(&mut self, x: f64, y: f64) {
        if let Some(ref mut path) = self.current_path {
            path.line_to(x, y);
        }
    }

    /// 添加矩形路径
    pub fn rect_path(&mut self, x: f64, y: f64, width: f64, height: f64) {
        if let Some(ref mut path) = self.current_path {
            path.rect(x, y, width, height);
        }
    }

    /// Adds a circular arc using Draw2D-compatible degree angles.
    pub fn arc(
        &mut self,
        x: f64,
        y: f64,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        anticlockwise: bool,
    ) {
        if let Some(ref mut path) = self.current_path {
            path.arc(x, y, radius, start_angle, end_angle, anticlockwise);
        }
    }

    /// 二次贝塞尔曲线
    pub fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        if let Some(ref mut path) = self.current_path {
            path.quad_to(cpx, cpy, x, y);
        }
    }

    /// 三次贝塞尔曲线
    pub fn bezier_curve_to(&mut self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) {
        if let Some(ref mut path) = self.current_path {
            path.cubic_to(cp1x, cp1y, cp2x, cp2y, x, y);
        }
    }

    /// 填充当前路径
    #[allow(clippy::collapsible_if)]
    pub fn fill(&mut self) {
        if let Some(path) = self.current_path.take() {
            if let Some(paint) = &self.state.fill_paint {
                let paint = paint.with_global_alpha(self.state.global_alpha);
                // 跳过完全透明的颜色
                if paint.is_visible() {
                    self.create_command(RenderCommandKind::FillPath {
                        path,
                        paint,
                        rule: self.state.fill_rule,
                    });
                }
            }
        }
    }

    /// 描边当前路径
    #[allow(clippy::collapsible_if)]
    pub fn stroke(&mut self) {
        if let Some(path) = self.current_path.take() {
            if let Some(paint) = &self.state.stroke_paint {
                let paint = paint.with_global_alpha(self.state.global_alpha);
                self.create_command(RenderCommandKind::StrokePath {
                    path,
                    paint,
                    stroke: self.state.stroke.clone(),
                });
            }
        }
    }

    /// 填充并描边当前路径
    pub fn fill_and_stroke(&mut self) {
        if let Some(path) = self.current_path.take() {
            if let Some(paint) = &self.state.fill_paint {
                let paint = paint.with_global_alpha(self.state.global_alpha);
                self.create_command(RenderCommandKind::FillPath {
                    path: path.clone(),
                    paint,
                    rule: self.state.fill_rule,
                });
            }
            if let Some(paint) = &self.state.stroke_paint {
                let paint = paint.with_global_alpha(self.state.global_alpha);
                self.create_command(RenderCommandKind::StrokePath {
                    path,
                    paint,
                    stroke: self.state.stroke.clone(),
                });
            }
        }
    }

    pub fn set_fill_rule(&mut self, rule: FillRule) {
        self.state.fill_rule = rule;
    }

    pub fn clip_path(&mut self, clip: &ClipPath) {
        self.state.clip_depth += 1;
        self.create_command(RenderCommandKind::ClipPath { clip: clip.clone() });
    }

    pub fn clip_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let rect = Rectangle::new(x, y, width, height);
        self.state.clip_depth += 1;
        self.create_command(RenderCommandKind::Clip { rect });
    }

    pub fn set_clip(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.reset_clip();
        self.clip_rect(x, y, width, height);
    }

    pub fn reset_clip(&mut self) {
        self.state.clip_depth = 0;
        self.create_command(RenderCommandKind::ResetClip);
    }

    pub fn clear_commands(&mut self) {
        self.commands.clear();
    }

    pub fn damage(&self) -> &DamageSet {
        &self.damage
    }

    pub fn damage_mut(&mut self) -> &mut DamageSet {
        &mut self.damage
    }

    pub fn commands(&self) -> &[RenderCommand] {
        &self.commands
    }

    pub fn to_submission(&self) -> RenderSubmission {
        self.to_submission_for_surface(Default::default())
    }

    pub fn to_submission_for_surface(
        &self,
        surface: crate::render::submission::SurfaceInfo,
    ) -> RenderSubmission {
        self.to_submission_for_frame(
            surface,
            Default::default(),
            crate::render::submission::FrameId::default(),
        )
    }

    pub fn to_submission_for_frame(
        &self,
        surface: crate::render::submission::SurfaceInfo,
        resources: crate::render::submission::ResourceDelta,
        frame_id: crate::render::submission::FrameId,
    ) -> RenderSubmission {
        self.to_submission_for_session(
            surface,
            crate::render::submission::ResourceSync::Delta(resources),
            crate::render::submission::BackendSessionId::default(),
            frame_id,
        )
    }

    pub fn to_submission_for_session(
        &self,
        surface: crate::render::submission::SurfaceInfo,
        resources: crate::render::submission::ResourceSync,
        session_id: crate::render::submission::BackendSessionId,
        frame_id: crate::render::submission::FrameId,
    ) -> RenderSubmission {
        RenderSubmission {
            commands: self.commands.clone(),
            damage: self.damage.clone(),
            resources,
            surface,
            session_id,
            frame_id,
        }
    }

    pub fn fill_style(&mut self, color: Color) {
        self.set_fill_paint(color.into());
    }

    pub fn set_fill_paint(&mut self, paint: Paint) {
        self.state.fill_paint = Some(paint);
    }

    pub fn set_background_color(&mut self, color: Color) {
        self.fill_style(color);
    }

    pub fn stroke_style(&mut self, color: Color) {
        self.set_stroke_paint(color.into());
    }

    pub fn set_stroke_paint(&mut self, paint: Paint) {
        self.state.stroke_paint = Some(paint);
    }

    pub fn set_foreground_color(&mut self, color: Color) {
        self.stroke_style(color);
    }

    pub fn set_stroke(&mut self, stroke: StrokeStyle) {
        self.state.stroke = stroke;
    }

    pub fn line_width(&mut self, width: f64) -> Result<(), GraphicsInputError> {
        self.state.stroke = self.state.stroke.with_width(width)?;
        Ok(())
    }

    pub fn set_line_width(&mut self, width: f64) -> Result<(), GraphicsInputError> {
        self.line_width(width)
    }

    pub fn set_miter_limit(&mut self, limit: f64) -> Result<(), GraphicsInputError> {
        self.state.stroke = self.state.stroke.with_miter_limit(limit)?;
        Ok(())
    }

    pub fn set_dash_offset(&mut self, offset: f64) -> Result<(), GraphicsInputError> {
        self.state.stroke = self.state.stroke.with_dash_offset(offset)?;
        Ok(())
    }

    pub fn set_dash_pattern(&mut self, pattern: DashPattern) {
        self.state.stroke = self.state.stroke.clone().with_dash_pattern(pattern);
    }

    pub fn line_cap(&mut self, cap: crate::render::command::LineCap) {
        self.state.stroke = self.state.stroke.clone().with_cap(cap);
    }

    pub fn line_join(&mut self, join: crate::render::command::LineJoin) {
        self.state.stroke = self.state.stroke.clone().with_join(join);
    }

    pub fn line_style(&mut self, style: crate::render::command::LineStyle) {
        self.set_dash_pattern(style.into());
    }

    pub fn set_line_style(&mut self, style: crate::render::command::LineStyle) {
        self.line_style(style);
    }

    #[deprecated(
        since = "0.1.0",
        note = "use fill_text_layout with fill paint, or Graphics::fill_text"
    )]
    pub fn draw_text_layout(&mut self, layout: &TextLayout, x: f64, y: f64) {
        let Some(paint) = &self.state.stroke_paint else {
            return;
        };
        let paint = paint.with_global_alpha(self.state.global_alpha);
        if paint.is_visible() {
            self.draw_glyph_runs(layout, x, y, crate::render::text::GlyphPaint::Fill(paint));
        }
    }

    pub fn fill_text_layout(&mut self, layout: &TextLayout, x: f64, y: f64) {
        let Some(paint) = &self.state.fill_paint else {
            return;
        };
        let paint = paint.with_global_alpha(self.state.global_alpha);
        if paint.is_visible() {
            self.draw_glyph_runs(layout, x, y, crate::render::text::GlyphPaint::Fill(paint));
        }
    }

    pub fn stroke_text_layout(&mut self, layout: &TextLayout, x: f64, y: f64) {
        let Some(paint) = &self.state.stroke_paint else {
            return;
        };
        let paint = paint.with_global_alpha(self.state.global_alpha);
        if paint.is_visible() {
            self.draw_glyph_runs(
                layout,
                x,
                y,
                crate::render::text::GlyphPaint::Stroke {
                    paint,
                    stroke: self.state.stroke.clone(),
                },
            );
        }
    }

    fn draw_glyph_runs(
        &mut self,
        layout: &TextLayout,
        x: f64,
        y: f64,
        paint: crate::render::text::GlyphPaint,
    ) {
        if let Some(outlines) = layout.outlines() {
            let path = match outlines.path_at(Point::new(x, y)) {
                Ok(path) => path,
                Err(error) => {
                    self.reject_recording(crate::graphics::GraphicsError::Font(error));
                    return;
                }
            };
            for run in layout.glyph_runs() {
                if !self.font_dependencies.contains(&run.font) {
                    self.font_dependencies.push(run.font.clone());
                }
            }
            if !path.operations().is_empty() {
                let kind = match paint {
                    crate::render::text::GlyphPaint::Fill(paint) => RenderCommandKind::FillPath {
                        path,
                        paint,
                        rule: FillRule::NonZero,
                    },
                    crate::render::text::GlyphPaint::Stroke { paint, stroke } => {
                        RenderCommandKind::StrokePath {
                            path,
                            paint,
                            stroke,
                        }
                    }
                };
                self.create_command(kind);
            }
            return;
        }
        for run in layout.glyph_runs() {
            if run.glyphs.is_empty() {
                continue;
            }
            self.create_command(RenderCommandKind::DrawGlyphRun {
                run: run.clone(),
                origin: Point::new(x, y),
                paint: paint.clone(),
            });
        }
    }

    pub fn draw_image(&mut self, image: crate::render::command::ImageResourceRef, x: f64, y: f64) {
        let (width, height) = image.logical_size();
        self.draw_image_with_size(image, x, y, width, height);
    }

    pub fn draw_image_with_size(
        &mut self,
        image: crate::render::command::ImageResourceRef,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) {
        if self.state.global_alpha <= 0.0 {
            return;
        }
        let source_rect = Rectangle::new(
            0.0,
            0.0,
            f64::from(image.width()),
            f64::from(image.height()),
        );
        let dest_rect = Rectangle::new(x, y, width, height);
        self.create_command(RenderCommandKind::Image {
            image,
            source_rect,
            dest_rect,
            alpha: self.state.global_alpha,
        });
    }

    /// Draws a physical-pixel source region into a logical destination rectangle.
    ///
    /// Zero-sized source or destination rectangles and zero global alpha are successful no-ops.
    ///
    /// # Errors
    ///
    /// Returns [`ImageDrawError`] when either rectangle is non-finite, has a negative extent, or
    /// when a non-empty source rectangle lies outside the image's physical pixel bounds.
    pub fn draw_image_region(
        &mut self,
        image: crate::render::command::ImageResourceRef,
        source_rect: Rectangle,
        dest_rect: Rectangle,
    ) -> Result<(), ImageDrawError> {
        if validate_image_draw_geometry(image.width(), image.height(), source_rect, dest_rect)?
            == ImageDrawDisposition::NoOp
        {
            return Ok(());
        }
        if self.state.global_alpha <= 0.0 {
            return Ok(());
        }

        self.create_command(RenderCommandKind::Image {
            image,
            source_rect,
            dest_rect,
            alpha: self.state.global_alpha,
        });
        Ok(())
    }

    pub fn global_alpha(&mut self, alpha: f64) {
        let alpha = alpha.clamp(0.0, 1.0);
        self.state.global_alpha = alpha;
        self.create_command(RenderCommandKind::SetGlobalAlpha { alpha });
    }

    pub fn set_alpha(&mut self, alpha: f64) {
        self.global_alpha(alpha);
    }

    pub fn clip_depth(&self) -> usize {
        self.state.clip_depth
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::command::LineJoin;

    fn assert_transform_eq(actual: Affine2D, expected: Affine2D) {
        let actual = actual.coeffs();
        let expected = expected.coeffs();
        assert_eq!(actual, expected);
    }

    #[test]
    fn rotate_uses_draw2d_degree_units() {
        let mut canvas = NdCanvas::new();

        canvas.rotate(90.0);

        let RenderCommandKind::ConcatTransform { matrix } = canvas.commands()[0].kind else {
            panic!("expected rotate as ConcatTransform");
        };
        let point = matrix.transform_point(Point::new(1.0, 0.0));

        assert!(point.x().abs() < 1e-10, "expected x=0, got {}", point.x());
        assert!(
            (point.y() - 1.0).abs() < 1e-10,
            "expected y=1, got {}",
            point.y()
        );
    }

    #[test]
    fn arc_honors_clockwise_and_anticlockwise_sweeps() {
        fn recorded_bounds(anticlockwise: bool) -> crate::geometry::Rectangle {
            let mut canvas = NdCanvas::new();
            canvas.stroke_style(Color::BLACK);
            canvas.begin_path();
            canvas.arc(0.0, 0.0, 10.0, 0.0, 90.0, anticlockwise);
            canvas.stroke();

            let RenderCommandKind::StrokePath { path, .. } =
                &canvas.commands().last().expect("stroke command").kind
            else {
                panic!("expected StrokePath");
            };
            path.bounding_box().expect("arc path bounds")
        }

        let clockwise = recorded_bounds(false);
        let anticlockwise = recorded_bounds(true);

        assert!(clockwise.x >= -1e-9);
        assert!(clockwise.y >= -1e-9);
        assert!((clockwise.width - 10.0).abs() < 1e-6);
        assert!((clockwise.height - 10.0).abs() < 1e-6);
        assert!((anticlockwise.x + 10.0).abs() < 1e-6);
        assert!((anticlockwise.y + 10.0).abs() < 1e-6);
        assert!((anticlockwise.width - 20.0).abs() < 1e-6);
        assert!((anticlockwise.height - 20.0).abs() < 1e-6);
    }

    #[test]
    fn graphics_state_stack_restores_nested_clip_transform_and_stroke_state() {
        let mut canvas = NdCanvas::new();
        canvas.translate(10.0, 0.0);
        canvas.clip_rect(0.0, 0.0, 100.0, 100.0);
        canvas.line_width(2.0).unwrap();

        canvas.push_state();
        canvas.scale(2.0, 2.0);
        canvas.clip_rect(10.0, 10.0, 20.0, 20.0);
        canvas.line_width(7.0).unwrap();

        assert_eq!(canvas.clip_depth(), 2);

        canvas.restore_state();
        assert_eq!(canvas.clip_depth(), 1);

        canvas.stroke_style(Color::rgba(1.0, 0.0, 0.0, 1.0));
        canvas.line_join(LineJoin::Bevel);
        canvas.begin_path();
        canvas.move_to(0.0, 0.0);
        canvas.line_to(10.0, 10.0);
        canvas.stroke();

        let stroke = canvas.commands().last().expect("stroke command");
        let RenderCommandKind::StrokePath { stroke, .. } = &stroke.kind else {
            panic!("expected StrokePath after restored state");
        };
        assert_eq!(stroke.width(), 2.0);
        assert_eq!(stroke.join(), LineJoin::Bevel);

        canvas.pop_state();
        assert_eq!(canvas.clip_depth(), 1);
    }

    #[test]
    fn set_transform_and_reset_transform_emit_snapshot_commands() {
        let mut canvas = NdCanvas::new();

        canvas.translate(10.0, 20.0);
        canvas.set_transform(2.0, 0.0, 0.0, 2.0, 5.0, 6.0);
        canvas.reset_transform();

        let commands = canvas.commands();
        assert_eq!(commands.len(), 3);

        match commands[0].kind {
            RenderCommandKind::ConcatTransform { matrix } => {
                assert_transform_eq(matrix, Affine2D::from_translation(10.0, 20.0));
            }
            _ => panic!("expected translate as ConcatTransform"),
        }

        match commands[1].kind {
            RenderCommandKind::SetTransform { matrix } => {
                assert_transform_eq(matrix, Affine2D::new(2.0, 0.0, 0.0, 2.0, 5.0, 6.0));
            }
            _ => panic!("expected SetTransform"),
        }

        assert!(matches!(
            commands[2].kind,
            RenderCommandKind::ResetTransform
        ));
    }

    #[test]
    fn concat_transform_uses_parent_times_local_order() {
        let mut canvas = NdCanvas::new();

        canvas.scale(2.0, 3.0);
        canvas.translate(10.0, 20.0);

        assert_transform_eq(
            canvas.state.transform,
            Affine2D::from_scale(2.0, 3.0).post_concat(Affine2D::from_translation(10.0, 20.0)),
        );
        assert_eq!(
            canvas.state.transform.transform_point(Point::ORIGIN),
            Point::new(20.0, 60.0)
        );
    }

    #[test]
    fn first_command_promotes_unspecified_damage_to_full() {
        let mut canvas = NdCanvas::new();
        assert!(canvas.damage().is_empty());

        canvas.fill_rect_with_color(0.0, 0.0, 10.0, 10.0, Color::WHITE);

        assert!(canvas.damage().is_full());
    }

    #[test]
    fn clip_reset_and_restore_are_visible_in_command_snapshot() {
        let mut canvas = NdCanvas::new();

        canvas.push_state();
        canvas.clip_rect(0.0, 0.0, 100.0, 100.0);
        canvas.translate(5.0, 6.0);
        canvas.clip_rect(10.0, 10.0, 30.0, 40.0);
        canvas.restore_state();
        canvas.reset_clip();
        canvas.pop_state();

        assert_eq!(canvas.clip_depth(), 0);
        let kinds = canvas
            .commands()
            .iter()
            .map(|command| &command.kind)
            .collect::<Vec<_>>();

        assert!(matches!(kinds[0], RenderCommandKind::PushState));
        assert!(matches!(kinds[1], RenderCommandKind::Clip { .. }));
        assert!(matches!(
            kinds[2],
            RenderCommandKind::ConcatTransform { .. }
        ));
        assert!(matches!(kinds[3], RenderCommandKind::Clip { .. }));
        assert!(matches!(kinds[4], RenderCommandKind::RestoreState));
        assert!(matches!(kinds[5], RenderCommandKind::ResetClip));
        assert!(matches!(kinds[6], RenderCommandKind::PopState));
    }

    #[test]
    fn global_alpha_is_scoped_and_applied_to_shapes() {
        let mut canvas = NdCanvas::new();

        canvas.fill_style(Color::rgba(1.0, 0.0, 0.0, 0.8));
        canvas.global_alpha(0.5);
        canvas.fill_rectangle(0.0, 0.0, 10.0, 10.0);

        canvas.push_state();
        canvas.global_alpha(0.25);
        canvas.fill_rectangle(0.0, 0.0, 10.0, 10.0);
        canvas.pop_state();

        canvas.fill_rectangle(0.0, 0.0, 10.0, 10.0);

        let commands = canvas.commands();
        assert!(matches!(
            commands[0].kind,
            RenderCommandKind::SetGlobalAlpha { alpha } if alpha == 0.5
        ));

        let RenderCommandKind::FillRect {
            paint: Paint::Solid(color),
            ..
        } = commands[1].kind
        else {
            panic!("expected FillRect");
        };
        assert_eq!(color.alpha(), 0.4);

        let RenderCommandKind::FillRect {
            paint: Paint::Solid(color),
            ..
        } = commands[4].kind
        else {
            panic!("expected FillRect");
        };
        assert_eq!(color.alpha(), 0.2);

        let RenderCommandKind::FillRect {
            paint: Paint::Solid(color),
            ..
        } = commands[6].kind
        else {
            panic!("expected restored FillRect");
        };
        assert_eq!(color.alpha(), 0.4);
    }

    #[test]
    fn draw_image_records_destination_and_alpha_snapshot() {
        let mut canvas = NdCanvas::new();
        let image = crate::render::command::ImageResourceRef::new(
            crate::ResourceId::new(uuid::Uuid::nil(), 1),
            3,
            20,
            10,
            2.0,
        );

        canvas.global_alpha(0.5);
        canvas.draw_image(image, 4.0, 5.0);
        canvas.draw_image_with_size(image, 10.0, 20.0, 30.0, 40.0);

        let RenderCommandKind::Image {
            image,
            source_rect,
            dest_rect,
            alpha,
        } = canvas.commands()[1].kind
        else {
            panic!("expected Image");
        };
        assert_eq!(image.width(), 20);
        assert_eq!(image.revision(), 3);
        assert_eq!(source_rect, Rectangle::new(0.0, 0.0, 20.0, 10.0));
        assert_eq!(dest_rect, Rectangle::new(4.0, 5.0, 10.0, 5.0));
        assert_eq!(alpha, 0.5);

        let RenderCommandKind::Image {
            dest_rect, alpha, ..
        } = canvas.commands()[2].kind
        else {
            panic!("expected second Image");
        };
        assert_eq!(dest_rect, Rectangle::new(10.0, 20.0, 30.0, 40.0));
        assert_eq!(alpha, 0.5);
    }
}
