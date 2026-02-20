pub mod aggregator;
pub mod sampler;
pub mod structured;
pub mod tracer;

pub use aggregator::LogAggregator;
pub use sampler::{LogSampler, SamplingStrategy};
pub use structured::{LogEntry, LogEntryBuilder, LogLevel};
pub use tracer::{
    create_span, create_span_with_attributes, current_trace_ids, extract_trace_ids,
    init_tracer, shutdown_tracer, TraceSpan, TracerConfig, TracerError,
};
