use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tauri::State;
// Ensure tokio::sync::Mutex is used if AppState's Mutex is from tokio, which it should be for async .lock().await
// use tokio::sync::Mutex; // Already in mcp_server.rs, AppState uses tokio::sync::Mutex

use crate::mcp_server::{AppState, ProjectData}; // Make these types accessible
use crate::rustdoc_processor;
use crate::embedder::GLOBAL_EMBEDDER;

// Define the return type for query results to match the UI
#[derive(Debug, serde::Serialize, Clone)] // Added Clone for convenience if needed later
pub struct QueryDocResultItem {
    pub project_path: String,
    pub item_full_path: String,
    pub item_type: String,
    pub description_snippet: Option<String>,
    pub score: f32,
}


#[tauri::command]
pub async fn invoke_process_rust_project(
    path: String,
    app_state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    log::info!("[Tauri Command] invoke_process_rust_project called for path: {}", path);
    let project_path_obj = Path::new(&path);

    if !project_path_obj.exists() || !project_path_obj.is_dir() {
        let err_msg = format!("Project path does not exist or is not a directory: {}", path);
        log::error!("{}", err_msg);
        return Err(err_msg);
    }

    // Logic adapted from ProcessRustProjectTool in mcp_server.rs
    match rustdoc_processor::generate_rustdoc_json(project_path_obj, &app_state.rustdoc_output_dir) {
        Ok(json_path) => {
            log::info!("Generated rustdoc at: {}", json_path.display());
            match rustdoc_processor::parse_rustdoc_json_file(&json_path) {
                Ok(crate_docs) => {
                    log::info!("Parsed rustdoc for crate: {}", crate_docs.crate_name);
                    let mut project_embeddings = HashMap::new();
                    let embedder_guard = GLOBAL_EMBEDDER.lock().map_err(|e| format!("Failed to lock global embedder: {}", e))?;

                    if let Some(embedder) = embedder_guard.as_ref() {
                        let mut texts_to_embed = Vec::new();
                        let mut item_paths_for_embedding = Vec::new();

                        for (item_full_path, doc_item) in &crate_docs.items {
                            if let Some(desc) = &doc_item.description {
                                if !desc.trim().is_empty() {
                                    // Using a slightly more descriptive format for embedding
                                    texts_to_embed.push(format!("Crate: {}, Item: {}, Type: {}, Docs: {}", doc_item.crate_name, doc_item.name, doc_item.item_type, desc));
                                    item_paths_for_embedding.push(item_full_path.clone());
                                }
                            }
                        }

                        if !texts_to_embed.is_empty() {
                            log::info!("Embedding {} docs for {}", texts_to_embed.len(), crate_docs.crate_name);
                            match embedder.embed_batch(&texts_to_embed) {
                                Ok(embeddings_vec) => {
                                    for (item_path_key, embedding) in item_paths_for_embedding.into_iter().zip(embeddings_vec.into_iter()) {
                                        project_embeddings.insert(item_path_key, embedding);
                                    }
                                    log::info!("Successfully embedded {} items for {}.", project_embeddings.len(), crate_docs.crate_name);
                                }
                                Err(e) => {
                                    log::error!("Failed to embed batch for {}: {:?}", crate_docs.crate_name, e);
                                    // Non-fatal for now, proceed without embeddings for these items
                                }
                            }
                        } else {
                            log::info!("No suitable descriptions found for embedding in {}.", crate_docs.crate_name);
                        }
                    } else {
                        log::warn!("Embedder not initialized. Skipping embedding generation.");
                    }

                    let project_data = ProjectData {
                        crate_docs: Arc::new(crate_docs.clone()),
                        embeddings: Arc::new(project_embeddings),
                    };

                    let mut projects_map_guard = app_state.processed_projects.lock().await;
                    projects_map_guard.insert(path.clone(), project_data);

                    let num_embedded = projects_map_guard.get(&path).map_or(0, |pd| pd.embeddings.len());
                    let success_msg = format!("Successfully processed project {} and embedded {} items. Total processed projects: {}.", path, num_embedded, projects_map_guard.len());
                    log::info!("{}", success_msg);
                    Ok(success_msg)
                }
                Err(e) => {
                    let err_msg = format!("Failed to parse rustdoc JSON for {}: {:?}", path, e);
                    log::error!("{}", err_msg);
                    Err(err_msg)
                }
            }
        }
        Err(e) => {
            let err_msg = format!("Failed to generate rustdoc JSON for {}: {:?}", path, e);
            log::error!("{}", err_msg);
            Err(err_msg)
        }
    }
}

