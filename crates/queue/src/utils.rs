/// QueueType being processed.
/// All QueueType that are not included in this enumeration will be discarded
#[derive(Debug, Clone, Copy)]
pub enum QueueType {
    RunPipeline,
    GenerateReport,
}

impl QueueType {
    pub fn to_str(&self) -> &'static str {
        match self {
            QueueType::RunPipeline => "run_pipeline",
            QueueType::GenerateReport => "generate_report",
        }
    }
}
