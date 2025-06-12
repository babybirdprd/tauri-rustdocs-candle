use anyhow::{Context, Result, Error as AnyhowError};
use candle_core::{Device, Tensor, D};
use candle_nn::VarBuilder;
// Assuming Qwen2Model and Qwen2Config are available.
// If these lines cause a compilation error, candle-transformers doesn't support Qwen2Model as expected.
use candle_transformers::models::qwen2::{Model as Qwen2Model, Config as Qwen2Config, DTYPE};
use hf_hub::{api::sync::Api, Repo, RepoType};
use tokenizers::Tokenizer;
use std::collections::{HashMap, HashSet}; // HashSet for collecting unique filenames
use std::path::PathBuf; // Keep for potential future use
use std::sync::Mutex;

// Configuration for the embedding model
const EMBEDDING_MODEL_REPO: &str = "BAAI/bge-code-v1"; // Updated to bge-code-v1
const EMBEDDING_MODEL_REVISION: &str = "main";

pub struct Embedder {
    model: Qwen2Model, // Updated model type
    tokenizer: Tokenizer,
    device: Device,
}

impl Embedder {
    pub fn new() -> Result<Self> {
        log::info!("Initializing Embedder with model: {}", EMBEDDING_MODEL_REPO);
        let device = match Device::cuda_if_available(0) {
            Ok(cuda_device) => cuda_device,
            Err(_) => {
                log::warn!("CUDA device not found or CUDA not compiled. Falling back to CPU.");
                Device::Cpu
            }
        };
        log::info!("Embedder will use device: {:?}", device);

        let api = Api::new().context("Failed to create HuggingFace API client")?;
        let repo = api.repo(Repo::with_revision(
            EMBEDDING_MODEL_REPO.to_string(),
            RepoType::Model,
            EMBEDDING_MODEL_REVISION.to_string(),
        ));

        log::info!("Fetching model files from HuggingFace Hub: {}", EMBEDDING_MODEL_REPO);
        let config_filename = repo.get("config.json")
            .context(format!("Failed to get config.json from {}", EMBEDDING_MODEL_REPO))?;
        let tokenizer_filename = repo.get("tokenizer.json")
            .context(format!("Failed to get tokenizer.json from {}", EMBEDDING_MODEL_REPO))?;

        // Handle sharded weights using model.safetensors.index.json
        let model_files = match repo.get("model.safetensors.index.json") {
            Ok(index_json_path) => {
                log::info!("Found model.safetensors.index.json. Processing sharded weights for {}.", EMBEDDING_MODEL_REPO);
                let index_json_content = std::fs::read_to_string(index_json_path)
                    .context("Failed to read model.safetensors.index.json")?;
                let index: serde_json::Value = serde_json::from_str(&index_json_content)
                    .context("Failed to parse model.safetensors.index.json")?;
                let weight_map = index.get("weight_map")
                    .context("Missing 'weight_map' in model.safetensors.index.json")?
                    .as_object()
                    .context("'weight_map' is not an object")?;

                let mut filenames = HashSet::new();
                for filename_val in weight_map.values() {
                    if let Some(filename_str) = filename_val.as_str() {
                        filenames.insert(filename_str.to_string());
                    } else {
                        // This case should ideally not happen in a valid index.json
                        log::warn!("Non-string or null value found in weight_map: {:?}. Skipping.", filename_val);
                    }
                }

                if filenames.is_empty() {
                    return Err(anyhow::anyhow!("No filenames found in weight_map of model.safetensors.index.json for {}", EMBEDDING_MODEL_REPO));
                }
                log::info!("Identified sharded weight files for {}: {:?}", EMBEDDING_MODEL_REPO, filenames);

                filenames.into_iter().map(|f| {
                    log::debug!("Fetching sharded weight file: {}", f);
                    repo.get(&f) // hf-hub will cache these
                }).collect::<Result<Vec<_>, _>>()
                    .map_err(|e| anyhow::anyhow!("Failed to download or access a sharded weight file for {}: {}", EMBEDDING_MODEL_REPO, e))?
            }
            Err(e) => {
                // This model ('BAAI/bge-code-v1') IS sharded. If index.json is missing, it's an issue with the repo or network.
                log::error!("model.safetensors.index.json not found for {} (Error: {}). This model is expected to be sharded. Cannot proceed without it.", EMBEDDING_MODEL_REPO, e);
                return Err(anyhow::anyhow!("model.safetensors.index.json is required for sharded model {} but was not found. Error: {}", EMBEDDING_MODEL_REPO, e));
            }
        };

        log::info!("Model config file for {}: {:?}", EMBEDDING_MODEL_REPO, config_filename);
        log::info!("Tokenizer file for {}: {:?}", EMBEDDING_MODEL_REPO, tokenizer_filename);
        log::info!("Model weight files to load for {}: {:?}", EMBEDDING_MODEL_REPO, model_files);

        let config_str = std::fs::read_to_string(config_filename)?;
        let config: Qwen2Config = serde_json::from_str(&config_str)
            .context(format!("Failed to parse Qwen2Config from config.json for {}", EMBEDDING_MODEL_REPO))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_filename)
            .map_err(|e| AnyhowError::msg(format!("Failed to load tokenizer for {}: {}", EMBEDDING_MODEL_REPO, e)))?;

        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&model_files, DTYPE, &device)?
        };

        let model = Qwen2Model::load(vb, &config)?;

        log::info!("Embedding model {} loaded successfully.", EMBEDDING_MODEL_REPO);

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    pub fn embed_batch(&self, sentences: &[String]) -> Result<Vec<Vec<f32>>> {
        if sentences.is_empty() {
            return Ok(Vec::new());
        }

        let encodings = self.tokenizer.encode_batch(
            sentences.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            true // add_special_tokens
        ).map_err(|e| AnyhowError::msg(format!("Failed to tokenize batch: {}", e)))?;

        let mut all_embeddings = Vec::new();

        for encoding in encodings {
            let token_ids_vec: Vec<u32> = encoding.get_ids().to_vec();
            let token_ids = Tensor::new(token_ids_vec.as_slice(), &self.device)?.unsqueeze(0)?;

            // Qwen2Model::forward takes (tokens, start_pos)
            // start_pos = 0 for non-kv-cached scenario (i.e., full sequence processing)
            let model_output = self.model.forward(&token_ids, 0)?;
            // log::trace!("Raw Qwen2 model output (last_hidden_state) shape: {:?}", model_output.shape());

            // Pooling strategy: For BGE, typically CLS token embedding is used.
            // This assumes the first token [CLS] is used for pooling.
            // Output shape from Qwen2Model forward is (batch_size, seq_len, hidden_size)
            let sentence_embedding = model_output.i((0, 0, ..))?;
            // log::trace!("CLS token embedding shape: {:?}", sentence_embedding.shape());

            // Normalization (L2 norm) - crucial for BGE models
            let norm = sentence_embedding.sqr()?.sum_keepdim(D::Last)?.sqrt()?;
            let sentence_embedding_normalized = sentence_embedding.broadcast_div(&norm)?;
            // log::trace!("Normalized CLS token embedding shape: {:?}", sentence_embedding_normalized.shape());

            all_embeddings.push(sentence_embedding_normalized.to_vec1::<f32>()?);
        }

        log::debug!("Generated {} embeddings with model {}.", all_embeddings.len(), EMBEDDING_MODEL_REPO);
        Ok(all_embeddings)
    }

    pub fn embed_sentence(&self, sentence: &str) -> Result<Vec<f32>> {
        let embeddings_batch = self.embed_batch(&[sentence.to_string()])?;
        embeddings_batch.into_iter().next()
            .context(format!("Embedding batch returned no results for a single sentence using model {}", EMBEDDING_MODEL_REPO))
    }
}

