use crate::Config;
use anyhow::{Result, anyhow, bail};
use perfetto_sdk::{
    heap_buffer::HeapBuffer,
    pb_msg::{PbMsg, PbMsgWriter},
    producer::{Backends, Producer, ProducerInitArgsBuilder},
    protos::config::{
        data_source_config::DataSourceConfig,
        trace_config::{
            TraceConfig, TraceConfigBufferConfig, TraceConfigDataSource,
            TraceConfigIncrementalStateConfig,
        },
        track_event::track_event_config::TrackEventConfig,
    },
    tracing_session::TracingSession,
    track_event::{EventContext, ScopeGuard, TrackEvent, TrackEventDebugArg, TrackEventType},
};
use serde_json::Value;
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const TRACE_BUFFER_KB: u32 = 256 * 1024;
const PRODUCER_SHMEM_KB: u32 = 32 * 1024;
const INCREMENTAL_CLEAR_MS: u32 = 1000;
const FILE_ATTEMPTS: usize = 8;

perfetto_sdk::track_event_categories! {
    pub mod perfetto_te_ns {
        ("novadraw.host", "Research operations and backend acknowledgements", []),
    }
}

pub struct HostScope {
    _guard: Option<ScopeGuard<fn()>>,
    _name: Option<std::ffi::CString>,
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

/// Host-owned SDK session; retained export bytes are already encoded by Perfetto.
pub struct TraceSession {
    session: Option<TracingSession>,
    pending: Option<Vec<u8>>,
    enabled: bool,
    requests: bool,
    running: bool,
    activity: bool,
    metadata: Value,
}

impl TraceSession {
    pub fn new(config: &Config) -> Result<Self> {
        // Producer registration is process-wide in the official SDK. No engine state lives here.
        static INITIALIZED: OnceLock<Result<(), String>> = OnceLock::new();
        INITIALIZED
            .get_or_init(|| {
                Producer::init(
                    ProducerInitArgsBuilder::new()
                        .backends(Backends::IN_PROCESS)
                        .shmem_size_hint_kb(PRODUCER_SHMEM_KB)
                        .build(),
                );
                TrackEvent::init();
                novadraw::text::register_perfetto_categories()?;
                novadraw_backend_vello::register_perfetto_categories()?;
                perfetto_te_ns::register().map_err(|e| e.to_string())
            })
            .as_ref()
            .map_err(|e| anyhow!("{e}"))?;
        let mut trace = Self {
            session: None,
            pending: None,
            enabled: config.trace,
            requests: config.requests,
            running: false,
            activity: false,
            metadata: serde_json::json!({"kind":"config", "fixture":"text_flow_large_example",
                "profile":env!("RESEARCH_PROFILE"),"rustc":env!("RESEARCH_RUSTC"),
                "git":env!("RESEARCH_GIT"),"dirty":env!("RESEARCH_DIRTY"),
                "requests":config.requests,"headless":config.headless,
                "cycles":config.cycles,"replay":config.replay,"gpu_wait":config.gpu_wait,
                "buffer_kb":TRACE_BUFFER_KB,"producer_shmem_kb":PRODUCER_SHMEM_KB,
                "requested_damage":if config.natural_damage {"natural"} else {"full"},
                "scroll_replay":config.scroll_replay,"text_raster_cache":config.text_raster_cache,
                "raster_validation":config.raster_validation,"coarse_trace":config.coarse_trace,
                "offscreen_gpu":config.offscreen_gpu}),
        };
        trace.start()?;
        Ok(trace)
    }

