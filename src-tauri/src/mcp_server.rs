use anyhow::Result;
use async_trait::async_trait;
use rust_mcp_sdk::mcp_server::{
    CallToolError, CallToolRequest, CallToolResult, InitializeResult, ListToolsRequest,
    ListToolsResult, McpServer, ServerCapabilities, ServerHandler, ServerHandlerCore,
};
use rust_mcp_sdk::mcp_types::{Implementation, ServerCapabilitiesTools, LATEST_PROTOCOL_VERSION};
use rust_mcp_sdk::hyper_server::create_hyper_server;
use rust_mcp_sdk::transport::HyperServerOptions;
use std::sync::Arc;
use tokio::sync::Mutex;

use serde::{Deserialize, Serialize};
use rust_mcp_sdk::mcp_tool::{self, JsonSchema};
use crate::rustdoc_processor::{CrateDocs, DocItem};
use std::collections::HashMap;
use crate::embedder::GLOBAL_EMBEDDER;
use std::path::{Path, PathBuf};
use serde_json::json; // For creating simple JSON responses if needed

// --- Tool Definitions ---

#[mcp_tool(name = "process_rust_project", description = "Processes a Rust project to extract and embed its documentation.")]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ProcessRustProjectTool {
    #[schemars(description = "Absolute path to the Rust project directory (containing Cargo.toml).")]
    pub path: String,
}

fn default_num_results() -> Option<usize> { Some(5) }

#[mcp_tool(name = "query_documentation", description = "Queries the processed Rust documentation using a natural language query.")]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryDocumentationTool {
    #[schemars(description = "The natural language query.")]
    pub natural_language_query: String,
    #[schemars(description = "Optional: Absolute path of a specific Rust project to query. If None, queries all processed projects.")]
    pub project_path: Option<String>,
    #[schemars(description = "Number of results to return.", default = "default_num_results")]
    pub num_results: Option<usize>,
}


#[mcp_tool(name = "get_raw_documentation", description = "Retrieves raw documentation for a specific Rust item from a processed project.")]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetRawDocumentationTool {
    #[schemars(description = "The full path to the Rust item (e.g., my_crate::module::MyStruct).")]
    pub item_path: String,
    #[schemars(description = "Absolute path of the Rust project the item belongs to.")]
    pub project_path: String,
}

// --- Helper Structs and Functions ---

#[derive(Clone)]
pub struct ProjectData {
    pub crate_docs: Arc<CrateDocs>,
    pub embeddings: Arc<HashMap<String, Vec<f32>>>,
}

// Struct for query results
#[derive(Debug, Serialize, JsonSchema)]
struct QueryDocResultItem {
    project_path: String,
    item_full_path: String,
    item_type: String,
    description_snippet: Option<String>,
    score: f32,
}

// Cosine similarity function
fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.is_empty() || v2.is_empty() || v1.len() != v2.len() {
        log::warn!("Cosine similarity: invalid vectors. v1_len={}, v2_len={}", v1.len(), v2.len());
        return 0.0;
    }

    let dot_product: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
    let norm_v1: f32 = v1.iter().map(|x| x.powi(2)).sum::<f32>().sqrt();
    let norm_v2: f32 = v2.iter().map(|x| x.powi(2)).sum::<f32>().sqrt();

    if norm_v1 == 0.0 || norm_v2 == 0.0 {
        log::warn!("Cosine similarity: zero norm vector detected.");
        0.0
    } else {
        dot_product / (norm_v1 * norm_v2)
    }
}


// --- AppState Definition ---
pub struct AppState {
    processed_projects: Mutex<HashMap<String, ProjectData>>,
    http_client: reqwest::Client,
    rustdoc_output_dir: PathBuf,
}

impl AppState {
    pub fn new(rustdoc_output_dir: PathBuf) -> Self {
        Self {
            processed_projects: Mutex::new(HashMap::new()),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("Failed to build reqwest client"),
            rustdoc_output_dir,
        }
    }
}

// --- MCP Server Handler ---
pub struct MyMcpServerHandler {
    app_state: Arc<AppState>,
}

impl MyMcpServerHandler {
    pub fn new(app_state: Arc<AppState>) -> Self {
        Self { app_state }
    }
}

