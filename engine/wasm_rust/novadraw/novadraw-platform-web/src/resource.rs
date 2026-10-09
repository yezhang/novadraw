use std::error::Error;
use std::fmt;

use js_sys::Uint8Array;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::Response;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebResourceError {
    WindowUnavailable,
    FetchRejected(String),
    InvalidResponse,
    HttpStatus { status: u16, status_text: String },
    BodyReadFailed(String),
}

impl fmt::Display for WebResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WindowUnavailable => formatter.write_str("browser window is unavailable"),
            Self::FetchRejected(message) => write!(formatter, "resource fetch failed: {message}"),
            Self::InvalidResponse => {
                formatter.write_str("resource fetch returned a non-Response value")
            }
            Self::HttpStatus {
                status,
                status_text,
            } => write!(
                formatter,
                "resource fetch returned HTTP {status} {status_text}"
            ),
            Self::BodyReadFailed(message) => {
                write!(
                    formatter,
                    "resource response body could not be read: {message}"
                )
            }
        }
    }
}

impl Error for WebResourceError {}

#[derive(Clone, Copy, Debug, Default)]
pub struct FetchResourceProvider;

impl FetchResourceProvider {
    pub const fn new() -> Self {
        Self
    }

    pub async fn load(&self, url: &str) -> Result<Vec<u8>, WebResourceError> {
        let window = web_sys::window().ok_or(WebResourceError::WindowUnavailable)?;
        let response = JsFuture::from(window.fetch_with_str(url))
            .await
            .map_err(|error| WebResourceError::FetchRejected(js_message(error)))?
            .dyn_into::<Response>()
            .map_err(|_| WebResourceError::InvalidResponse)?;
        if !response.ok() {
            return Err(WebResourceError::HttpStatus {
                status: response.status(),
                status_text: response.status_text(),
            });
        }
        let body = response
            .array_buffer()
            .map_err(|error| WebResourceError::BodyReadFailed(js_message(error)))?;
        let buffer = JsFuture::from(body)
            .await
            .map_err(|error| WebResourceError::BodyReadFailed(js_message(error)))?;
        Ok(Uint8Array::new(&buffer).to_vec())
    }
}

fn js_message(value: JsValue) -> String {
    value.as_string().unwrap_or_else(|| format!("{value:?}"))
}
