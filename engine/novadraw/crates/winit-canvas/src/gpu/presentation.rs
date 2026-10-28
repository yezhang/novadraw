//! Window surface presentation policy. Applications own the surface and call
//! this after creation/recovery and around drawable acquisition/presentation.

/// Keep the previous drawable unscaled at the top-left during native resize.
/// The background fills newly exposed window pixels before the next frame.
/// This is a no-op on platforms other than macOS/Metal.
pub fn configure_surface(surface: &wgpu::Surface<'_>, background: wgpu::Color) {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: the HAL guard keeps the Metal surface alive while accessing
        // its layer; callers use this on the window/event-loop thread.
        let Some(surface) = (unsafe { surface.as_hal::<wgpu::hal::api::Metal>() }) else {
            return;
        };
        let layer = surface.render_layer().lock();
        layer.setContentsGravity(unsafe { objc2_quartz_core::kCAGravityTopLeft });
        layer.setOpaque(background.a == 1.0);
        let color = objc2_core_graphics::CGColor::new_srgb(
            background.r,
            background.g,
            background.b,
            background.a,
        );
        layer.setBackgroundColor(Some(&color));
        layer.setPresentsWithTransaction(false);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (surface, background);
}

fn set_transactional(surface: &wgpu::Surface<'_>, enabled: bool) {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: see configure_surface.
        if let Some(surface) = unsafe { surface.as_hal::<wgpu::hal::api::Metal>() } {
            surface
                .render_layer()
                .lock()
                .setPresentsWithTransaction(enabled);
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (surface, enabled);
}

/// Align a resize drawable with the window's Core Animation transaction.
///
/// Create **before** `get_current_texture`, keep alive through `present`, then
/// drop. WGPU snapshots this setting at acquire time. Drop restores ordinary
/// presentation on success, early return, surface recovery and unwinding.
/// Use only on the window/event-loop thread. Non-Metal platforms are a no-op.
pub struct SurfacePresentation<'a, 'window> {
    surface: &'a wgpu::Surface<'window>,
    resize: bool,
}

impl<'a, 'window> SurfacePresentation<'a, 'window> {
    pub fn begin(surface: &'a wgpu::Surface<'window>, resize: bool) -> Self {
        if resize {
            set_transactional(surface, true);
        }
        Self { surface, resize }
    }
}

impl Drop for SurfacePresentation<'_, '_> {
    fn drop(&mut self) {
        if self.resize {
            set_transactional(self.surface, false);
        }
    }
}
