use crate::detection::{Detectors, Inference};
use ndarray::{Array, IxDyn};
use ort::inputs;
use ort::session::Session;

impl Inference for Detectors {
    fn yolo11_inference(
        session: &Session,
        frame: Array<f32, ndarray::Dim<[usize; 4]>>,
    ) -> anyhow::Result<Array<f32, IxDyn>> {
        let input = inputs!["images"=>frame]?;

        let outputs = session.run(input)?;

        Ok(outputs["output0"]
            .try_extract_tensor::<f32>()?
            .t()
            .into_owned())
    }
}
