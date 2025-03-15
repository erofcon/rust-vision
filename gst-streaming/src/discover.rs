use anyhow::Result;
use gst_pbutils::prelude::DiscovererStreamInfoExt;
use gst_pbutils::{Discoverer, DiscovererInfo, DiscovererStreamInfo};

#[derive(Debug)]
pub struct FileInfo {
    pub width: i32,
    pub height: i32,
}

fn discover_resolution_from_stream_info(stream_info: &DiscovererStreamInfo) -> Option<FileInfo> {
    let mut width = None;
    let mut height = None;

    if let Some(caps) = stream_info.caps() {
        let caps_ref = caps.as_ref();
        for struct_ref in caps_ref.iter() {
            for (name, value) in struct_ref.iter() {
                if name == "width" {
                    width = Some(value.get().unwrap());
                }
                if name == "height" {
                    height = Some(value.get().unwrap());
                }
                if let (Some(width), Some(height)) = (width, height) {
                    return Some(FileInfo { width, height });
                }
            }
        }
    }

    None
}

fn discover_resolution(info: &DiscovererInfo) -> anyhow::Result<FileInfo> {
    if let Some(stream_info) = info.stream_info() {
        if let Some(file_info) = discover_resolution_from_stream_info(&stream_info) {
            return Ok(file_info);
        }
    }

    for child_stream in info.stream_list() {
        if let Some(file_info) = discover_resolution_from_stream_info(&child_stream) {
            return Ok(file_info);
        }
    }
    Err(anyhow::anyhow!(
        "No stream with a width/height feature pair discovered"
    ))
}

pub fn discover(path: &str) -> Result<FileInfo> {
    let timeout = gst::ClockTime::from_seconds(10);
    let discoverer = Discoverer::new(timeout);

    let file_url = format!("file:///{}", path);

    let info = discoverer?.discover_uri(&file_url)?;

    discover_resolution(&info)
}
