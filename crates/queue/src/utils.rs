/// Queues being processed.
/// All queues that are not included in this enumeration will be discarded

/// Configuration for worker
pub struct WorkerConfig {
    pub url: String,
    pub queue_name: String,
    pub prefetch_count: u16,
    pub consumer_tag: String,
}
