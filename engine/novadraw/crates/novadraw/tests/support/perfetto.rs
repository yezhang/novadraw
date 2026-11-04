//! Real SDK recordings decoded with the SDK's official protobuf decoder.
use perfetto_sdk::{
    heap_buffer::HeapBuffer,
    pb_decoder::{PbDecoder, PbDecoderField},
    pb_msg::{PbMsg, PbMsgWriter},
    producer::{Backends, Producer, ProducerInitArgsBuilder},
    protos::{
        config::{
            data_source_config::DataSourceConfig,
            trace_config::{TraceConfig, TraceConfigBufferConfig, TraceConfigDataSource},
            track_event::track_event_config::TrackEventConfig,
        },
        trace::{
            interned_data::interned_data::InternedDataFieldNumber,
            trace::TraceFieldNumber,
            trace_packet::TracePacketFieldNumber,
            track_event::{
                debug_annotation::DebugAnnotationFieldNumber,
                track_event::{EventNameFieldNumber, TrackEventFieldNumber, TrackEventType},
            },
        },
    },
    tracing_session::TracingSession,
    track_event::TrackEvent,
};
use std::sync::{Arc, Mutex};

pub struct Capture(
    TracingSession,
    Option<novadraw::text::TextLayoutTimingScope>,
);

impl Capture {
    pub fn new() -> Self {
        Producer::init(
            ProducerInitArgsBuilder::new()
                .backends(Backends::IN_PROCESS)
                .build(),
        );
        TrackEvent::init();
        novadraw::text::register_perfetto_categories().unwrap();
        let writer = PbMsgWriter::new();
        let buffer = HeapBuffer::new(writer.stream_writer());
        let mut msg = PbMsg::new(&writer).unwrap();
        let mut config = TraceConfig { msg: &mut msg };
        config.set_buffers(|b: &mut TraceConfigBufferConfig| {
            b.set_size_kb(32 * 1024);
        });
        config.set_data_sources(|d: &mut TraceConfigDataSource| {
            d.set_config(|d: &mut DataSourceConfig| {
                d.set_name("track_event");
                d.set_track_event_config(|c: &mut TrackEventConfig| {
                    c.set_disabled_categories("*");
                    c.set_enabled_categories("novadraw.phase");
                    c.set_enabled_categories("novadraw.request");
                });
            });
        });
        msg.finalize();
        let mut config = vec![0; writer.stream_writer().get_written_size()];
        buffer.copy_into(&mut config);
        let mut session = TracingSession::in_process().unwrap();
        session.setup(&config);
        session.start_blocking();
        let marker =
            novadraw::text::TextLayoutDiagnostics::default().time("perfetto-contract-marker");
        Self(session, Some(marker))
    }

    pub fn finish(mut self) -> Decoded {
        drop(self.1.take());
        self.0.stop_blocking();
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = bytes.clone();
        self.0
            .read_trace_blocking(move |data, _| output.lock().unwrap().extend_from_slice(data));
        Decoded::from_bytes(&bytes.lock().unwrap())
    }
}

pub struct Decoded {
    strings: Vec<String>,
    begins: usize,
    ends: usize,
}

impl Decoded {
    pub fn contains(&self, value: &str) -> bool {
        self.strings.iter().any(|s| s.contains(value))
    }

    pub fn assert_balanced(&self) {
        assert!(self.begins > 0);
        assert_eq!(self.begins, self.ends);
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut sequences = std::collections::HashMap::<u64, Self>::new();
        for (id, field) in PbDecoder::new(bytes).map(Result::unwrap) {
            if id != TraceFieldNumber::Packet as u32 {
                continue;
            }
            let PbDecoderField::Delimited(packet) = field else {
                panic!("invalid packet");
            };
            let sequence = PbDecoder::new(packet)
                .map(Result::unwrap)
                .find_map(|(id, field)| {
                    if id == TracePacketFieldNumber::TrustedPacketSequenceId as u32
                        && let PbDecoderField::Varint(value) = field
                    {
                        Some(value)
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            let decoded = sequences.entry(sequence).or_insert_with(|| Self {
                strings: Vec::new(),
                begins: 0,
                ends: 0,
            });
            for (id, field) in PbDecoder::new(packet).map(Result::unwrap) {
                let PbDecoderField::Delimited(data) = field else {
                    continue;
                };
                if id == TracePacketFieldNumber::InternedData as u32 {
                    for (id, field) in PbDecoder::new(data).map(Result::unwrap) {
                        if id == InternedDataFieldNumber::EventNames as u32
                            && let PbDecoderField::Delimited(name) = field
                        {
                            decoded
                                .strings
                                .extend(strings(name, EventNameFieldNumber::Name as u32));
                        }
                    }
                } else if id == TracePacketFieldNumber::TrackEvent as u32 {
                    for (id, field) in PbDecoder::new(data).map(Result::unwrap) {
                        if id == TrackEventFieldNumber::Type as u32
                            && let PbDecoderField::Varint(kind) = field
                        {
                            if kind == TrackEventType::TypeSliceBegin as u64 {
                                decoded.begins += 1;
                            }
                            if kind == TrackEventType::TypeSliceEnd as u64 {
                                decoded.ends += 1;
                            }
                        } else if id == TrackEventFieldNumber::DebugAnnotations as u32
                            && let PbDecoderField::Delimited(args) = field
                        {
                            decoded.strings.extend(strings(
                                args,
                                DebugAnnotationFieldNumber::StringValue as u32,
                            ));
                        } else if id == TrackEventFieldNumber::Name as u32
                            && let PbDecoderField::Delimited(name) = field
                        {
                            decoded
                                .strings
                                .push(String::from_utf8(name.to_vec()).unwrap());
                        }
                    }
                }
            }
        }
        sequences
            .into_values()
            .find(|trace| trace.contains("perfetto-contract-marker"))
            .expect("missing recording-thread marker")
    }
}

fn strings(bytes: &[u8], wanted: u32) -> Vec<String> {
    PbDecoder::new(bytes)
        .map(Result::unwrap)
        .filter_map(|(id, field)| {
            if id == wanted
                && let PbDecoderField::Delimited(value) = field
            {
                Some(String::from_utf8(value.to_vec()).unwrap())
            } else {
                None
            }
        })
        .collect()
}