    fn start(&mut self) -> Result<()> {
        if !self.enabled || self.session.is_some() {
            return Ok(());
        }
        let writer = PbMsgWriter::new();
        let buffer = HeapBuffer::new(writer.stream_writer());
        let mut msg = PbMsg::new(&writer).map_err(|e| anyhow!("{e}"))?;
        let mut config = TraceConfig { msg: &mut msg };
        config.set_buffers(|b: &mut TraceConfigBufferConfig| {
            b.set_size_kb(TRACE_BUFFER_KB);
        });
        config.set_incremental_state_config(|c: &mut TraceConfigIncrementalStateConfig| {
            c.set_clear_period_ms(INCREMENTAL_CLEAR_MS);
        });
        config.set_data_sources(|d: &mut TraceConfigDataSource| {
            d.set_config(|d: &mut DataSourceConfig| {
                d.set_name("track_event");
                d.set_track_event_config(|c: &mut TrackEventConfig| {
                    c.set_disabled_categories("*");
                    c.set_enabled_categories("novadraw.host");
                    if self.metadata["coarse_trace"] != true {
                        c.set_enabled_categories("novadraw.phase");
                    }
                    c.set_enabled_categories("novadraw.backend");
                    if self.requests {
                        c.set_enabled_categories("novadraw.glyph");
                    }
                    if self.requests {
                        c.set_enabled_categories("novadraw.request");
                    }
                });
            });
        });
        msg.finalize();
        let mut config = vec![0; writer.stream_writer().get_written_size()];
        buffer.copy_into(&mut config);
        let mut session = TracingSession::in_process().map_err(|e| anyhow!("{e}"))?;
        session.setup(&config);
        session.start_blocking();
        self.session = Some(session);
        self.running = true;
        Self::emit(&self.metadata);
        Ok(())
    }

    pub fn set_enabled(&mut self, enabled: bool) -> Result<()> {
        if !enabled {
            // Keep a stopped session available for export.
            if self.running {
                if let Some(session) = &mut self.session {
                    session.stop_blocking();
                }
                self.running = false;
            }
        } else if self.session.is_some() || self.pending.is_some() {
            bail!("export the stopped trace with F7 before starting another capture");
        }
        self.enabled = enabled;
        self.start()
    }

    pub fn has_records(&self) -> bool {
        self.activity || self.pending.is_some() || (!self.running && self.session.is_some())
    }

    pub fn push(&mut self, value: Value) {
        self.activity |= self.running;
        Self::emit(&value);
    }

    fn emit(value: &Value) {
        perfetto_sdk::track_event!(
            "novadraw.host",
            TrackEventType::Instant(c"host-record".as_ptr()),
            |ctx: &mut EventContext| {
                ctx.add_debug_arg("record", TrackEventDebugArg::String(&value.to_string()));
            }
        );
    }

    pub fn scope(&self, name: &'static str) -> HostScope {
        let mut owned_name = None;
        let guard = if self.enabled && self.running {
            let name = std::ffi::CString::new(name).expect("static scope name");
            perfetto_sdk::track_event!("novadraw.host", TrackEventType::SliceBegin(name.as_ptr()));
            owned_name = Some(name);
            Some(ScopeGuard::new(
                (|| perfetto_sdk::track_event!("novadraw.host", TrackEventType::SliceEnd))
                    as fn(),
            ))
        } else {
            None
        };
        HostScope {
            _guard: guard,
            _name: owned_name,
            _thread: Default::default(),
        }
    }