// Helper for cosine similarity
fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.is_empty() || v2.is_empty() || v1.len() != v2.len() { return 0.0; }
    let dot_product: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
    let norm_v1: f32 = v1.iter().map(|x| x.powi(2)).sum::<f32>().sqrt();
    let norm_v2: f32 = v2.iter().map(|x| x.powi(2)).sum::<f32>().sqrt();
    if norm_v1 == 0.0 || norm_v2 == 0.0 { 0.0 } else { dot_product / (norm_v1 * norm_v2) }
}

#[tauri::command]
pub async fn invoke_query_documentation(
    query: String, // Parameter name from JS: naturalLanguageQuery, but Rust style is snake_case.
                  // Tauri will match if `invoke` uses naturalLanguageQuery.
                  // For clarity, could rename here or ensure JS matches.
                  // Let's assume JS sends `natural_language_query` and Tauri maps it.
                  // If not, ensure client sends `query` or rename this to `natural_language_query`.
                  // For now, using `query` as per the command definition.
    project_path: Option<String>,
    num_results: Option<usize>, // Added num_results parameter
    app_state: State<'_, Arc<AppState>>,
) -> Result<Vec<QueryDocResultItem>, String> {
    log::info!("[Tauri Command] invoke_query_documentation: '{}', project_filter: {:?}, num_results: {:?}", query, project_path, num_results);
    let num_results_cap = num_results.unwrap_or(5); // Use provided num_results or default

    let query_embedding = {
        let embedder_guard = GLOBAL_EMBEDDER.lock().map_err(|e| format!("Failed to lock global embedder: {}",e))?;
        if let Some(embedder) = embedder_guard.as_ref() {
            embedder.embed_sentence(&query).map_err(|e| format!("Failed to embed query: {}", e))?
        } else {
            return Err("Embedder not initialized".to_string());
        }
    };

    let projects_map_guard = app_state.processed_projects.lock().await;
    let mut all_scored_items = Vec::new();

    for (current_proj_path, proj_data) in projects_map_guard.iter() {
        if project_path.as_ref().map_or(true, |p| p == current_proj_path) {
            for (item_full_path, item_embedding) in proj_data.embeddings.iter() {
                let score = cosine_similarity(&query_embedding, item_embedding);
                // Consider a threshold for score if results are too noisy
                // Example: if score > 0.5 { ... }
                if let Some(doc_item) = proj_data.crate_docs.items.get(item_full_path) {
                    all_scored_items.push(QueryDocResultItem {
                        project_path: current_proj_path.clone(),
                        item_full_path: item_full_path.clone(),
                        item_type: doc_item.item_type.clone(),
                        description_snippet: doc_item.description.as_ref().map(|d| d.chars().take(300).collect()),
                        score,
                    });
                }
            }
        }
    }

    all_scored_items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    all_scored_items.truncate(num_results_cap);

    log::info!("Found {} results for query '{}'.", all_scored_items.len(), query);
    Ok(all_scored_items)
}

#[tauri::command]
pub async fn get_processed_project_list(
    app_state: State<'_, Arc<AppState>>,
) -> Result<Vec<String>, String> {
    log::info!("[Tauri Command] get_processed_project_list");
    let guard = app_state.processed_projects.lock().await;
    Ok(guard.keys().cloned().collect())
}
