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

#[derive(Debug, Clone, Copy)]
pub enum TaskStatus {
    NotStarted,
    Waiting,
    Processing,
    Completed,
    Error,
}

impl TaskStatus {
    pub fn to_str(&self) -> &'static str {
        match self {
            TaskStatus::NotStarted => "not_started",
            TaskStatus::Waiting => "waiting",
            TaskStatus::Processing => "processing",
            TaskStatus::Completed => "completed",
            TaskStatus::Error => "error",
        }
    }

    pub fn from_str(status: &str) -> Option<TaskStatus> {
        match status {
            "not_started" => Some(TaskStatus::NotStarted),
            "waiting" => Some(TaskStatus::Waiting),
            "processing" => Some(TaskStatus::Processing),
            "completed" => Some(TaskStatus::Completed),
            "error" => Some(TaskStatus::Error),
            _ => None,
        }
    }
}
