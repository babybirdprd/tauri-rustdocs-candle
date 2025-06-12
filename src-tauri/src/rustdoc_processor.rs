use anyhow::{Context, Result};
use serde_json::Value; // For parsing rustdoc JSON
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::fs;

// Basic structure for storing extracted documentation.
// This will likely expand as we understand the rustdoc JSON format better.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocItem {
    pub id: String, // The ItemId from rustdoc JSON
    pub crate_name: String,
    pub name: String,
    pub path: Vec<String>, // Module path + item name
    pub description: Option<String>,
    pub item_type: String, // e.g., "function", "struct", "module"
    pub full_path_str: String, // e.g., my_crate::module::MyStruct
}

// A collection of docs for a whole crate
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CrateDocs {
    pub crate_name: String,
    pub items: HashMap<String, DocItem>, // Keyed by full_path_str for easy lookup
    // Could also store the original rustdoc JSON path or root module ID
}

/// Executes rustdoc to generate documentation JSON for a given project path.
///
/// # Arguments
/// * `project_path`: Path to the root of the Rust project (where Cargo.toml is).
/// * `target_dir`: A directory where the rustdoc JSON output should be stored.
///
/// # Returns
/// Path to the generated JSON file.
pub fn generate_rustdoc_json(project_path: &Path, target_dir: &Path) -> Result<PathBuf> {
    log::info!(
        "Generating rustdoc JSON for project at: {}",
        project_path.display()
    );

    if !project_path.join("Cargo.toml").exists() {
        return Err(anyhow::anyhow!(
            "Cargo.toml not found in project path: {}",
            project_path.display()
        ));
    }

    // Ensure target_dir exists
    fs::create_dir_all(target_dir)
        .with_context(|| format!("Failed to create target directory: {}", target_dir.display()))?;

    // Determine a unique name for the output file, perhaps based on crate name or a hash
    // For now, let's use a fixed name, but this needs to be more robust for multiple crates.
    // Ideally, we'd parse Cargo.toml to get the crate name first.
    let crate_name_fallback = project_path.file_name().unwrap_or_default().to_string_lossy().to_string();
    // let output_file_name = format!("{}.json", crate_name); // Will use determined_crate_name later
    // let output_path = target_dir.join(output_file_name); // Will define later


    // Using nightly toolchain explicitly for potentially unstable rustdoc JSON format.
    // Users might need to have `nightly` toolchain installed: `rustup toolchain install nightly`
    // Alternatively, try to use the default `rustdoc` and hope the JSON format is stable enough.
    // For broad compatibility, let's first try with the default `rustdoc`.
    // If specific nightly features are needed later, this can be changed.
    // let rustdoc_executable = "rustdoc";

    // Simpler approach: use `cargo rustdoc`
    let cargo_cmd = Command::new("cargo");
    let mut cargo_cmd_configured = cargo_cmd;
    cargo_cmd_configured
        .current_dir(project_path)
        .arg("+nightly") // Using nightly for -Z unstable-options
        .arg("rustdoc")
        .arg("-q") // quiet mode for cargo
        .arg("--lib") // Assuming we are primarily interested in the library part of a crate
                      // For workspaces or multiple targets, this might need to be more specific.
        .arg("--") // Separator for arguments to rustdoc itself
        .arg("-Z").arg("unstable-options")
        .arg("--output-format").arg("json")
        .arg("--document-private-items"); // Optional

    // The output of `cargo rustdoc -- --output-format json` goes into `target/doc/your_crate.json` by default.
    // We need to capture this output or make it predictable.
    // A common way is to use `cargo rustdoc` which places it in `target/doc/<crate_name>.json`.
    // Let's try to predict this path.
    // First, get crate name from Cargo.toml (simplified)
    // A proper Cargo.toml parser would be better (e.g., the `cargo_toml` crate).
    let manifest_path = project_path.join("Cargo.toml");
    let manifest_content = fs::read_to_string(&manifest_path) // Added borrow here
        .with_context(|| format!("Failed to read Cargo.toml from {}", project_path.display()))?;

    let parsed_manifest: toml::Value = manifest_content.parse()
        .with_context("Failed to parse Cargo.toml")?;

    let determined_crate_name = parsed_manifest.get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or(&crate_name_fallback) // Fallback to directory name
        .replace("-", "_"); // Rustdoc often replaces hyphens with underscores in the output JSON file name.

    let output_file_name = format!("{}.json", determined_crate_name);
    let output_path = target_dir.join(output_file_name);


    let default_rustdoc_json_path = project_path
        .join("target")
        .join("doc")
        .join(format!("{}.json", determined_crate_name));

    log::info!(
        "Attempting to run: cargo +nightly rustdoc -q --lib -- -Z unstable-options --output-format json --document-private-items in directory {}",
        project_path.display()
    );
    // For logging, it's better to reconstruct the command string or log args separately
    // log::info!("Cargo command args: {:?}", cargo_cmd_configured.get_args().collect::<Vec<_>>());

    let output = cargo_cmd_configured
        .output()
        .context("Failed to execute `cargo rustdoc` command")?;

    if !output.status.success() {
        log::error!(
            "`cargo rustdoc` failed: STDOUT: {}, STDERR: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(anyhow::anyhow!(
            "`cargo rustdoc` command failed. STDERR: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    log::info!(
        "`cargo rustdoc` STDOUT: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    log::info!(
        "`cargo rustdoc` STDERR: {}", // Often progress/warnings go here
        String::from_utf8_lossy(&output.stderr)
    );

    if !default_rustdoc_json_path.exists() {
        log::error!("Expected rustdoc JSON output not found at: {}. Check rustdoc output.", default_rustdoc_json_path.display());
        // List files in target/doc to help debug
        let doc_dir = project_path.join("target").join("doc");
        if doc_dir.exists() {
            log::info!("Contents of {}:", doc_dir.display());
            for entry in fs::read_dir(doc_dir)? {
                let entry = entry?;
                log::info!("  {}", entry.path().display());
            }
        }
        return Err(anyhow::anyhow!(
            "rustdoc JSON output not found at expected path: {}. Check `cargo rustdoc` execution details.", default_rustdoc_json_path.display()
        ));
    }

    // Move the generated file to our target_dir
    fs::rename(&default_rustdoc_json_path, &output_path).with_context(|| {
        format!(
            "Failed to move rustdoc JSON from {} to {}",
            default_rustdoc_json_path.display(),
            output_path.display()
        )
    })?;

    log::info!(
        "Successfully generated rustdoc JSON at: {}",
        output_path.display()
    );
    Ok(output_path)
}

/// Parses the rustdoc JSON file and extracts documentation items.
/// (This is a complex part and will be an initial, simplified version)
pub fn parse_rustdoc_json_file(json_path: &Path) -> Result<CrateDocs> {
    log::info!("Parsing rustdoc JSON from: {}", json_path.display());
    let content = fs::read_to_string(json_path)
        .with_context(|| format!("Failed to read rustdoc JSON file: {}", json_path.display()))?;

    let root: Value = serde_json::from_str(&content)
        .with_context("Failed to parse rustdoc JSON content")?;

    // The rustdoc JSON format is complex. We need to navigate it.
    // Key parts: "index" (map of ItemId to Item), "paths" (map of ItemId to path info), "format_version"
    // "root" (ItemId of the root module of the crate)
    // See: https://rust-lang.github.io/rfcs/2963-rustdoc-json.html (though it might be outdated)
    // And: https://github.com/rust-lang/rust/blob/master/src/librustdoc/json/conversions.rs for the current structure.

    let index = root.get("index").and_then(|i| i.as_object()).context("Missing 'index' in rustdoc JSON")?;
    let paths = root.get("paths").and_then(|p| p.as_object()).context("Missing 'paths' in rustdoc JSON")?;
    let crate_id_val = root.get("root").and_then(|r| r.as_str()).context("Missing 'root' crate ID in rustdoc JSON")?; // Renamed to avoid conflict

    let root_item = index.get(crate_id_val).context("Root crate item not found in index")?; // Use crate_id_val
    let crate_name = root_item.get("name").and_then(|n| n.as_str()).unwrap_or("unknown_crate").to_string();

    let mut items_map = HashMap::new();

    for (item_id, item_json) in index {
        let name = item_json.get("name").and_then(|n| n.as_str());
        let docs = item_json.get("docs").and_then(|d| d.as_str());
        let kind = item_json.get("kind").and_then(|k| k.as_str()).unwrap_or("unknown");

        // Visibility check might be needed if not using --document-private-items
        // let visibility = item_json.get("visibility").and_then(|v| v.as_str()).unwrap_or("public");
        // if visibility != "public" { continue; }

        let path_info = paths.get(item_id);
        let path_array: Vec<String> = path_info
            .and_then(|pi| pi.get("path"))
            .and_then(|p_arr| p_arr.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let mut full_path_parts = vec![crate_name.clone()];
        full_path_parts.extend(path_array.clone());

        // Only add item name to path if it's not a module (modules already have their name in path_array from rustdoc)
        // Or if it's a module, path_array might already contain its own name if it's a sub-module.
        // The name from item_json.get("name") is the item's own name.
        // The path_array is the module path *to* the item.
        // So, for an item `my_mod::my_func`, path_array could be `["my_mod"]` and name `my_func`.
        // Or for `my_mod::sub_mod::MyStruct`, path_array could be `["my_mod", "sub_mod"]` and name `MyStruct`.
        // If the item is a module itself, e.g. `my_mod::sub_mod`, path_array `["my_mod"]` and name `sub_mod`.
        // This logic seems correct:
        if let Some(item_name_str) = name {
             if kind != "module" || !path_array.last().map_or(false, |last_part| last_part == item_name_str) {
                full_path_parts.push(item_name_str.to_string());
            }
        }
        let full_path_str = full_path_parts.join("::");


        if let Some(item_name) = name {
            // Only include items that are not "hidden" or implicitly generated (e.g. some impls)
            // This filtering needs refinement based on what's useful.
            if item_json.get("inner").and_then(|i| i.get("is_stripped")).and_then(|s| s.as_bool()).unwrap_or(false) {
                 // Skip "stripped" items, often private items that weren't fully documented due to privacy even with --document-private-items
                 // This can happen if a public item re-exports a private one without `#[doc(inline)]` or similar.
                 // Or it might be an item that rustdoc couldn't fully resolve/render.
                 // log::trace!("Skipping stripped item: {} ({})", full_path_str, item_id);
                 // continue;
            }


            let doc_item = DocItem {
                id: item_id.clone(),
                crate_name: crate_name.clone(),
                name: item_name.to_string(),
                path: path_array, // This is the module path, not including the item name itself
                description: docs.map(String::from),
                item_type: kind.to_string(),
                full_path_str: full_path_str.clone(),
            };
            items_map.insert(full_path_str, doc_item);
        }
    }

    log::info!("Successfully parsed {} items from {}", items_map.len(), json_path.display());

    Ok(CrateDocs {
        crate_name,
        items: items_map,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // This test requires a sample Rust project with a Cargo.toml and src/lib.rs
    // For now, it's more of an integration test that would need setup.
    // We can create a dummy project structure for testing.
    fn create_dummy_project(dir: &Path, crate_name: &str) -> Result<()> {
        let src_dir = dir.join("src");
        fs::create_dir_all(&src_dir)?;
        fs::write(dir.join("Cargo.toml"), format!("[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[lib]
name = "{}"
path = "src/lib.rs"
", crate_name, crate_name.replace("-", "_")))?; // Ensure lib name is valid
        fs::write(src_dir.join("lib.rs"), "/// A test function
pub fn hello() -> &'static str { "hello" }
/// A test struct
pub struct TestStruct { pub field: i32 }

pub mod my_module {
    /// A function inside a module
    pub fn goodbye() {}
}
")?;
        Ok(())
    }

    #[tokio::test]
    #[ignore] // Ignored because it runs `cargo rustdoc` and needs a nightly toolchain and project setup
              // To run: `cargo test -- --ignored rustdoc_processor::tests::test_generate_and_parse_rustdoc`
              // Ensure `rustup toolchain install nightly` has been run.
    async fn test_generate_and_parse_rustdoc() -> Result<()> {
        let temp_project_dir = tempdir().context("Failed to create temp project dir")?;
        let crate_name = "my-test-crate"; // Use hyphen to test replacement
        create_dummy_project(temp_project_dir.path(), crate_name)?;

        let temp_output_dir = tempdir().context("Failed to create temp output dir")?;

        // Note: This test relies on `cargo +nightly rustdoc ...` succeeding.
        // It might fail if the nightly toolchain is not installed or if there are
        // issues with the rustdoc JSON output on the specific nightly version.

        let json_path = generate_rustdoc_json(temp_project_dir.path(), temp_output_dir.path())?;
        assert!(json_path.exists(), "JSON file should be generated");

        let crate_docs = parse_rustdoc_json_file(&json_path)?;
        assert_eq!(crate_docs.crate_name, crate_name.replace("-", "_"), "Crate name should match and be sanitized");

        // Check for specific items (adjust paths based on actual rustdoc output)
        let expected_hello_path = format!("{}::hello", crate_name.replace("-", "_"));
        let expected_struct_path = format!("{}::TestStruct", crate_name.replace("-", "_"));
        let expected_goodbye_path = format!("{}::my_module::goodbye", crate_name.replace("-", "_"));

        assert!(crate_docs.items.contains_key(&expected_hello_path), "Should contain hello function");
        let hello_fn = crate_docs.items.get(&expected_hello_path).unwrap();
        assert_eq!(hello_fn.name, "hello");
        assert_eq!(hello_fn.item_type, "function");
        assert_eq!(hello_fn.description.as_deref(), Some("A test function"));

        assert!(crate_docs.items.contains_key(&expected_struct_path), "Should contain TestStruct");
        let test_struct = crate_docs.items.get(&expected_struct_path).unwrap();
        assert_eq!(test_struct.name, "TestStruct");
        assert_eq!(test_struct.item_type, "struct");
        assert_eq!(test_struct.description.as_deref(), Some("A test struct"));

        assert!(crate_docs.items.contains_key(&expected_goodbye_path), "Should contain my_module::goodbye function");
        let goodbye_fn = crate_docs.items.get(&expected_goodbye_path).unwrap();
        assert_eq!(goodbye_fn.name, "goodbye");
        assert_eq!(goodbye_fn.item_type, "function");
        assert_eq!(goodbye_fn.path, vec!["my_module"]); // Path to item, not including item name itself for functions/structs etc.
        assert_eq!(goodbye_fn.description.as_deref(), Some("A function inside a module"));


        // Test a module item if present (rustdoc JSON includes modules as items)
        let expected_module_path = format!("{}::my_module", crate_name.replace("-", "_"));
         if let Some(module_item) = crate_docs.items.get(&expected_module_path) {
            assert_eq!(module_item.name, "my_module");
            assert_eq!(module_item.item_type, "module");
        } else {
            // This might or might not be an error depending on how rustdoc versions list modules.
            // Sometimes the path in `paths` for a module points to itself.
            log::warn!("Module item {} not found directly in items map. This might be okay.", expected_module_path);
        }


        temp_project_dir.close()?;
        temp_output_dir.close()?;
        Ok(())
    }
}
