use anyhow::Result;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    Uploaded,
    Waiting,
    Processing,
    Completed,
    Failed,
}

impl TaskStatus {
    pub fn to_str(&self) -> &'static str {
        match self {
            TaskStatus::Uploaded => "uploaded",
            TaskStatus::Waiting => "waiting",
            TaskStatus::Processing => "processing",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
        }
    }

    pub fn from_str(status: &str) -> Option<TaskStatus> {
        match status {
            "uploaded" => Some(TaskStatus::Uploaded),
            "waiting" => Some(TaskStatus::Waiting),
            "processing" => Some(TaskStatus::Processing),
            "completed" => Some(TaskStatus::Completed),
            "failed" => Some(TaskStatus::Failed),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Payload {
    pub id: Uuid,
}

impl Payload {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        Ok(bincode::serialize(&self)?)
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        Ok(bincode::deserialize(data)?)
    }
}
