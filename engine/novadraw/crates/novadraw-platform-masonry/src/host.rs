//! Native lifecycle bridge using unmodified Masonry's public event delegation.
use masonry::app::RenderRoot;
use masonry::core::WidgetRef;
use masonry_winit::app::{AppDriver, EventLoop, MasonryState, MasonryUserEvent, NewWindow};
use masonry_winit::winit::{
    application::ApplicationHandler,
    error::EventLoopError,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::ActiveEventLoop,
    window::WindowId,
};

/// Request one animation pass on all widgets in all current visual layers.
/// Used for DPI/exposure changes; no timer or global state is installed.
pub fn request_frame(root: &mut RenderRoot) {
    let (plan, _) = root.redraw();
    let mut ids = Vec::new();
    for layer in plan.layers {
        if let Some(widget) = root.get_widget(layer.widget_id) {
            collect_ids(widget, &mut ids);
        }
    }
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(*id));
    for id in ids {
        root.edit_widget(id, |mut widget| widget.ctx.request_anim_frame());
    }
}

fn collect_ids(
    widget: WidgetRef<'_, dyn masonry::core::Widget>,
    ids: &mut Vec<masonry::core::WidgetId>,
) {
    let mut pending = vec![(widget, 0)];
    while let Some((widget, depth)) = pending.pop() {
        ids.push(widget.id());
        if depth < MAX_TREE_DEPTH {
            pending.extend(
                widget
                    .children()
                    .into_iter()
                    .map(|child| (child, depth + 1)),
            );
        }
    }
}
const MAX_TREE_DEPTH: usize = 10_000;

/// Run Xilem's driver and windows with Canvas DPI/exposure invalidation.
pub fn run(
    event_loop: EventLoop,
    windows: Vec<NewWindow>,
    driver: impl AppDriver + 'static,
    properties: masonry::core::DefaultProperties,
) -> Result<(), EventLoopError> {
    let _ = masonry::app::try_init_tracing();
    let mut host = Host {
        state: MasonryState::new(event_loop.create_proxy(), windows, properties),
        driver,
    };
    event_loop.run_app(&mut host)
}

struct Host<D> {
    state: MasonryState,
    driver: D,
}
impl<D: AppDriver> ApplicationHandler<MasonryUserEvent> for Host<D> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.state.handle_resumed(event_loop, &mut self.driver);
        for root in self.state.roots() {
            request_frame(root);
        }
    }
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.state.handle_suspended(event_loop);
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        // Schedule before delegation so upstream drains the emitted redraw signals.
        // The ensuing animation callback reads the new scale after Rescale is handled.
        if matches!(
            event,
            WindowEvent::ScaleFactorChanged { .. } | WindowEvent::Occluded(false)
        ) {
            for root in self.state.roots() {
                request_frame(root);
            }
        }
        self.state
            .handle_window_event(event_loop, id, event, &mut self.driver);
    }
    fn device_event(&mut self, event_loop: &ActiveEventLoop, id: DeviceId, event: DeviceEvent) {
        self.state
            .handle_device_event(event_loop, id, event, &mut self.driver);
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: MasonryUserEvent) {
        self.state
            .handle_user_event(event_loop, event, &mut self.driver);
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.state.handle_about_to_wait(event_loop);
    }
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        self.state.handle_new_events(event_loop, cause);
    }
    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        self.state.handle_exiting(event_loop);
    }
    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        self.state.handle_memory_warning(event_loop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use masonry::app::{RenderRootOptions, WindowSizePolicy};
    use masonry::core::NewWidget;
    use masonry_testing::ModularWidget;
    use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

    #[test]
    fn host_rescale_animates_nested_widgets_once_without_idle_tick() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let child =
            NewWidget::new(ModularWidget::new(Rc::clone(&observed)).anim_frame_fn(
                |observed, ctx, _, _| observed.borrow_mut().push(ctx.scale_factor()),
            ));
        let parent = NewWidget::new(ModularWidget::new_parent(child));
        let mut root = RenderRoot::new(
            parent,
            |_| {},
            RenderRootOptions {
                default_properties: Arc::new(masonry::core::DefaultProperties::default()),
                use_system_fonts: false,
                size_policy: WindowSizePolicy::User,
                size: masonry::dpi::PhysicalSize::new(640, 480),
                scale_factor: 1.0,
                test_font: None,
            },
        );
        root.redraw();
        // Upstream schedules an initial animation for newly registered widgets.
        root.handle_window_event(masonry::core::WindowEvent::AnimFrame(
            Duration::from_millis(1),
        ));
        observed.borrow_mut().clear();
        assert!(!root.needs_anim());
        request_frame(&mut root);
        root.handle_window_event(masonry::core::WindowEvent::Rescale(2.0));
        assert!(root.needs_anim());
        root.handle_window_event(masonry::core::WindowEvent::AnimFrame(
            Duration::from_millis(1),
        ));
        assert_eq!(&*observed.borrow(), &[2.0]);
        assert!(!root.needs_anim());
        root.handle_window_event(masonry::core::WindowEvent::AnimFrame(
            Duration::from_millis(1),
        ));
        assert_eq!(&*observed.borrow(), &[2.0]);
    }
}
