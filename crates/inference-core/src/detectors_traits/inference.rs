use crate::detectors::{Detectors, Inference};
use anyhow::anyhow;
use ndarray::{Array, Array4, IxDyn};
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

    // fn extract_face_embedding(
    //     session: &Session,
    //     input_tensor: Array4<f32>,
    // ) -> anyhow::Result<Vec<f32>> {
    //     // only for current model (face-recognition.onnx). In the future, it is necessary to use universal method
    //
    //     let input = inputs!["input.1"=>input_tensor]?;
    //
    //     let outputs = session.run(input)?;
    //
    //     let embedding_array = outputs["1197"]
    //         .try_extract_tensor::<f32>()?
    //         .t()
    //         .into_owned();
    //
    //     let embedding: Vec<f32> = embedding_array.into_iter().collect();
    //
    //     let norm = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    //     let normalized_embedding: Vec<f32> = if norm > 0.0 {
    //         embedding.iter().map(|x| x / norm).collect()
    //     } else {
    //         embedding
    //     };
    //
    //     Ok(normalized_embedding)
    // }

    fn extract_face_embedding(
        session: &Session,
        input_tensor: Array4<f32>,
    ) -> anyhow::Result<Vec<f32>> {
        let input = inputs!["input.1"=>input_tensor]?;
        let outputs = session.run(input)?;

        let embedding_array = outputs["1197"]
            .try_extract_tensor::<f32>()?
            .t()
            .into_owned();

        let embedding: Vec<f32> = embedding_array.into_iter().collect();

        if embedding.iter().any(|&x| !x.is_finite()) {
            return Err(anyhow!("Embedding contains invalid values"));
        }

        let norm_sq: f32 = embedding.iter().map(|x| x * x).sum();

        if norm_sq == 0.0 {
            return Err(anyhow!("Zero embedding"));
        }

        let norm = norm_sq.sqrt();

        if norm < 1e-8 {
            return Err(anyhow!("Embedding is too low: {}", norm));
        }

        let normalized_embedding: Vec<f32> = embedding.iter().map(|x| x / norm).collect();

        let check_norm: f32 = normalized_embedding
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        if (check_norm - 1.0).abs() > 1e-5 {
            return Err(anyhow!("Normalization error: normal = {}", check_norm));
        }

        Ok(normalized_embedding)
    }
}
