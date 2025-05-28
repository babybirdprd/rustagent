use wasm_bindgen::prelude::*;
use crate::agent::AgentSystem;
use web_sys; // Ensure web_sys is imported for console logging
#[cfg(debug_assertions)]
use console_error_panic_hook; // For better panic messages

mod agent;
mod llm;

// Expose RustAgent to JavaScript
#[wasm_bindgen]
pub struct RustAgent {
    agents: AgentSystem,
    api_url: Option<String>,
    model_name: Option<String>,
    api_key: Option<String>,
}

#[wasm_bindgen]
impl RustAgent {
    #[wasm_bindgen(constructor)]
    pub fn new() -> RustAgent {
        RustAgent {
            agents: AgentSystem::new(),
            api_url: None,
            model_name: None,
            api_key: None,
        }
    }

    #[wasm_bindgen]
    pub fn set_llm_config(&mut self, api_url: String, model_name: String, api_key: String) {
        self.api_url = Some(api_url);
        self.model_name = Some(model_name);
        self.api_key = Some(api_key);
    }

use serde::{Serialize, Deserialize}; // For serializing/deserializing results

// ... (rest of the existing imports)

// Define a serializable struct for results if needed for more complex data,
// but Vec<Result<String, String>> can be serialized directly by serde_json.

// ... (RustAgent struct and new/set_llm_config methods remain the same)

    // Example method: Automate a task by delegating to agents
    #[wasm_bindgen]
    pub async fn automate(&self, tasks_json: String) -> Result<JsValue, JsValue> {
        // 1. LLM Configuration Check
        let (api_key, api_url, model_name) = match (&self.api_key, &self.api_url, &self.model_name) {
            (Some(k), Some(u), Some(m)) => (k, u, m),
            _ => return Err(JsValue::from_str("LLM configuration not set. Please call set_llm_config first.")),
        };

        // 2. Parse tasks_json
        let tasks: Vec<String> = match serde_json::from_str(&tasks_json) {
            Ok(parsed_tasks) => parsed_tasks,
            Err(_) => return Err(JsValue::from_str("Invalid JSON task list. Expected an array of strings.")),
        };

        if tasks.is_empty() {
            return Err(JsValue::from_str("Task list is empty."));
        }

        // 3. Iterate through tasks and execute
        let mut results_list: Vec<Result<String, String>> = Vec::new();
        let mut previous_result: Option<String> = None;

        for task_template in tasks {
            let current_task_string = if let Some(ref prev_res) = previous_result {
                // Only replace if previous task was successful.
                // If previous task failed, its "result" is an error message which might not be suitable for substitution.
                // Or, we could define that {{PREVIOUS_RESULT}} always uses the raw string from the previous step.
                // For now, let's assume successful result substitution.
                // If the previous_result itself was an Err, this replacement won't happen if we store Result in previous_result.
                // Let's refine: previous_result will store the Ok value of the last successful task.
                task_template.replace("{{PREVIOUS_RESULT}}", prev_res)
            } else {
                task_template.clone()
            };
            
            web_sys::console::log_1(&format!("Executing processed task: {}", current_task_string).into());

            match self.agents.run_task(&current_task_string, api_key, api_url, model_name).await {
                Ok(result_string) => {
                    previous_result = Some(result_string.clone()); // Store successful result for next iteration
                    results_list.push(Ok(result_string));
                }
                Err(error_string) => {
                    // If a task fails, we clear previous_result so {{PREVIOUS_RESULT}} isn't filled from a failure.
                    previous_result = None; 
                    results_list.push(Err(error_string));
                    // Optional: Stop execution on first error
                    // return Err(JsValue::from_str(&format!("Task failed: {}", error_string))); 
                }
            }
        }

        // 4. Serialize results_list and return
        match serde_json::to_string(&results_list) {
            Ok(json_results) => Ok(JsValue::from_str(&json_results)),
            Err(e) => Err(JsValue::from_str(&format!("Failed to serialize results: {}", e))),
        }
    }
}

// Initialize WASM module and log to console
#[wasm_bindgen(start)]
pub fn run() -> Result<(), JsValue> {
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once(); // Better panic messages in browser
    web_sys::console::log_1(&"RustAgent WASM module initialized!".into());
    Ok(())
}