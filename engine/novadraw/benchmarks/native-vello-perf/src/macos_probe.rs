use objc2::{msg_send, runtime::AnyObject};
use objc2_core_graphics::{
    CGEvent, CGEventSource, CGEventSourceStateID, CGImage, CGPreflightScreenCaptureAccess,
    CGRectNull, CGWindowID, CGWindowImageOption, CGWindowListOption,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

#[allow(deprecated)]
use objc2_core_graphics::CGWindowListCreateImage;

const SPACE_VIRTUAL_KEY: u16 = 49;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelSignature([u8; 4]);

impl PixelSignature {
    pub fn bytes(self) -> [u8; 4] {
        self.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CapturedPixel {
    pub signature: PixelSignature,
    pub image_width: usize,
    pub image_height: usize,
    pub bits_per_pixel: usize,
}

pub struct WindowServerProbe {
    window_id: CGWindowID,
}

impl WindowServerProbe {
    pub fn new(window: &Window) -> Result<Self, String> {
        if !CGPreflightScreenCaptureAccess() {
            return Err(
                "screen capture permission is required for WindowServer visibility evidence"
                    .to_owned(),
            );
        }

        let handle = window
            .window_handle()
            .map_err(|error| format!("obtain AppKit window handle: {error}"))?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return Err("WindowServer probe requires an AppKit window".to_owned());
        };
        let ns_view = unsafe { handle.ns_view.cast::<AnyObject>().as_ref() };
        let ns_window: *mut AnyObject = unsafe { msg_send![ns_view, window] };
        let Some(ns_window) = (unsafe { ns_window.as_ref() }) else {
            return Err("AppKit view is not attached to an NSWindow".to_owned());
        };
        let window_number: isize = unsafe { msg_send![ns_window, windowNumber] };
        let window_id = u32::try_from(window_number)
            .map_err(|_| format!("invalid NSWindow number: {window_number}"))?;
        if window_id == 0 {
            return Err("NSWindow has no WindowServer identifier".to_owned());
        }
        Ok(Self { window_id })
    }

    pub fn window_id(&self) -> CGWindowID {
        self.window_id
    }

    pub fn capture_center_pixel(&self) -> Result<CapturedPixel, String> {
        let list_options =
            CGWindowListOption::OptionIncludingWindow | CGWindowListOption::OptionOnScreenOnly;
        let image_options =
            CGWindowImageOption::BoundsIgnoreFraming | CGWindowImageOption::BestResolution;
        #[allow(deprecated)]
        let image = CGWindowListCreateImage(
            unsafe { CGRectNull },
            list_options,
            self.window_id,
            image_options,
        )
        .ok_or_else(|| {
            "WindowServer did not return an image for the benchmark window".to_owned()
        })?;

        let image_width = CGImage::width(Some(&image));
        let image_height = CGImage::height(Some(&image));
        let bits_per_pixel = CGImage::bits_per_pixel(Some(&image));
        let bytes_per_row = CGImage::bytes_per_row(Some(&image));
        let bytes_per_pixel = bits_per_pixel.div_ceil(8);
        if image_width == 0 || image_height == 0 || bytes_per_pixel < 4 {
            return Err(format!(
                "unsupported WindowServer image: {image_width}x{image_height}, \
                 {bits_per_pixel} bits per pixel"
            ));
        }

        let provider = CGImage::data_provider(Some(&image))
            .ok_or_else(|| "WindowServer image has no data provider".to_owned())?;
        let data = objc2_core_graphics::CGDataProvider::data(Some(&provider))
            .ok_or_else(|| "copy WindowServer image bytes".to_owned())?;
        let data_length = usize::try_from(data.length())
            .map_err(|_| "WindowServer image byte length is invalid".to_owned())?;
        let x = image_width / 2;
        let y = image_height / 2;
        let offset = y
            .checked_mul(bytes_per_row)
            .and_then(|row| row.checked_add(x.checked_mul(bytes_per_pixel)?))
            .ok_or_else(|| "WindowServer center-pixel offset overflow".to_owned())?;
        let end = offset
            .checked_add(4)
            .ok_or_else(|| "WindowServer center-pixel range overflow".to_owned())?;
        if end > data_length {
            return Err(format!(
                "WindowServer center pixel exceeds image bytes: {end} > {data_length}"
            ));
        }
        let bytes = unsafe { std::slice::from_raw_parts(data.byte_ptr(), data_length) };
        Ok(CapturedPixel {
            signature: PixelSignature(bytes[offset..end].try_into().expect("four-byte slice")),
            image_width,
            image_height,
            bits_per_pixel,
        })
    }

    pub fn post_space_to_self(&self) -> Result<(), String> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .ok_or_else(|| "create CoreGraphics HID event source".to_owned())?;
        let key_down = CGEvent::new_keyboard_event(Some(&source), SPACE_VIRTUAL_KEY, true)
            .ok_or_else(|| "create synthetic Space key-down event".to_owned())?;
        let key_up = CGEvent::new_keyboard_event(Some(&source), SPACE_VIRTUAL_KEY, false)
            .ok_or_else(|| "create synthetic Space key-up event".to_owned())?;
        let process_id = std::process::id() as libc::pid_t;
        CGEvent::post_to_pid(process_id, Some(&key_down));
        CGEvent::post_to_pid(process_id, Some(&key_up));
        Ok(())
    }
}
