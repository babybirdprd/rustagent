use wasm_bindgen::prelude::*;
use crate::agent::AgentSystem;

mod agent;
mod llm;

// Expose RustAgent to JavaScript
#[wasm_bindgen]
pub struct RustAgent {
    agents: AgentSystem,
}

#[wasm_bindgen]
impl RustAgent {
    #[wasm_bindgen(constructor)]
    pub fn new() -> RustAgent {
        RustAgent {
            agents: AgentSystem::new(),
        }
    }

    // Example method: Automate a task by delegating to agents
    #[wasm_bindgen]
    pub fn automate(&mut self, task: &str) -> String {
        self.agents.run_task(task)
    }
}

// Initialize WASM module and log to console
#[wasm_bindgen(start)]
pub fn run() -> Result<(), JsValue> {
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once(); // Better panic messages in browser
    web_sys::console::log_1(&"RustAgent initialized!".into());
    Ok(())
}