/// Task being processed.
/// All Task that are not included in this enumeration will be discarded
pub enum TaskType {
    RunPipeline,
    GenerateReport,
}

/// Configuration for worker
pub struct WorkerConfig {
    pub url: String,
    pub queue_name: String,
    pub prefetch_count: u16,
    pub consumer_tag: String,
}
