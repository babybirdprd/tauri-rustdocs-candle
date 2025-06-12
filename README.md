# RustDoc LLM Server - Tauri Edition

RustDoc LLM Server is a Tauri-based desktop application that processes your local Rust project documentation, generates semantic embeddings using the `BAAI/bge-code-v1` model, and provides an interface for advanced semantic querying. It's designed to help you understand and navigate Rust codebases more effectively. While it can work standalone for documentation processing and semantic search, it's also designed to integrate with local LLM inference servers like `candle-vllm` for future enhancements.

## Features

*   **Local Rust Project Processing:** Extracts documentation directly from your Rust projects using `rustdoc`.
*   **Advanced Semantic Embeddings:** Generates high-quality semantic embeddings for documentation items using the `BAAI/bge-code-v1` model, which is loaded and run locally within the Tauri backend.
*   **MCP Server:** Includes a Model Context Protocol (MCP) server, compliant with [MCP RFC 0001](https://github.com/rust-mcp/mcp-specs/blob/main/rfcs/0001-model-context-protocol.md). This server runs on `http://127.0.0.1:3001` by default and exposes tools for programmatic interaction by LLMs or other developer utilities.
*   **Semantic Query Interface:** Allows you to ask natural language questions about your Rust documentation and receive relevant code items based on semantic similarity.
*   **Cross-Platform Tauri UI:** Provides a user-friendly interface for:
    *   Managing a list of local Rust projects.
    *   Processing documentation and generating embeddings for these projects.
    *   Querying the processed documentation via a dedicated search page.
*   **`candle-vllm` Integration (Future):** Designed to work with local LLM inference servers like `candle-vllm` for features such as AI-powered summarization or more complex question-answering (these features are planned for future versions).

## Prerequisites

Before you begin, ensure your system meets the following requirements:

*   **Rust Toolchain (Stable):** Essential for building the Tauri application and its Rust backend. Install from [rust-lang.org](https://rust-lang.org/).
*   **Rust Nightly Toolchain:** Required because the backend uses `cargo +nightly rustdoc -Z unstable-options --output-format json` to generate documentation data. Install using:
    ```bash
    rustup toolchain install nightly
    ```
*   **Node.js and pnpm:** Needed for the Next.js frontend. This project uses `pnpm` for package management.
    *   Install Node.js (LTS recommended) from [nodejs.org](https://nodejs.org/).
    *   Install pnpm globally: `npm install -g pnpm` (For other installation methods, see [pnpm installation guide](https://pnpm.io/installation)).
*   **Tauri CLI:** The command-line interface for Tauri development.
    ```bash
    cargo install tauri-cli
    ```
*   **Git:** For cloning this repository and, if needed, other dependencies like `candle-vllm`.
*   **(Optional) GPU Acceleration:** For significantly faster performance with `candle-vllm` (if used) and the local `BAAI/bge-code-v1` embedding model:
    *   **NVIDIA GPUs:** Install the CUDA Toolkit. Visit the [NVIDIA developer website](https://developer.nvidia.com/cuda-toolkit) for downloads and installation instructions.
    *   **Apple Silicon (macOS):** Ensure your macOS is up-to-date, as Metal support is built into the OS.
    The local embedding service in this application will automatically attempt to use CUDA if available and compiled with CUDA support, otherwise, it will fall back to CPU.

## Setup and Running

### 1. `candle-vllm` Server (Optional External LLM)

The core features of this application (documentation processing, embedding, semantic search) **do not** require `candle-vllm`. However, `candle-vllm` is required if you wish to use or develop features that leverage a local Large Language Model via an OpenAI-compatible API (e.g., for future query summarization or advanced Q&A capabilities).

**If you plan to use `candle-vllm`:**

*   **Refer to the Official Project:** For the most detailed and up-to-date setup instructions, please visit the `candle-vllm` repository: [https://github.com/EricLBuehler/candle-vllm](https://github.com/EricLBuehler/candle-vllm).
*   **Brief Summary:**
    1.  **Clone `candle-vllm`:**
        ```bash
        git clone https://github.com/EricLBuehler/candle-vllm.git
        cd candle-vllm
        ```
    2.  **Build `candle-vllm`:** (Example for CUDA, adapt for your hardware)
        ```bash
        cargo build --release --features cuda,nccl
        ```
    3.  **Download a Compatible LLM:** GGUF format models are recommended (e.g., Llama 2, Mistral, Mixtral). Download from Hugging Face.
        ```bash
        # Example:
        mkdir models
        wget https://huggingface.co/TheBloke/Llama-2-7B-Chat-GGUF/resolve/main/llama-2-7b-chat.Q4_K_M.gguf -P models/
        ```
    4.  **Run the `candle-vllm` Server:**
        ```bash
        # Adjust paths and model details as necessary
        target/release/candle-vllm --model-id TheBloke/Llama-2-7B-Chat-GGUF --weight-file ./models/llama-2-7b-chat.Q4_K_M.gguf --quant gguf --port 8080 --openai-api
        ```
        Keep this server running in a separate terminal. The port `8080` is an example; this application does not currently make direct calls to `candle-vllm` but may in the future or via MCP tools.

### 2. This Tauri Application (RustDoc LLM Server)

*   **A. Clone This Repository:**
    ```bash
    git clone https://github.com/your-username/rustdoc-llm-server-tauri.git # Replace with the actual repository URL
    cd rustdoc-llm-server-tauri # Replace with the actual repository name
    ```

*   **B. Install Frontend Dependencies:**
    Navigate to the root of the cloned repository:
    ```bash
    pnpm install
    ```

*   **C. Run in Development Mode:**
    ```bash
    cargo tauri dev
    ```
    *   **IMPORTANT (First Run):** The first time the backend starts (triggered by `cargo tauri dev`), it will automatically download the `BAAI/bge-code-v1` embedding model files from Hugging Face Hub. This model is sharded and can be several gigabytes in size. This process requires a stable internet connection and may take some time. Subsequent application startups will use the locally cached model files.
    *   The application UI should open, allowing you to manage projects and query documentation.

*   **D. Building for Production:**
    To create a standalone, distributable application bundle:
    ```bash
    cargo tauri build
    ```
    The bundled application will be located in `src-tauri/target/release/bundle/`.

## Using the Application

1.  **Project Management Page (`/projects`):**
    *   **Add Project:** In the input field, provide the **absolute path** to a local Rust project directory (this directory must contain a `Cargo.toml` file). Click "Add Project".
    *   **Process Project:** Once a project is added to the list, click its "Process" button. This action initiates the following backend tasks:
        1.  Generation of comprehensive documentation data using `cargo +nightly rustdoc`.
        2.  Parsing of this data to identify all relevant documentation items (functions, structs, traits, etc.).
        3.  Generation of semantic vector embeddings for each item's description using the `BAAI/bge-code-v1` model.
        4.  Storage of the processed documentation and embeddings in the application's memory for the current session.
    *   The UI will display the status of each project (`idle`, `processing`, `processed`, or `error`).

2.  **Query Page (`/query`):**
    *   **Enter Query:** Type a natural language question or keyword phrase related to the Rust code you've processed (e.g., "how to handle results in a function", "implementing the Display trait", "example of using Arc<Mutex<T>>").
    *   **Select Project (Optional):** If you have processed multiple projects, a dropdown menu allows you to focus your query on a single project or search across all processed projects.
    *   **Search:** Click the "Search Documentation" button.
    *   **View Results:** The system embeds your query and performs a semantic similarity search against the stored documentation embeddings. The most relevant items are displayed, along with their full path, type, a snippet of their description, and the similarity score.

## Backend Services

*   **MCP Server:**
    *   An MCP (Model Context Protocol) server is automatically started by the Tauri application's backend.
    *   It listens on `http://127.0.0.1:3001` by default.
    *   This server exposes tools (e.g., `process_rust_project`, `query_documentation`, `get_raw_documentation`) that can be invoked programmatically. This is primarily intended for interaction with LLM agents or other developer tools that support the MCP specification. The Tauri UI itself uses similar logic but interacts via direct Rust function calls (Tauri commands) rather than HTTP calls to this MCP server.

*   **Embedding Service:**
    *   The `BAAI/bge-code-v1` model is loaded and managed directly by the Tauri application's backend.
    *   This service is responsible for generating the vector embeddings used in semantic search, both when processing projects and when interpreting user queries.
    *   It will attempt to use CUDA for GPU acceleration if available and compiled with support, otherwise, it will operate on the CPU.

## Troubleshooting / Notes

*   **`candle-vllm` Dependency:** Core features (project processing, embedding, semantic search via UI) work without `candle-vllm`. It's only needed for potential future features like LLM-based summarization of search results.
*   **Processing Duration:** Processing a large Rust project for the first time can be lengthy due to `rustdoc` generation and comprehensive embedding of all documentation items. Subsequent processing of the same project (if re-triggered) should be faster if `rustdoc` output is cached by Cargo, but embeddings will still be regenerated.
*   **Internet for Model Download:** A stable internet connection is crucial for the first run of the application backend, as it needs to download the `BAAI/bge-code-v1` model files (which can be large).
*   **Disk Space:** Ensure you have sufficient disk space for the embedding model (cached by `hf-hub` typically in `~/.cache/huggingface/hub/`) and for `rustdoc` build artifacts in your projects' `target` directories.
*   **Nightly Toolchain for `rustdoc`:** The application specifically uses `cargo +nightly rustdoc`. If the nightly toolchain is not installed or accessible, the "Process Project" step will fail. Check logs for errors related to `rustdoc` execution.
*   **Application Logs:** Check the terminal output where you ran `cargo tauri dev` for detailed logs from both the frontend and backend, including messages from `hf-hub` during model downloads or `candle` during model operations. These logs are invaluable for diagnosing issues.
