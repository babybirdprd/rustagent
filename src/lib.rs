use wasm_bindgen::prelude::*;
use crate::agent::AgentSystem;
use web_sys; // Ensure web_sys is imported for console logging
#[cfg(debug_assertions)]
use console_error_panic_hook; // For better panic messages

mod agent;
mod llm;
mod dom_utils; // Declare dom_utils module

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
        let mut previous_task_successful_output: Option<String> = None; // Renamed for clarity

        for original_task_template in tasks { // Renamed for clarity
            web_sys::console::log_1(&format!("Original task template: {}", original_task_template).into());

            let current_task_string: String;
            if original_task_template.contains("{{PREVIOUS_RESULT}}") {
                let replacement_value = previous_task_successful_output.as_deref().unwrap_or("");
                web_sys::console::log_1(&format!("Placeholder {{PREVIOUS_RESULT}} found. Replacing with: '{}'", replacement_value).into());
                current_task_string = original_task_template.replace("{{PREVIOUS_RESULT}}", replacement_value);
            } else {
                current_task_string = original_task_template.clone();
            }
            
            web_sys::console::log_1(&format!("Executing task (after substitution): {}", current_task_string).into());

            match self.agents.run_task(&current_task_string, api_key, api_url, model_name).await {
                Ok(result_string) => {
                    web_sys::console::log_1(&format!("Task succeeded. Storing for {{PREVIOUS_RESULT}}: {}", result_string).into());
                    previous_task_successful_output = Some(result_string.clone());
                    results_list.push(Ok(result_string));
                }
                Err(error_string) => {
                    web_sys::console::log_1(&format!("Task failed. Clearing {{PREVIOUS_RESULT}}. Error: {}", error_string).into());
                    previous_task_successful_output = None; 
                    results_list.push(Err(error_string));
                    // Optional: Stop execution on first error can be added here if desired
                    // For example: return Err(JsValue::from_str(&format!("Task failed: {}", error_string))); 
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

use serde::{Serialize, Deserialize}; // For serializing/deserializing results

// Initialize WASM module and log to console
#[wasm_bindgen(start)]
pub fn run() -> Result<(), JsValue> {
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once(); // Better panic messages in browser
    web_sys::console::log_1(&"RustAgent WASM module initialized!".into());
    Ok(())
}

#[cfg(test)]
#[cfg(feature = "mock-llm")] // Ensure mock-llm is active for these tests
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    use serde_json::Value; // For parsing JSON results

    wasm_bindgen_test_configure!(run_in_browser);

    fn setup_agent() -> RustAgent {
        let mut agent = RustAgent::new();
        agent.set_llm_config(
            "dummy_url".to_string(),
            "dummy_model".to_string(),
            "dummy_key".to_string(),
        );
        agent
    }

    #[wasm_bindgen_test]
    async fn test_automate_single_task_no_placeholder() {
        let agent = setup_agent();
        let tasks_json = serde_json::to_string(&vec!["click #first_button"]).unwrap();
        
        let result_js = agent.automate(tasks_json).await.unwrap();
        let result_str = result_js.as_string().unwrap();
        let results: Vec<Result<String, String>> = serde_json::from_str(&result_str).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        // Mock for "click #first_button" returns "Clicked #first_button" (simple string)
        // which agent.rs then wraps in "Agent X completed task via LLM: Clicked #first_button"
        // The agent selection logic in agent.rs should pick Generic Agent (3) for "click #first_button"
        // if parse_dom_command returns None.
        // The mock in llm.rs for "click #first_button" returns "Clicked #first_button"
        // This is then processed by agent.rs. If "Clicked #first_button" is NOT a JSON array of commands,
        // it will be wrapped as "Agent 3 (Generic) completed task via LLM: Clicked #first_button"
        assert_eq!(results[0].as_ref().unwrap(), "Agent 3 (Generic) completed task via LLM: Clicked #first_button");
    }

    #[wasm_bindgen_test]
    async fn test_automate_two_tasks_second_uses_placeholder_successfully() {
        let agent = setup_agent();
        let tasks = vec![
            "get text from #element", // Mock returns "Text from #element" (simple string)
            "TYPE css:#input {{PREVIOUS_RESULT}}" // Mock for "TYPE css:#input Text from #element" returns JSON command
        ];
        let tasks_json = serde_json::to_string(&tasks).unwrap();

        let result_js = agent.automate(tasks_json).await.unwrap();
        let result_str = result_js.as_string().unwrap();
        let results: Vec<Result<String, String>> = serde_json::from_str(&result_str).unwrap();
        
        assert_eq!(results.len(), 2);
        // Task 1: "get text from #element" -> LLM returns "Text from #element"
        // Agent 3 (Generic) selected for "get text from #element"
        assert_eq!(results[0].as_ref().unwrap(), "Agent 3 (Generic) completed task via LLM: Text from #element");

        // Task 2: "TYPE css:#input Text from #element"
        // LLM returns "[{\"action\": \"TYPE\", \"selector\": \"css:#input\", \"value\": \"Text from #element\"}]"
        // This is a JSON command, so agent.rs's run_task will try to execute it.
        // The execution will fail because "css:#input" doesn't exist.
        // The result of this execution will be a JSON string itself: e.g., "[{\"error\":\"Error typing...\"}]"
        assert!(results[1].is_ok());
        let task2_result_str = results[1].as_ref().unwrap();
        let task2_inner_results: Vec<Result<String, String>> = serde_json::from_str(task2_result_str).unwrap();
        assert_eq!(task2_inner_results.len(), 1);
        assert!(task2_inner_results[0].is_err());
        assert!(task2_inner_results[0].as_ref().err().unwrap().contains("Error typing in element"));
        assert!(task2_inner_results[0].as_ref().err().unwrap().contains("css:#input"));
    }

    #[wasm_bindgen_test]
    async fn test_automate_two_tasks_first_fails_second_uses_placeholder() {
        let agent = setup_agent();
        let tasks = vec![
            "CLICK #nonexistent_button", // This specific task is not mocked in llm.rs to return an error directly.
                                         // Instead, it will be parsed by parse_dom_command.
                                         // The DOM command execution will fail.
            "TYPE css:#input {{PREVIOUS_RESULT}}"
        ];
        let tasks_json = serde_json::to_string(&tasks).unwrap();

        let result_js = agent.automate(tasks_json).await.unwrap();
        let result_str = result_js.as_string().unwrap();
        let results: Vec<Result<String, String>> = serde_json::from_str(&result_str).unwrap();

        assert_eq!(results.len(), 2);
        // Task 1: "CLICK #nonexistent_button" should fail during DOM execution.
        // Agent 3 (Generic) selected.
        assert!(results[0].is_err());
        assert!(results[0].as_ref().err().unwrap().contains("Agent 3 (Generic): Error clicking element:"));
        assert!(results[0].as_ref().err().unwrap().contains("#nonexistent_button"));


        // Task 2: "TYPE css:#input " (placeholder became empty string)
        // Mock for "TYPE css:#input " (with empty value) returns:
        // "[{\"action\": \"TYPE\", \"selector\": \"css:#input\", \"value\": \"\"}]"
        // This is a JSON command, agent.rs's run_task will execute it. It will fail.
        assert!(results[1].is_ok()); // The automate step is Ok, but the inner command execution is an Err
        let task2_result_str = results[1].as_ref().unwrap();
        let task2_inner_results: Vec<Result<String, String>> = serde_json::from_str(task2_result_str).unwrap();
        assert_eq!(task2_inner_results.len(), 1);
        assert!(task2_inner_results[0].is_err()); // The type command itself fails
        assert!(task2_inner_results[0].as_ref().err().unwrap().contains("Successfully typed '' in element with selector: 'css:#input'"), "Actual: {}", task2_inner_results[0].as_ref().err().unwrap());
        // Correction: The mock for "TYPE css:#input " returns a command to type empty string.
        // If the element #input doesn't exist, it will be an error "Error typing in element".
        // If it existed, it would be "Successfully typed ''..."
        // Since it doesn't exist, it should be an error.
        assert!(task2_inner_results[0].as_ref().err().unwrap().contains("Error typing in element"));
    }
    
    #[wasm_bindgen_test]
    async fn test_automate_first_task_uses_placeholder_is_empty() {
        let agent = setup_agent();
         // Mock for "TYPE css:#input " (with empty value) returns:
        // "[{\"action\": \"TYPE\", \"selector\": \"css:#input\", \"value\": \"\"}]"
        let tasks = vec!["TYPE css:#input {{PREVIOUS_RESULT}}"];
        let tasks_json = serde_json::to_string(&tasks).unwrap();

        let result_js = agent.automate(tasks_json).await.unwrap();
        let result_str = result_js.as_string().unwrap();
        let results: Vec<Result<String, String>> = serde_json::from_str(&result_str).unwrap();
        
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        let task1_result_str = results[0].as_ref().unwrap();
        let task1_inner_results: Vec<Result<String, String>> = serde_json::from_str(task1_result_str).unwrap();
        assert_eq!(task1_inner_results.len(), 1);
        assert!(task1_inner_results[0].is_err());
        assert!(task1_inner_results[0].as_ref().err().unwrap().contains("Error typing in element"));
        assert!(task1_inner_results[0].as_ref().err().unwrap().contains("css:#input"));
    }

    #[wasm_bindgen_test]
    async fn test_automate_multiple_tasks_chained_placeholders() {
        let agent = setup_agent();
        let tasks = vec![
            "click #first_button", // Mock returns "Clicked #first_button"
            "process {{PREVIOUS_RESULT}} for task B", // Mock for "process Clicked #first_button for task B" returns "Processed Clicked #first_button"
            "process {{PREVIOUS_RESULT}} for task C"  // Mock for "process Processed Clicked #first_button for task C" returns "Final result from C"
        ];
        let tasks_json = serde_json::to_string(&tasks).unwrap();

        let result_js = agent.automate(tasks_json).await.unwrap();
        let result_str = result_js.as_string().unwrap();
        let results: Vec<Result<String, String>> = serde_json::from_str(&result_str).unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].as_ref().unwrap(), "Agent 3 (Generic) completed task via LLM: Clicked #first_button");
        assert_eq!(results[1].as_ref().unwrap(), "Agent 3 (Generic) completed task via LLM: Processed Clicked #first_button");
        assert_eq!(results[2].as_ref().unwrap(), "Agent 3 (Generic) completed task via LLM: Final result from C");
    }

    #[wasm_bindgen_test]
    async fn test_automate_placeholder_produces_multicommand_json() {
        let agent = setup_agent();
        let tasks = vec![
            "get simple id", // Mock returns "element_id_123" (simple string)
            "LLM_ACTION_EXPECTING_JSON_CMDS {{PREVIOUS_RESULT}}" 
            // Mock for "LLM_ACTION_EXPECTING_JSON_CMDS element_id_123" returns
            // "[{\"action\": \"CLICK\", \"selector\": \"#element_id_123\"}, {\"action\": \"READ\", \"selector\": \"#another_element\"}]"
        ];
        let tasks_json = serde_json::to_string(&tasks).unwrap();

        let result_js = agent.automate(tasks_json).await.unwrap();
        let result_str = result_js.as_string().unwrap();
        let results: Vec<Result<String, String>> = serde_json::from_str(&result_str).unwrap();
        
        assert_eq!(results.len(), 2);
        // Task 1: "get simple id" -> LLM returns "element_id_123"
        // Agent 3 (Generic) selected.
        assert_eq!(results[0].as_ref().unwrap(), "Agent 3 (Generic) completed task via LLM: element_id_123");

        // Task 2: LLM receives "LLM_ACTION_EXPECTING_JSON_CMDS element_id_123"
        // LLM returns JSON: "[{\"action\": \"CLICK\", \"selector\": \"#element_id_123\"}, {\"action\": \"READ\", \"selector\": \"#another_element\"}]"
        // agent.rs run_task executes these. Both fail as elements don't exist.
        // The result is a JSON string of these two errors.
        assert!(results[1].is_ok());
        let task2_result_str = results[1].as_ref().unwrap();
        let task2_inner_results: Vec<Result<String, String>> = serde_json::from_str(task2_result_str).unwrap();
        assert_eq!(task2_inner_results.len(), 2);
        
        assert!(task2_inner_results[0].is_err());
        assert!(task2_inner_results[0].as_ref().err().unwrap().contains("Error clicking element: ElementNotFound: No element found for selector '#element_id_123'"));

        assert!(task2_inner_results[1].is_err());
        assert!(task2_inner_results[1].as_ref().err().unwrap().contains("Error reading text from element: ElementNotFound: No element found for selector '#another_element'"));
    }

    // Integration tests for new commands via automate()
    #[wasm_bindgen_test]
    async fn test_automate_get_url_direct_command() {
        let agent = setup_agent();
        let tasks_json = serde_json::to_string(&vec!["GET_URL"]).unwrap();
        let result_js = agent.automate(tasks_json).await.unwrap();
        let results: Vec<Result<String, String>> = serde_json::from_str(&result_js.as_string().unwrap()).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        assert!(results[0].as_ref().unwrap().contains("Agent 3 (Generic): Current URL is:"));
        assert!(results[0].as_ref().unwrap().contains("http") || results[0].as_ref().unwrap().contains("file:"));
    }

    #[wasm_bindgen_test]
    async fn test_automate_element_exists_direct_command() {
        let agent = setup_agent();
        let (_window, document) = dom_utils::get_window_document().unwrap();
        let el = dom_utils::setup_element(&document, "integ-exists-direct", "div", None);

        let tasks_true_json = serde_json::to_string(&vec!["ELEMENT_EXISTS css:#integ-exists-direct"]).unwrap();
        let result_true_js = agent.automate(tasks_true_json).await.unwrap();
        let results_true: Vec<Result<String, String>> = serde_json::from_str(&result_true_js.as_string().unwrap()).unwrap();
        assert_eq!(results_true.len(), 1);
        assert_eq!(results_true[0].as_ref().unwrap(), "Agent 3 (Generic): Element 'css:#integ-exists-direct' exists: true");

        let tasks_false_json = serde_json::to_string(&vec!["ELEMENT_EXISTS css:#integ-nonexistent-direct"]).unwrap();
        let result_false_js = agent.automate(tasks_false_json).await.unwrap();
        let results_false: Vec<Result<String, String>> = serde_json::from_str(&result_false_js.as_string().unwrap()).unwrap();
        assert_eq!(results_false.len(), 1);
        assert_eq!(results_false[0].as_ref().unwrap(), "Agent 3 (Generic): Element 'css:#integ-nonexistent-direct' exists: false");
        
        dom_utils::cleanup_element(el);
    }

    #[wasm_bindgen_test]
    async fn test_automate_wait_for_element_direct_command() {
        let agent = setup_agent();
        let (_window, document) = dom_utils::get_window_document().unwrap();
        let el = dom_utils::setup_element(&document, "integ-wait-direct", "div", None);

        let tasks_success_json = serde_json::to_string(&vec!["WAIT_FOR_ELEMENT css:#integ-wait-direct 100"]).unwrap();
        let result_success_js = agent.automate(tasks_success_json).await.unwrap();
        let results_success: Vec<Result<String, String>> = serde_json::from_str(&result_success_js.as_string().unwrap()).unwrap();
        assert_eq!(results_success.len(), 1);
        assert_eq!(results_success[0].as_ref().unwrap(), "Agent 3 (Generic): Element 'css:#integ-wait-direct' appeared.");
        
        dom_utils::cleanup_element(el);

        let tasks_timeout_json = serde_json::to_string(&vec!["WAIT_FOR_ELEMENT css:#integ-wait-timeout-direct 100"]).unwrap();
        let result_timeout_js = agent.automate(tasks_timeout_json).await.unwrap();
        let results_timeout: Vec<Result<String, String>> = serde_json::from_str(&result_timeout_js.as_string().unwrap()).unwrap();
        assert_eq!(results_timeout.len(), 1);
        assert!(results_timeout[0].is_err());
        assert!(results_timeout[0].as_ref().err().unwrap().contains("Agent 3 (Generic): Element 'css:#integ-wait-timeout-direct' not found after 100ms timeout"));
    }

    // LLM-Driven Tests for new commands
    #[wasm_bindgen_test]
    async fn test_automate_llm_get_url() {
        let agent = setup_agent();
        let tasks_json = serde_json::to_string(&vec!["What is the current page URL?"]).unwrap(); // Mock: [{"action": "GET_URL"}]
        let result_js = agent.automate(tasks_json).await.unwrap();
        let results: Vec<Result<String, String>> = serde_json::from_str(&result_js.as_string().unwrap()).unwrap();
        assert_eq!(results.len(), 1); // LLM response is array, automate executes each
        let inner_result_str = results[0].as_ref().unwrap(); 
        let inner_results: Vec<Result<String, String>> = serde_json::from_str(inner_result_str).unwrap();
        assert_eq!(inner_results.len(), 1);
        assert!(inner_results[0].is_ok());
        assert!(inner_results[0].as_ref().unwrap().contains("Current URL is:"));
    }

    #[wasm_bindgen_test]
    async fn test_automate_llm_element_exists() {
        let agent = setup_agent();
        let (_window, document) = dom_utils::get_window_document().unwrap();
        let el = dom_utils::setup_element(&document, "llm-exists", "div", None); // Matches mock selector

        let tasks_json = serde_json::to_string(&vec!["Is the button #llm-exists present?"]).unwrap(); // Mock: [{"action": "ELEMENT_EXISTS", "selector": "css:#llm-exists"}]
        let result_js = agent.automate(tasks_json).await.unwrap();
        let results: Vec<Result<String, String>> = serde_json::from_str(&result_js.as_string().unwrap()).unwrap();
        let inner_results: Vec<Result<String, String>> = serde_json::from_str(results[0].as_ref().unwrap()).unwrap();
        assert_eq!(inner_results.len(), 1);
        assert_eq!(inner_results[0].as_ref().unwrap(), "Element 'css:#llm-exists' exists: true");
        
        dom_utils::cleanup_element(el);
    }

    #[wasm_bindgen_test]
    async fn test_automate_llm_wait_for_element() {
        let agent = setup_agent();
        let (_window, document) = dom_utils::get_window_document().unwrap();
        let el = dom_utils::setup_element(&document, "llm-wait-immediate", "div", None); // Matches mock selector

        let tasks_json = serde_json::to_string(&vec!["Wait for #llm-wait-immediate for 100ms"]).unwrap(); // Mock: [{"action": "WAIT_FOR_ELEMENT", "selector": "css:#llm-wait-immediate", "value": "100"}]
        let result_js = agent.automate(tasks_json).await.unwrap();
        let results: Vec<Result<String, String>> = serde_json::from_str(&result_js.as_string().unwrap()).unwrap();
        let inner_results: Vec<Result<String, String>> = serde_json::from_str(results[0].as_ref().unwrap()).unwrap();
        assert_eq!(inner_results.len(), 1);
        assert_eq!(inner_results[0].as_ref().unwrap(), "Element 'css:#llm-wait-immediate' appeared.");

        dom_utils::cleanup_element(el);
    }
}