    pub fn export(&mut self, directory: &Path) -> Result<PathBuf> {
        if self.pending.is_none() {
            let mut session = self
                .session
                .take()
                .ok_or_else(|| anyhow!("no active Perfetto capture; use --trace or F8"))?;
            if self.running {
                session.stop_blocking();
            }
            self.running = false;
            let bytes = Arc::new(Mutex::new(Vec::new()));
            let output = bytes.clone();
            session
                .read_trace_blocking(move |data, _| output.lock().unwrap().extend_from_slice(data));
            self.pending = Some(std::mem::take(&mut *bytes.lock().unwrap()));
        }
        std::fs::create_dir_all(directory)?;
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        for suffix in 0..FILE_ATTEMPTS {
            let path = directory.join(format!(
                "textflow-{timestamp}-{}-{suffix}.pftrace",
                std::process::id()
            ));
            let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            };
            file.write_all(self.pending.as_ref().unwrap())?;
            file.flush()?;
            self.pending = None;
            self.activity = false;
            self.start()?;
            return Ok(path);
        }
        bail!("cannot allocate a fresh trace filename")
    }
}

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../../tests/support/perfetto.rs"]
mod decoder;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::{INITIAL_WIDTH, NARROW_WIDTH, Operation, Research, SCROLL_STEP};

    #[test]
    fn sdk_capture_operations_failure_retry_and_fresh_export() -> Result<()> {
        let directory =
            std::env::current_dir()?.join("target/verification/textflow-large-app/sdk-contract");
        std::fs::create_dir_all(&directory)?;
        let config = Config {
            headless: true,
            replay: true,
            cycles: 1,
            trace: true,
            requests: true,
            gpu_wait: false,
            exit_after_replay: false,
            screenshot: false,
            output: directory.clone(),
        };
        let mut trace = TraceSession::new(&config)?;
        let marker = trace.scope("perfetto-contract-marker");
        let mut research = Research::new(true);
        research.apply(Operation::Reset, &mut trace)?;
        let mut renderer = novadraw_backend_vello::scene::VelloSceneRenderer::new();
        let mut scene = novadraw_backend_vello::scene::Scene::new();
        research.render(
            &mut renderer.bind(&mut scene),
            crate::research::surface(INITIAL_WIDTH),
            "cpu_recorded",
            &mut trace,
        )?;
        use novadraw::render::{
            BackendCapabilities, RenderBackend, RenderDisposition, RenderFailure,
            RenderFailureDiagnostic, RenderResult, RenderSubmission,
        };
        struct Backend {
            capabilities: BackendCapabilities,
            retries: usize,
            fatal: bool,
        }
        impl RenderBackend for Backend {
            fn capabilities(&self) -> BackendCapabilities {
                self.capabilities
            }
            fn submit(&mut self, _: &RenderSubmission) -> RenderResult {
                if self.fatal {
                    Err(RenderFailure::Fatal(RenderFailureDiagnostic::new(
                        "sdk test failure",
                    )))
                } else if self.retries > 0 {
                    self.retries -= 1;
                    Ok(RenderDisposition::Retry)
                } else {
                    Ok(RenderDisposition::Presented)
                }
            }
            fn resize(&mut self, _: u32, _: u32, _: f64) {}
        }
        let mut backend = Backend {
            capabilities: renderer.bind(&mut scene).capabilities(),
            retries: 1,
            fatal: false,
        };
        research.render(
            &mut backend,
            crate::research::surface(INITIAL_WIDTH),
            "test_only",
            &mut trace,
        )?;
        backend.fatal = true;
        assert!(
            research
                .render(
                    &mut backend,
                    crate::research::surface(INITIAL_WIDTH),
                    "test_only",
                    &mut trace
                )
                .is_err()
        );
        research.apply(Operation::Reset, &mut trace)?;
        research.apply(Operation::Append, &mut trace)?;
        research.apply(Operation::Scroll(SCROLL_STEP), &mut trace)?;
        research.apply(Operation::Resize(NARROW_WIDTH), &mut trace)?;
        research
            .runtime
            .frames()
            .set_preparation_budget(novadraw::figure::PreparationBudget::new(0, 0));
        assert!(research.apply(Operation::Stable, &mut trace).is_err());
        research.apply(Operation::Reset, &mut trace)?;
        drop(marker);
        trace.set_enabled(false)?;
        trace.set_enabled(false)?;
        let blocked = directory.join(format!(
            "blocked-{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        std::fs::write(&blocked, b"not a directory")?;
        assert!(trace.export(&blocked).is_err());
        assert!(trace.pending.is_some());
        let first = trace.export(&directory)?;
        let bytes = std::fs::read(&first)?;
        let decoded = decoder::Decoded::from_bytes(&bytes);
        decoded.assert_balanced();
        assert!(decoded.contains("\"succeeded\":false"));
        assert!(decoded.contains("\"snapshot\":null"));
        assert!(decoded.contains("host-backend-submit"));
        assert!(decoded.contains("\"acknowledgement\":\"Retry\""));
        assert!(decoded.contains("sdk test failure"));
        assert!(decoded.contains("\"bytes\":51232"));
        assert!(decoded.contains("\"bytes\":51233"));
        assert!(decoded.contains("\"effective_width\":386"));
        trace.set_enabled(true)?;
        let marker = trace.scope("perfetto-contract-marker");
        trace.push(serde_json::json!({"kind":"second-capture"}));
        drop(marker);
        let second = trace.export(&directory)?;
        assert_ne!(first, second);
        assert_eq!(std::fs::read(first)?, bytes);
        assert!(!trace.has_records());
        trace.set_enabled(false)?;
        Ok(())
    }
}
