# RustAgent

![RustAgent Logo](ra_logo.png)

**RustAgent** is an open-source, browser-based web automation tool written in Rust and compiled to WebAssembly (WASM). It serves as a free alternative to tools like OpenAI Operator, offering flexible LLM (Large Language Model) integration and a multi-agent system for automating web tasks. Whether you need to fill forms, navigate pages, or scrape data, RustAgent aims to provide a lightweight, extensible solution.

## Features
- **Runs in the Browser**: Powered by WebAssembly for seamless client-side execution.
- **Multi-Agent System**: Coordinate tasks across multiple agents (e.g., navigators, form-fillers).
- **Flexible LLM Support**: Integrate with any LLM via API (placeholder included; customize as needed).
- **Rust-Powered**: Leverages Rust’s performance, safety, and concurrency.
- **Open Source**: Free to use, modify, and contribute to.

## Installation

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (install via `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- WASM target: `rustup target add wasm32-unknown-unknown`
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) (`cargo install wasm-pack`)
- A local HTTP server (e.g., `python -m http.server`)

### Build
1. Clone the repository:
   ```bash
   git clone https://github.com/makalin/rustagent.git
   cd rustagent
   ```
2. Build the WASM module:
   ```bash
   wasm-pack build --target web
   ```
3. Serve the project:
   ```bash
   python -m http.server 8000
   ```
4. Open `http://localhost:8000` in your browser.

## Usage
1. Open `index.html` in a browser.
2. Click the "Run Automation" button to execute a sample task (e.g., "fill out login form").
3. Check the result displayed on the page.

### Example
The default demo delegates a task to an agent, which calls a placeholder LLM. Customize `src/llm.rs` to connect to your preferred LLM API.

```javascript
// JavaScript (in index.html)
const agent = new RustAgent();
const result = agent.automate("fill out login form");
console.log(result); // "Agent 1 completed task: LLM response to 'Agent 1 (navigator): fill out login form'"
```

## Project Structure
```
rustagent/
├── Cargo.toml       # Rust dependencies and config
├── src/
│   ├── lib.rs       # WASM entry point
│   ├── agent.rs     # Multi-agent system
│   └── llm.rs       # LLM integration
└── index.html       # Browser demo
```

## Roadmap
- DOM manipulation via `web-sys` for real web automation.
- Configurable LLM endpoints (e.g., Hugging Face, local models).
- Advanced multi-agent coordination.
- User-friendly UI for task definition.

## Contributing
We welcome contributions! Here’s how to get started:
1. Fork the repository.
2. Create a branch: `git checkout -b feature/your-feature`.
3. Commit changes: `git commit -m "Add your feature"`.
4. Push to your fork: `git push origin feature/your-feature`.
5. Open a Pull Request.

Please follow the [Code of Conduct](CODE_OF_CONDUCT.md) (TBD) and check [issues](https://github.com/<your-username>/rustagent/issues) for tasks.

## License
RustAgent is licensed under the [MIT License](LICENSE). Feel free to use, modify, and distribute it.

## Acknowledgments
- Built with [Rust](https://www.rust-lang.org/) and [wasm-bindgen](https://rustwasm.github.io/wasm-bindgen/).
- Inspired by the need for open-source, browser-based automation tools.