use once_cell::sync::Lazy;
pub static GLOBAL_EMBEDDER: Lazy<Mutex<Option<Embedder>>> = Lazy::new(|| Mutex::new(None));

pub fn init_global_embedder() -> Result<()> {
    log::info!("Attempting to initialize global embedder with model {}...", EMBEDDING_MODEL_REPO);
    let mut guard = GLOBAL_EMBEDDER.lock().map_err(|e| AnyhowError::msg(format!("Failed to acquire lock on GLOBAL_EMBEDDER: {}",e)))?;
    if guard.is_none() {
        match Embedder::new() {
            Ok(embedder) => {
                *guard = Some(embedder);
                log::info!("Global embedder initialized successfully with model {}.", EMBEDDING_MODEL_REPO);
            }
            Err(e) => {
                log::error!("Failed to initialize global embedder with model {}: {:?}", EMBEDDING_MODEL_REPO, e);
                return Err(e.context(format!("Embedder::new() failed for model {} during global initialization", EMBEDDING_MODEL_REPO)));
            }
        }
    } else {
        // Check if the existing embedder is for the correct model, though this function is usually called once.
        log::info!("Global embedder (model {}) already initialized or initialization was attempted.", EMBEDDING_MODEL_REPO);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    // Test is enabled
    async fn test_embedder_init_and_embed() -> Result<()> {
        // It's good practice to ensure test output is visible, env_logger or similar.
        // RUST_LOG=info cargo test -- --nocapture embedder::tests::test_embedder_init_and_embed
        let _ = env_logger::builder().is_test(true).filter_level(log::LevelFilter::Debug).try_init();

        init_global_embedder().context("Test failed to initialize global embedder")?;

        let embedder_guard = GLOBAL_EMBEDDER.lock().unwrap();
        let embedder = embedder_guard.as_ref().context("Embedder not initialized after init_global_embedder call")?;

        let sentence = "This is a test sentence for the BGE code embedder.";
        let embedding = embedder.embed_sentence(sentence).context("Failed to embed single sentence")?;

        assert!(!embedding.is_empty(), "Embedding should not be empty");
        // BAAI/bge-code-v1 has a hidden size of 1536
        assert_eq!(embedding.len(), 1536, "Embedding dimension mismatch for {}. Expected 1536, got {}", EMBEDDING_MODEL_REPO, embedding.len());
        log::info!("Single sentence embedding (first 5 dims for {}): {:?}", EMBEDDING_MODEL_REPO, &embedding[..5.min(embedding.len())]);

        let sentences = vec![
            "fn main() { println!(\"Hello, world!\"); }".to_string(),
            "struct MyStruct { field: i32 }".to_string(),
        ];
        let batch_embeddings = embedder.embed_batch(&sentences).context("Failed to embed batch of sentences")?;
        assert_eq!(batch_embeddings.len(), sentences.len(), "Number of embeddings should match number of input sentences");

        for (i, emb) in batch_embeddings.iter().enumerate() {
            assert_eq!(emb.len(), 1536, "Embedding dimension mismatch for sentence {} in batch (model {}). Expected 1536, got {}", i, EMBEDDING_MODEL_REPO, emb.len());
        }
        log::info!("Batch embeddings generated for {} sentences using {}.", batch_embeddings.len(), EMBEDDING_MODEL_REPO);

        let empty_batch_embeddings = embedder.embed_batch(&[]).context("Failed to process empty batch")?;
        assert!(empty_batch_embeddings.is_empty(), "Embedding an empty batch should result in an empty list of embeddings");

        Ok(())
    }
}
