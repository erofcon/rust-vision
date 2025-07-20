use crate::detectors::{Detectors, Inference};
use anyhow::anyhow;
use ndarray::{Array, Array4, ArrayBase, Dim, Ix, IxDyn, OwnedRepr};
use ort::inputs;
use ort::session::{Session, SessionOutputs};

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

    fn extract_face_embedding(
        session: &Session,
        input_tensor: Array4<f32>,
    ) -> anyhow::Result<Vec<f32>> {
        let input = inputs!["input.1" => input_tensor]?;
        let outputs = session.run(input)?;

        let embedding_array = outputs["1197"]
            .try_extract_tensor::<f32>()?
            .view()
            .to_owned();

        let embedding: Vec<f32> = embedding_array.into_iter().collect();

        if embedding.iter().any(|&x| !x.is_finite()) {
            return Err(anyhow!("Embedding contains invalid values"));
        }

        if embedding.len() != 512 {
            return Err(anyhow!(
                "Expected 512-dim embedding, got {}",
                embedding.len()
            ));
        }

        Ok(embedding)
    }

    fn inference(
        session: &Session,
        model_input: ArrayBase<OwnedRepr<f32>, Dim<[Ix; 4]>>,
    ) -> anyhow::Result<Array<f32, IxDyn>> {
        let outputs: SessionOutputs = session.run(inputs!["images" => model_input]?)?;
        let output = outputs["output0"].try_extract_tensor::<f32>()?.into_owned();
        Ok(output)
    }
}
