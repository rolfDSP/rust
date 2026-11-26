mod hub;

use anyhow::{Error as E, Result};
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hub::Api;
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer};

const MODEL_ID: &str = "sentence-transformers/all-MiniLM-L6-v2";
const REVISION: &str = "refs/pr/21";

fn build_model_and_tokenizer(device: &Device) -> Result<(BertModel, Tokenizer)> {
    let api = Api::new()?;
    let repo = api.model(MODEL_ID.to_string()).with_revision(REVISION);
    let config_filename = repo.get("config.json")?;
    let tokenizer_filename = repo.get("tokenizer.json")?;
    let weights_filename = repo.get("model.safetensors")?;

    let config = std::fs::read_to_string(config_filename)?;
    let config: Config = serde_json::from_str(&config)?;
    let tokenizer = Tokenizer::from_file(tokenizer_filename).map_err(E::msg)?;

    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights_filename], DTYPE, device)? };
    let model = BertModel::load(vb, &config)?;
    Ok((model, tokenizer))
}

fn normalize_l2(v: &Tensor) -> Result<Tensor> {
    Ok(v.broadcast_div(&v.sqr()?.sum_keepdim(1)?.sqrt()?)?)
}

fn main() -> Result<()> {
    let device = match Device::new_cuda(0) {
        Ok(device) => {
            println!("Running on GPU: {device:?}");
            device
        }
        Err(err) => {
            println!("CUDA unavailable ({err}), falling back to CPU");
            Device::Cpu
        }
    };

    let (model, mut tokenizer) = build_model_and_tokenizer(&device)?;

    let sentences = [
        "The cat sits outside",
        "A man is playing guitar",
        "I love pasta",
        "The new movie is awesome",
        "The cat plays in the garden",
        "A woman watches TV",
        "The new movie is so great",
        "Do you like pizza?",
    ];
    let n_sentences = sentences.len();

    let pp = PaddingParams {
        strategy: PaddingStrategy::BatchLongest,
        ..Default::default()
    };
    tokenizer.with_padding(Some(pp));

    let tokens = tokenizer
        .encode_batch(sentences.to_vec(), true)
        .map_err(E::msg)?;
    let token_ids = tokens
        .iter()
        .map(|t| Ok(Tensor::new(t.get_ids().to_vec().as_slice(), &device)?))
        .collect::<Result<Vec<_>>>()?;
    let attention_mask = tokens
        .iter()
        .map(|t| Ok(Tensor::new(t.get_attention_mask().to_vec().as_slice(), &device)?))
        .collect::<Result<Vec<_>>>()?;

    let token_ids = Tensor::stack(&token_ids, 0)?;
    let attention_mask = Tensor::stack(&attention_mask, 0)?;
    let token_type_ids = token_ids.zeros_like()?;

    println!("running inference on batch {:?}", token_ids.shape());
    let embeddings = model.forward(&token_ids, &token_type_ids, Some(&attention_mask))?;
    println!("generated embeddings {:?}", embeddings.shape());

    // Mean-pool over tokens, masking out padding, then L2-normalize.
    let attention_mask_for_pooling = attention_mask.to_dtype(DTYPE)?.unsqueeze(2)?;
    let sum_mask = attention_mask_for_pooling.sum(1)?;
    let embeddings = (embeddings.broadcast_mul(&attention_mask_for_pooling)?).sum(1)?;
    let embeddings = embeddings.broadcast_div(&sum_mask)?;
    let embeddings = normalize_l2(&embeddings)?;
    println!("pooled embeddings {:?}", embeddings.shape());

    let mut similarities = vec![];
    for i in 0..n_sentences {
        let e_i = embeddings.get(i)?;
        for j in (i + 1)..n_sentences {
            let e_j = embeddings.get(j)?;
            let sum_ij = (&e_i * &e_j)?.sum_all()?.to_scalar::<f32>()?;
            let sum_i2 = (&e_i * &e_i)?.sum_all()?.to_scalar::<f32>()?;
            let sum_j2 = (&e_j * &e_j)?.sum_all()?.to_scalar::<f32>()?;
            let cosine_similarity = sum_ij / (sum_i2 * sum_j2).sqrt();
            similarities.push((cosine_similarity, i, j));
        }
    }
    similarities.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("\nTop 5 most similar sentence pairs:");
    for &(score, i, j) in similarities[..5].iter() {
        println!("score: {score:.2} '{}' '{}'", sentences[i], sentences[j]);
    }

    Ok(())
}