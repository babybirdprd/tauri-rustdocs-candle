// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

// Add module declaration
pub mod mcp_server;
pub mod rustdoc_processor;
pub mod embedder;
pub mod commands; // Declare commands module

// Keep existing if used, add others as needed
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex; // Ensure AppState uses this
use std::time::{SystemTime, UNIX_EPOCH};


#[tauri::command]
fn greet() -> String {
  let now = SystemTime::now();
  let epoch_ms = now.duration_since(UNIX_EPOCH).unwrap().as_millis();
  format!("Hello world from Rust! Current epoch: {}", epoch_ms)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  // Initialize env_logger for backend logging
  // It's good to do this early.
  // Consider making the log level configurable (e.g., via an environment variable).
  env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

  // Initialize the global embedder
  // This can take time, so consider if it should block startup or be async.
  // For now, let's do it synchronously and log errors.
  if let Err(e) = embedder::init_global_embedder() {
    log::error!("Failed to initialize global embedder during startup: {:?}. Some features might not work.", e);
    // Depending on how critical the embedder is, you might want to panic or show an error to the user.
  }

  // Initialize and spawn the MCP server
  // Create AppState instance first
  let base_dirs = directories::BaseDirs::new().expect("Could not get base directories");
  let cache_dir = base_dirs.cache_dir().join("rust_llm_mcp_server_cache"); // Ensure this matches mcp_server if it also constructs path
  if !cache_dir.exists() {
      std::fs::create_dir_all(&cache_dir).expect("Could not create main cache directory");
  }
  let rustdoc_json_output_dir = cache_dir.join("rustdoc_json_outputs");
  if !rustdoc_json_output_dir.exists() {
      std::fs::create_dir_all(&rustdoc_json_output_dir).expect("Could not create rustdoc_json_output_dir for AppState");
  }

  let app_state_instance = Arc::new(mcp_server::AppState::new(rustdoc_json_output_dir));


  // Pass the same AppState instance to the MCP server
  mcp_server::init_mcp_server(app_state_instance.clone());

  tauri::Builder::default()
    .manage(app_state_instance) // Add AppState to Tauri's managed state
    .plugin(tauri_plugin_opener::init())
    .invoke_handler(tauri::generate_handler![
        greet,
        commands::invoke_process_rust_project,
        commands::invoke_query_documentation,
        commands::get_processed_project_list
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
