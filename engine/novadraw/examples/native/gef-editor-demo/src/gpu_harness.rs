//! Keep upstream's input harness, but rasterize its public layer output on GPU.
use masonry_imaging::{ImageRenderer, vello};
use masonry_testing::{TestHarness as InputHarness, TestHarnessParams};
use std::ops::{Deref, DerefMut};
use xilem::dpi::PhysicalSize;
use xilem::masonry::{
    core::{
        DefaultProperties, FromDynWidget, Handled, NewWidget, PointerButton, Widget, WindowEvent,
    },
    imaging::{
        Painter,
        record::{Scene, replay_transformed},
    },
    kurbo::{Affine, Rect},
};

pub struct TestHarness<W: Widget + FromDynWidget> {
    input: InputHarness<W>,
    size: PhysicalSize<u32>,
    scale: f64,
    background: xilem::peniko::Color,
    padding: u32,
    renderer: Option<vello::Renderer>,
}
impl<W: Widget + FromDynWidget> Deref for TestHarness<W> {
    type Target = InputHarness<W>;
    fn deref(&self) -> &Self::Target {
        &self.input
    }
}
impl<W: Widget + FromDynWidget> DerefMut for TestHarness<W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.input
    }
}
impl<W: Widget + FromDynWidget> TestHarness<W> {
    pub fn mouse_button_press(&mut self, button: PointerButton) {
        self.input.mouse_button_press(Some(button));
    }
    pub fn mouse_button_release(&mut self, button: PointerButton) {
        self.input.mouse_button_release(Some(button));
    }
    pub fn create_with(
        properties: DefaultProperties,
        root: NewWidget<W>,
        params: TestHarnessParams,
    ) -> Self {
        let size = params.window_size;
        let scale = params.scale_factor;
        let background = params.background_color;
        let padding = params.root_padding;
        Self {
            input: InputHarness::create_with(properties, root, params),
            size,
            scale,
            background,
            padding,
            renderer: None,
        }
    }
    pub fn create_with_size(
        properties: DefaultProperties,
        root: NewWidget<W>,
        size: PhysicalSize<u32>,
    ) -> Self {
        Self::create_with(properties, root, TestHarnessParams::DEFAULT.with_size(size))
    }
    pub fn process_window_event(&mut self, event: WindowEvent) -> Handled {
        match event {
            WindowEvent::Resize(size) => self.size = size,
            WindowEvent::Rescale(scale) => {
                self.scale = scale;
                // Same host contract as host::request_frame, on the harness's public tree.
                let mut ids = Vec::new();
                self.input.inspect_widgets(|widget| ids.push(widget.id()));
                for id in ids {
                    self.input
                        .edit_widget_with_id(id, |mut widget| widget.ctx.request_anim_frame());
                }
            }
            _ => {}
        }
        self.input.process_window_event(event)
    }
    pub fn render(&mut self) -> image::RgbaImage {
        assert!(
            std::env::var("SKIP_RENDER_TESTS").is_err(),
            "GPU evidence cannot be skipped"
        );
        let (layers, _) = self.input.redraw();
        let width = self.size.width.max(1) + self.padding * 2;
        let height = self.size.height.max(1) + self.padding * 2;
        let mut logical = Scene::new();
        layers.replay_into(&mut logical);
        let mut physical = Scene::new();
        Painter::new(&mut physical).fill_rect(
            Rect::new(0.0, 0.0, f64::from(width), f64::from(height)),
            self.background,
        );
        replay_transformed(
            &logical,
            &mut physical,
            Affine::translate((f64::from(self.padding), f64::from(self.padding)))
                * Affine::scale(self.scale),
        );
        let renderer = self.renderer.get_or_insert_with(|| {
            assert_eq!(vello::BACKEND_NAME, "imaging_vello");
            vello::new_headless_renderer().expect("real GPU is required")
        });
        let image = renderer
            .render_source(&mut physical, width, height)
            .expect("GPU rasterization failed");
        image::RgbaImage::from_vec(image.width, image.height, image.data).unwrap()
    }
}