#[async_trait]
impl ServerHandler for MyMcpServerHandler {
    async fn handle_list_tools_request(
        &self,
        _request: ListToolsRequest,
        _runtime: &dyn McpServer,
    ) -> Result<ListToolsResult, CallToolError> {
        Ok(ListToolsResult {
            tools: vec![
                ProcessRustProjectTool::tool(),
                QueryDocumentationTool::tool(),
                GetRawDocumentationTool::tool(),
            ],
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        request: CallToolRequest,
        _runtime: &dyn McpServer,
    ) -> Result<CallToolResult, CallToolError> {
        log::info!("Handling CallToolRequest for tool: {}", request.tool_name());
        match request.tool_name() {
            ProcessRustProjectTool::TOOL_NAME => {
                let params: ProcessRustProjectTool = request.arguments()?;
                log::info!("Processing project at path: {}", params.path);
                let project_path_obj = Path::new(&params.path);
                if !project_path_obj.exists() || !project_path_obj.is_dir() {
                    return Err(CallToolError::invalid_arguments(format!(
                        "Project path does not exist or is not a directory: {}", params.path
                    )));
                }
                let rustdoc_output_dir = self.app_state.rustdoc_output_dir.clone();
                match crate::rustdoc_processor::generate_rustdoc_json(project_path_obj, &rustdoc_output_dir) {
                    Ok(json_path) => {
                        log::info!("Successfully generated rustdoc JSON at: {}", json_path.display());
                        match crate::rustdoc_processor::parse_rustdoc_json_file(&json_path) {
                            Ok(crate_docs) => {
                                log::info!("Successfully parsed rustdoc JSON for crate: {}", crate_docs.crate_name);
                                let mut project_embeddings = HashMap::new();
                                let embedder_guard = GLOBAL_EMBEDDER.lock().map_err(|e| CallToolError::internal_error(format!("Failed to lock global embedder: {}", e)))?;
                                if let Some(embedder) = embedder_guard.as_ref() {
                                    let mut texts_to_embed = Vec::new();
                                    let mut item_paths_for_embedding = Vec::new();
                                    for (item_full_path, doc_item) in &crate_docs.items {
                                        if let Some(desc) = &doc_item.description {
                                            if !desc.trim().is_empty() {
                                                texts_to_embed.push(format!("{}::{} [DOCS]: {}", doc_item.crate_name, doc_item.name, desc));
                                                item_paths_for_embedding.push(item_full_path.clone());
                                            }
                                        }
                                    }
                                    if !texts_to_embed.is_empty() {
                                        log::info!("Embedding {} documentation items for {}...", texts_to_embed.len(), crate_docs.crate_name);
                                        match embedder.embed_batch(&texts_to_embed) {
                                            Ok(embeddings_vec) => {
                                                for (path, embedding) in item_paths_for_embedding.into_iter().zip(embeddings_vec.into_iter()) {
                                                    project_embeddings.insert(path, embedding);
                                                }
                                                log::info!("Successfully embedded {} items for {}.", project_embeddings.len(), crate_docs.crate_name);
                                            }
                                            Err(e) => log::error!("Failed to embed batch for {}: {:?}", crate_docs.crate_name, e),
                                        }
                                    } else { log::info!("No descriptions found to embed for {}.", crate_docs.crate_name); }
                                } else { log::warn!("Embedder not initialized. Skipping embedding generation for {}.", crate_docs.crate_name); }
                                let project_data = ProjectData { crate_docs: Arc::new(crate_docs.clone()), embeddings: Arc::new(project_embeddings) };
                                let mut projects_guard = self.app_state.processed_projects.lock().await;
                                projects_guard.insert(params.path.clone(), project_data);
                                let num_embedded = projects_guard.get(&params.path).map_or(0, |pd| pd.embeddings.len());
                                Ok(CallToolResult::text_content(format!("Successfully processed project {} and embedded {} items. Total processed projects: {}.", params.path, num_embedded, projects_guard.len()), None))
                            }
                            Err(e) => Err(CallToolError::internal_error(format!("Failed to parse rustdoc JSON for {}: {}", params.path, e))),
                        }
                    }
                    Err(e) => Err(CallToolError::internal_error(format!("Failed to generate rustdoc JSON for {}: {}", params.path, e))),
                }
            }
            GetRawDocumentationTool::TOOL_NAME => {
                let params: GetRawDocumentationTool = request.arguments()?;
                log::info!("Attempting to get raw documentation for item '{}' in project '{}'", params.item_path, params.project_path);
                let projects_guard = self.app_state.processed_projects.lock().await;
                match projects_guard.get(&params.project_path) {
                    Some(project_data) => {
                        match project_data.crate_docs.items.get(&params.item_path) {
                            Some(doc_item) => {
                                log::info!("Found item '{}'. Returning its details.", params.item_path);
                                CallToolResult::json_content(serde_json::to_value(doc_item)
                                    .map_err(|e| CallToolError::internal_error(format!("Failed to serialize DocItem: {}", e)))?, None)
                            }
                            None => {
                                log::warn!("Item '{}' not found in project '{}'", params.item_path, params.project_path);
                                Err(CallToolError::resource_not_found(format!("Item '{}' not found in project '{}'", params.item_path, params.project_path)))
                            }
                        }
                    }
                    None => {
                        log::warn!("Project '{}' not found.", params.project_path);
                        Err(CallToolError::resource_not_found(format!("Project '{}' has not been processed or was not found.", params.project_path)))
                    }
                }
            }
            QueryDocumentationTool::TOOL_NAME => {
                let params: QueryDocumentationTool = request.arguments()?;
                log::info!("Querying documentation with: '{}'", params.natural_language_query);

                let embedder_guard = GLOBAL_EMBEDDER.lock().map_err(|e| CallToolError::internal_error(format!("Failed to lock global embedder: {}", e)))?;
                let embedder = embedder_guard.as_ref().ok_or_else(|| CallToolError::internal_error("Embedder not initialized. Cannot generate query embedding.".to_string()))?;

                let query_embedding = embedder.embed_sentence(&params.natural_language_query)
                    .map_err(|e| CallToolError::internal_error(format!("Failed to embed query: {}", e)))?;

                let projects_guard = self.app_state.processed_projects.lock().await;
                let mut scored_items = Vec::new();

                for (proj_path_key, project_data) in projects_guard.iter() {
                    if params.project_path.is_some() && params.project_path.as_ref() != Some(proj_path_key) {
                        continue; // Skip if a specific project is requested and this is not it
                    }
                    for (item_full_path, item_embedding) in project_data.embeddings.iter() {
                        if let Some(doc_item) = project_data.crate_docs.items.get(item_full_path) {
                            let score = cosine_similarity(&query_embedding, item_embedding);
                            scored_items.push((doc_item.clone(), score, proj_path_key.clone()));
                        }
                    }
                }

                // Sort by score descending
                scored_items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                let num_results = params.num_results.unwrap_or_else(|| default_num_results().unwrap_or(5));

                let results: Vec<QueryDocResultItem> = scored_items.into_iter().take(num_results).map(|(item, score, proj_path)| {
                    QueryDocResultItem {
                        project_path: proj_path,
                        item_full_path: item.full_path_str.clone(),
                        item_type: item.item_type.clone(),
                        description_snippet: item.description.as_ref().map(|d| d.chars().take(150).collect::<String>() + "..."), // Truncate description
                        score,
                    }
                }).collect();

                log::info!("Found {} results for query '{}'", results.len(), params.natural_language_query);
                CallToolResult::json_content(serde_json::to_value(results)
                    .map_err(|e| CallToolError::internal_error(format!("Failed to serialize query results: {}", e)))?, None)
            }
            _ => Err(CallToolError::unknown_tool(request.tool_name().to_string())),
        }
    }
}

// --- Server Initialization ---
pub async fn start_mcp_server(app_state: Arc<AppState>) -> Result<()> {
    log::info!("Starting MCP Server...");
    let server_details = InitializeResult {
        server_info: Implementation {
            name: "RustDoc LLM MCP Server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        capabilities: ServerCapabilities { tools: Some(ServerCapabilitiesTools { list_changed: None }), ..Default::default() },
        meta: None,
        instructions: Some("This server provides tools for LLMs to interact with Rust documentation.".to_string()),
        protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
    };
    let handler = MyMcpServerHandler::new(app_state);
    let options = HyperServerOptions { host: "127.0.0.1".to_string(), port: 3001, ..Default::default() };
    log::info!("MCP Server will listen on {}:{}", options.host, options.port);
    let server_runtime = create_hyper_server(server_details, handler, options)?;
    server_runtime.start().await?;
    Ok(())
}

// Modified to accept AppState instance
pub fn init_mcp_server(app_state_instance: Arc<AppState>) {
    tokio::spawn(async move { // app_state_instance is moved into the async block
        if let Err(e) = start_mcp_server(app_state_instance).await { // Pass it to start_mcp_server
            log::error!("MCP Server failed: {:?}", e);
        }
    });
    log::info!("MCP Server initialization process started using shared AppState.");
}
