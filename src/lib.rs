use wasm_bindgen::prelude::*;
use crate::agent::{AgentSystem, AgentError}; // Import AgentError
use crate::dom_utils::DomError; // Import DomError for From<AgentError>
use web_sys; // Ensure web_sys is imported for console logging
#[cfg(debug_assertions)]
use console_error_panic_hook; // For better panic messages
use serde::{Serialize, Deserialize}; // For LibError

mod agent;
mod llm;
mod dom_utils; // Declare dom_utils module

// Define LibError for serialization
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "error_type")] // This will add an "error_type" field to the JSON
pub enum LibError {
    DomOperation { kind: String, details: String },
    LlmCall { message: String },
    InvalidLlmResponse { message: String },
    CommandParse { message: String },
    Serialization { message: String },
    InternalAgent { message: String }, // Fallback for other AgentErrors
}

impl From<AgentError> for LibError {
    fn from(agent_error: AgentError) -> Self {
        match agent_error {
            AgentError::DomOperationFailed(dom_error) => {
                let kind = match dom_error {
                    DomError::ElementNotFound { .. } => "ElementNotFound".to_string(),
                    DomError::InvalidSelector { .. } => "InvalidSelector".to_string(),
                    DomError::ElementTypeError { .. } => "ElementTypeError".to_string(),
                    DomError::AttributeNotFound { .. } => "AttributeNotFound".to_string(),
                    DomError::SerializationError { .. } => "DomSerializationError".to_string(), // Distinguish from LibError::Serialization
                    DomError::JsError { .. } => "JsError".to_string(),
                    DomError::JsTypeError { .. } => "JsTypeError".to_string(),
                    DomError::JsSyntaxError { .. } => "JsSyntaxError".to_string(),
                    DomError::JsReferenceError { .. } => "JsReferenceError".to_string(),
                };
                LibError::DomOperation {
                    kind,
                    details: dom_error.to_string(),
                }
            }
            AgentError::LlmCallFailed(message) => LibError::LlmCall { message },
            AgentError::InvalidLlmResponse(message) => LibError::InvalidLlmResponse { message },
            AgentError::CommandParseError(message) => LibError::CommandParse { message },
            AgentError::SerializationError(message) => LibError::Serialization { message },
            // If AgentError grows more variants, they can be mapped here or fall into a generic category.
            // For now, let's assume any other AgentError is an InternalAgent error.
            // To make this more robust, one might want to ensure all AgentError variants are explicitly handled.
            // However, given the current AgentError definition, this mapping is exhaustive.
        }
    }
}


// Expose RustAgent to JavaScript
/// `RustAgent` is the main entry point for JavaScript to interact with the Rust-based agent system.
/// It encapsulates an `AgentSystem` and handles configuration for LLM (Large Language Model) interactions.
#[wasm_bindgen]
pub struct RustAgent {
    /// The core agent system that manages and runs agents.
    agents: AgentSystem,
    /// Optional URL for the LLM API endpoint.
    api_url: Option<String>,
    /// Optional name of the LLM model to be used.
    model_name: Option<String>,
    /// Optional API key for authenticating with the LLM service.
    api_key: Option<String>,
}

#[wasm_bindgen]
impl RustAgent {
    /// Creates a new instance of `RustAgent`.
    /// Initializes the underlying `AgentSystem` with a default set of agents.
    /// LLM configuration is initially unset.
    #[wasm_bindgen(constructor)]
    pub fn new() -> RustAgent {
        RustAgent {
            agents: AgentSystem::new(),
            api_url: None,
            model_name: None,
            api_key: None,
        }
    }

    /// Sets the configuration for the Large Language Model (LLM) to be used by the agents.
    /// All parameters are required to enable LLM-based task processing.
    ///
    /// # Arguments
    /// * `api_url`: The URL of the LLM API endpoint.
    /// * `model_name`: The specific model name to use (e.g., "gpt-3.5-turbo").
    /// * `api_key`: The API key for authentication with the LLM service.
    #[wasm_bindgen]
    pub fn set_llm_config(&mut self, api_url: String, model_name: String, api_key: String) {
        self.api_url = Some(api_url);
        self.model_name = Some(model_name);
        self.api_key = Some(api_key);
    }

    /// Automates a list of tasks provided as a JSON string.
    ///
    /// Each task in the list is processed sequentially. If a task string contains the
    /// placeholder `{{PREVIOUS_RESULT}}`, it will be substituted with the successful
    /// output of the immediately preceding task. If the preceding task failed,
    /// `{{PREVIOUS_RESULT}}` is replaced with an empty string.
    ///
    /// # Arguments
    /// * `tasks_json`: A JSON string representing a list of tasks.
    ///   Example: `["CLICK css:#button", "READ css:#label {{PREVIOUS_RESULT}}"]`
    ///
    /// # Returns
    /// A `Result` which, if successful (`Ok`), contains a `JsValue` that is a JSON string
    /// representing a `Vec<Result<String, LibError>>`. Each item in this vector corresponds
    /// to the outcome of a task in the input list:
    ///   - `Ok(String)`: Contains the success message or result string from the task.
    ///     If the task involved LLM-returned commands, this string itself might be a
    ///     JSON representation of `Vec<Result<String, LibError>>` for those sub-commands (though currently it's Vec<Result<String,String>> for inner commands).
    ///   - `Err(LibError)`: Contains the structured error if the task failed.
    ///
    /// If initial checks fail (e.g., LLM config not set, invalid `tasks_json`),
    /// it returns `Err(JsValue)` with an error message (this error is a simple string, not LibError).
    #[wasm_bindgen]
    pub async fn automate(&self, tasks_json: String) -> Result<JsValue, JsValue> {
        // 1. LLM Configuration Check: Ensure API key, URL, and model name are set.
        let (api_key, api_url, model_name) = match (&self.api_key, &self.api_url, &self.model_name) {
            (Some(k), Some(u), Some(m)) => (k, u, m),
            _ => return Err(JsValue::from_str("LLM configuration not set. Please call set_llm_config first.")),
        };

        // 2. Parse tasks_json: Deserialize the input JSON string into a vector of task strings.
        let tasks: Vec<String> = match serde_json::from_str(&tasks_json) {
            Ok(parsed_tasks) => parsed_tasks,
            Err(_) => return Err(JsValue::from_str("Invalid JSON task list. Expected an array of strings.")),
        };

        if tasks.is_empty() {
            return Err(JsValue::from_str("Task list is empty."));
        }

        // 3. Iterate through tasks and execute
        let mut results_list: Vec<Result<String, LibError>> = Vec::new();
        // Stores the successful output of the previous task for placeholder substitution.
        let mut previous_task_successful_output: Option<String> = None;

        for original_task_template in tasks {
            web_sys::console::log_1(&format!("Original task template: {}", original_task_template).into());

            let current_task_string: String;
            // Substitute {{PREVIOUS_RESULT}} placeholder if present.
            if original_task_template.contains("{{PREVIOUS_RESULT}}") {
                let replacement_value = previous_task_successful_output.as_deref().unwrap_or("");
                web_sys::console::log_1(&format!("Placeholder {{PREVIOUS_RESULT}} found. Replacing with: '{}'", replacement_value).into());
                current_task_string = original_task_template.replace("{{PREVIOUS_RESULT}}", replacement_value);
            } else {
                current_task_string = original_task_template.clone();
            }
            
            web_sys::console::log_1(&format!("Executing task (after substitution): {}", current_task_string).into());

            // Run the task using the agent system.
            match self.agents.run_task(&current_task_string, api_key, api_url, model_name).await {
                Ok(result_string) => {
                    // On success, store the output for potential use in the next task
                    // and add it to the list of results for this task sequence.
                    web_sys::console::log_1(&format!("Task succeeded. Storing for {{PREVIOUS_RESULT}}: {}", result_string).into());
                    previous_task_successful_output = Some(result_string.clone());
                    results_list.push(Ok(result_string));
                }
                Err(agent_error) => {
                    // On failure, clear the stored output
                    web_sys::console::log_1(&format!("Task failed. Clearing {{PREVIOUS_RESULT}}. Error: {}", agent_error).into());
                    previous_task_successful_output = None;
                    results_list.push(Err(LibError::from(agent_error))); // Convert AgentError to LibError
                    // Optional: Stop execution on first error
                    // For example: return Err(JsValue::from_str(&format!("Task failed: {}", LibError::from(agent_error))));
                }
            }
        }

        // 4. Serialize results_list and return: Convert the collected results into a JSON string.
        match serde_json::to_string(&results_list) {
            Ok(json_results) => Ok(JsValue::from_str(&json_results)),
            Err(e) => {
                // This serialization error should ideally be a LibError too, but JsValue is the function signature for this top-level error
                let lib_err = LibError::Serialization { message: format!("Failed to serialize final results list: {}", e) };
                let err_json = serde_json::to_string(&lib_err).unwrap_or_else(|_| "{\"error_type\":\"Serialization\",\"message\":\"Failed to serialize error object after failing to serialize results list.\"}".to_string());
                Err(JsValue::from_str(&err_json))
            }
        }
    }
}

// Note: Serialize, Deserialize were already imported for LibError

/// WASM entry point function, typically called once when the WASM module is initialized.
/// This function sets up a panic hook for better debugging in browser console (in debug builds)
/// and logs a message to the console indicating the module has been initialized.
#[wasm_bindgen(start)]
pub fn run() -> Result<(), JsValue> {
    // When the `console_error_panic_hook` feature is enabled, this will print panic messages to the console.
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"RustAgent WASM module initialized!".into());
    Ok(())
}

#[cfg(test)]
#[cfg(feature = "mock-llm")] // Ensure mock-llm is active for these tests
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    use serde_json::Value;

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
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_str).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
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
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_str).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert_eq!(results[0].as_ref().unwrap(), "Agent 3 (Generic) completed task via LLM: Text from #element");

        assert!(results[1].is_ok()); // The outer result for the task is Ok, because it contains a JSON string of command results
        let task2_result_str = results[1].as_ref().unwrap();
        // The inner results are still Vec<Result<String, String>> as per current agent.rs
        let task2_inner_results: Vec<Result<String, String>> = serde_json::from_str(task2_result_str).unwrap();
        assert_eq!(task2_inner_results.len(), 1);
        assert!(task2_inner_results[0].is_err());
        let inner_err_msg = task2_inner_results[0].as_ref().err().unwrap();
        // The error message from agent.rs includes the DOM error string directly
        assert!(inner_err_msg.contains("DOM Operation Failed: ElementNotFound: No element found for selector 'css:#input'"));
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
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_str).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].is_err());
        match results[0].as_ref().err().unwrap() {
            LibError::DomOperation { kind, details } => {
                assert_eq!(kind, "ElementNotFound");
                assert!(details.contains("No element found for selector '#nonexistent_button'"));
            }
            _ => panic!("Incorrect error type for task 1"),
        }

        assert!(results[1].is_ok());
        let task2_result_str = results[1].as_ref().unwrap();
        let task2_inner_results: Vec<Result<String, String>> = serde_json::from_str(task2_result_str).unwrap();
        assert_eq!(task2_inner_results.len(), 1);
        assert!(task2_inner_results[0].is_err());
        let inner_err_msg_task2 = task2_inner_results[0].as_ref().err().unwrap();
        assert!(inner_err_msg_task2.contains("DOM Operation Failed: ElementNotFound: No element found for selector 'css:#input'"));
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
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_str).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        let task1_result_str = results[0].as_ref().unwrap();
        let task1_inner_results: Vec<Result<String, String>> = serde_json::from_str(task1_result_str).unwrap();
        assert_eq!(task1_inner_results.len(), 1);
        assert!(task1_inner_results[0].is_err());
        assert!(task1_inner_results[0].as_ref().err().unwrap().contains("DOM Operation Failed: ElementNotFound: No element found for selector 'css:#input'"));
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
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_str).unwrap();

        assert_eq!(results.len(), 3);
        assert!(results[0].is_ok());
        assert_eq!(results[0].as_ref().unwrap(), "Agent 3 (Generic) completed task via LLM: Clicked #first_button");
        assert!(results[1].is_ok());
        assert_eq!(results[1].as_ref().unwrap(), "Agent 3 (Generic) completed task via LLM: Processed Clicked #first_button");
        assert!(results[2].is_ok());
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
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_str).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert_eq!(results[0].as_ref().unwrap(), "Agent 3 (Generic) completed task via LLM: element_id_123");

        assert!(results[1].is_ok());
        let task2_result_str = results[1].as_ref().unwrap();
        let task2_inner_results: Vec<Result<String, String>> = serde_json::from_str(task2_result_str).unwrap();
        assert_eq!(task2_inner_results.len(), 2);

        assert!(task2_inner_results[0].is_err());
        assert!(task2_inner_results[0].as_ref().err().unwrap().contains("DOM Operation Failed: ElementNotFound: No element found for selector '#element_id_123'"));

        assert!(task2_inner_results[1].is_err());
        assert!(task2_inner_results[1].as_ref().err().unwrap().contains("DOM Operation Failed: ElementNotFound: No element found for selector '#another_element'"));
    }

    // Integration tests for new commands via automate()
    #[wasm_bindgen_test]
    async fn test_automate_get_url_direct_command() {
        let agent = setup_agent();
        let tasks_json = serde_json::to_string(&vec!["GET_URL"]).unwrap();
        let result_js = agent.automate(tasks_json).await.unwrap();
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_js.as_string().unwrap()).unwrap();
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
        let results_true: Vec<Result<String, LibError>> = serde_json::from_str(&result_true_js.as_string().unwrap()).unwrap();
        assert_eq!(results_true.len(), 1);
        assert!(results_true[0].is_ok());
        assert_eq!(results_true[0].as_ref().unwrap(), "Agent 3 (Generic): Element 'css:#integ-exists-direct' exists: true");

        let tasks_false_json = serde_json::to_string(&vec!["ELEMENT_EXISTS css:#integ-nonexistent-direct"]).unwrap();
        let result_false_js = agent.automate(tasks_false_json).await.unwrap();
        let results_false: Vec<Result<String, LibError>> = serde_json::from_str(&result_false_js.as_string().unwrap()).unwrap();
        assert_eq!(results_false.len(), 1);
        assert!(results_false[0].is_ok());
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
        let results_success: Vec<Result<String, LibError>> = serde_json::from_str(&result_success_js.as_string().unwrap()).unwrap();
        assert_eq!(results_success.len(), 1);
        assert!(results_success[0].is_ok());
        assert_eq!(results_success[0].as_ref().unwrap(), "Agent 3 (Generic): Element 'css:#integ-wait-direct' appeared.");

        dom_utils::cleanup_element(el);

        let tasks_timeout_json = serde_json::to_string(&vec!["WAIT_FOR_ELEMENT css:#integ-wait-timeout-direct 100"]).unwrap();
        let result_timeout_js = agent.automate(tasks_timeout_json).await.unwrap();
        let results_timeout: Vec<Result<String, LibError>> = serde_json::from_str(&result_timeout_js.as_string().unwrap()).unwrap();
        assert_eq!(results_timeout.len(), 1);
        assert!(results_timeout[0].is_err());
        match results_timeout[0].as_ref().err().unwrap() {
            LibError::DomOperation { kind, details } => {
                assert_eq!(kind, "ElementNotFound");
                assert!(details.contains("Element 'css:#integ-wait-timeout-direct' not found after 100ms timeout"));
            }
            _ => panic!("Incorrect error type for WAIT_FOR_ELEMENT timeout"),
        }
    }

    // LLM-Driven Tests for new commands
    #[wasm_bindgen_test]
    async fn test_automate_llm_get_url() {
        let agent = setup_agent();
        let tasks_json = serde_json::to_string(&vec!["What is the current page URL?"]).unwrap(); // Mock: [{"action": "GET_URL"}]
        let result_js = agent.automate(tasks_json).await.unwrap();
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_js.as_string().unwrap()).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        let inner_result_str = results[0].as_ref().unwrap();
        let inner_results: Vec<Result<String, String>> = serde_json::from_str(inner_result_str).unwrap(); // Inner still String errors
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
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_js.as_string().unwrap()).unwrap();
        assert!(results[0].is_ok());
        let inner_results: Vec<Result<String, String>> = serde_json::from_str(results[0].as_ref().unwrap()).unwrap();
        assert_eq!(inner_results.len(), 1);
        assert!(inner_results[0].is_ok());
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
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_js.as_string().unwrap()).unwrap();
        assert!(results[0].is_ok());
        let inner_results: Vec<Result<String, String>> = serde_json::from_str(results[0].as_ref().unwrap()).unwrap();
        assert_eq!(inner_results.len(), 1);
        assert!(inner_results[0].is_ok());
        assert_eq!(inner_results[0].as_ref().unwrap(), "Element 'css:#llm-wait-immediate' appeared.");

        dom_utils::cleanup_element(el);
    }

    // Integration tests for IS_VISIBLE
    #[wasm_bindgen_test]
    async fn test_automate_is_visible_direct_command_true() {
        let agent = setup_agent();
        let (_window, document) = dom_utils::get_window_document().unwrap();
        let el = dom_utils::setup_element(&document, "integ-visible-true", "div", Some(vec![("style", "width:10px; height:10px;")]));

        let tasks_json = serde_json::to_string(&vec!["IS_VISIBLE css:#integ-visible-true"]).unwrap();
        let result_js = agent.automate(tasks_json).await.unwrap();
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_js.as_string().unwrap()).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        assert_eq!(results[0].as_ref().unwrap(), "Agent 3 (Generic): Element 'css:#integ-visible-true' is visible: true");

        dom_utils::cleanup_element(el);
    }

    #[wasm_bindgen_test]
    async fn test_automate_is_visible_direct_command_false() {
        let agent = setup_agent();
        let (_window, document) = dom_utils::get_window_document().unwrap();
        let el = dom_utils::setup_element(&document, "integ-visible-false", "div", Some(vec![("style", "display:none;")]));

        let tasks_json = serde_json::to_string(&vec!["IS_VISIBLE css:#integ-visible-false"]).unwrap();
        let result_js = agent.automate(tasks_json).await.unwrap();
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_js.as_string().unwrap()).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        assert_eq!(results[0].as_ref().unwrap(), "Agent 3 (Generic): Element 'css:#integ-visible-false' is visible: false");

        dom_utils::cleanup_element(el);
    }

    #[wasm_bindgen_test]
    async fn test_automate_llm_is_visible() {
        let agent = setup_agent();
        let (_window, document) = dom_utils::get_window_document().unwrap();
        // Mock "Is the #mainContent visible?" -> [{"action": "IS_VISIBLE", "selector": "css:#mainContent"}]
        let el = dom_utils::setup_element(&document, "mainContent", "div", Some(vec![("style", "width:10px; height:10px;")]));

        let tasks_json = serde_json::to_string(&vec!["Is the #mainContent visible?"]).unwrap();
        let result_js = agent.automate(tasks_json).await.unwrap();
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_js.as_string().unwrap()).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        let inner_result_str = results[0].as_ref().unwrap();
        let inner_results: Vec<Result<String, String>> = serde_json::from_str(inner_result_str).unwrap();
        assert_eq!(inner_results.len(), 1);
        assert!(inner_results[0].is_ok());
        assert_eq!(inner_results[0].as_ref().unwrap(), "Element 'css:#mainContent' is visible: true");

        dom_utils::cleanup_element(el);
    }

    // Integration tests for SCROLL_TO
    #[wasm_bindgen_test]
    async fn test_automate_scroll_to_direct_command() {
        let agent = setup_agent();
        let (_window, document) = dom_utils::get_window_document().unwrap();
        document.body().unwrap().set_attribute("style", "height: 2000px;").unwrap();
        let el = dom_utils::setup_element(&document, "integ-scroll-direct", "div", Some(vec![("style", "margin-top: 1800px; height: 50px;")]));

        let tasks_json = serde_json::to_string(&vec!["SCROLL_TO css:#integ-scroll-direct"]).unwrap();
        let result_js = agent.automate(tasks_json).await.unwrap();
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_js.as_string().unwrap()).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        assert_eq!(results[0].as_ref().unwrap(), "Agent 3 (Generic): Successfully scrolled to element 'css:#integ-scroll-direct'");

        let final_scroll_y = web_sys::window().unwrap().scroll_y().unwrap_or(0.0);
        assert!(final_scroll_y > 1500.0, "Final scroll Y ({}) should be significantly greater after scroll_to", final_scroll_y);

        dom_utils::cleanup_element(el);
        document.body().unwrap().remove_attribute("style").unwrap();
        web_sys::window().unwrap().scroll_to_with_x_and_y(0.0, 0.0);
    }

    #[wasm_bindgen_test]
    async fn test_automate_llm_scroll_to() {
        let agent = setup_agent();
        let (_window, document) = dom_utils::get_window_document().unwrap();
        document.body().unwrap().set_attribute("style", "height: 2000px;").unwrap();
        // Mock "Scroll to the footer" -> [{"action": "SCROLL_TO", "selector": "css:footer"}]
        let el = dom_utils::setup_element(&document, "footer", "footer", Some(vec![("style", "margin-top: 1800px; height: 50px;")]));

        let tasks_json = serde_json::to_string(&vec!["Scroll to the footer"]).unwrap();
        let result_js = agent.automate(tasks_json).await.unwrap();
        let results: Vec<Result<String, LibError>> = serde_json::from_str(&result_js.as_string().unwrap()).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        let inner_result_str = results[0].as_ref().unwrap();
        let inner_results: Vec<Result<String, String>> = serde_json::from_str(inner_result_str).unwrap();
        assert_eq!(inner_results.len(), 1);
        assert!(inner_results[0].is_ok());
        assert_eq!(inner_results[0].as_ref().unwrap(), "Successfully scrolled to element 'css:footer'");

        let final_scroll_y = web_sys::window().unwrap().scroll_y().unwrap_or(0.0);
        assert!(final_scroll_y > 1500.0, "Final scroll Y ({}) should be significantly greater after scroll_to", final_scroll_y);

        dom_utils::cleanup_element(el);
        document.body().unwrap().remove_attribute("style").unwrap();
        web_sys::window().unwrap().scroll_to_with_x_and_y(0.0, 0.0);
    }

    #[wasm_bindgen_test]
    async fn test_automate_llm_mixed_validity_commands() {
        let agent = setup_agent();
        let tasks_json = serde_json::to_string(&vec!["task with mixed valid and malformed json commands"]).unwrap();
        // Mock response for this task in llm.rs:
        // "[{\"action\": \"CLICK\", \"selector\": \"css:#valid\"}, {\"invalid_field\": \"some_value\", \"action\": \"EXTRA_INVALID_FIELD\"}, {\"action\": \"TYPE\", \"selector\": \"css:#anotherValid\", \"value\": \"test\"}]"

        let result_js = agent.automate(tasks_json).await.unwrap();
        let result_str_outer = result_js.as_string().unwrap();

        let results_outer: Vec<Result<String, LibError>> = serde_json::from_str(&result_str_outer).unwrap();
        assert_eq!(results_outer.len(), 1, "Expected one top-level task result");
        assert!(results_outer[0].is_ok(), "Expected the LLM command processing itself to be Ok");

        let inner_json_results_str = results_outer[0].as_ref().unwrap();
        // Inner results are still Vec<Result<String, String>> from agent.rs
        let inner_results: Vec<Result<String, String>> = serde_json::from_str(inner_json_results_str).unwrap();
        assert_eq!(inner_results.len(), 3, "Expected three inner command results");

        assert!(inner_results[0].is_err());
        assert!(inner_results[0].as_ref().err().unwrap().contains("DOM Operation Failed: ElementNotFound: No element found for selector 'css:#valid'"));

        assert!(inner_results[1].is_err());
        let err_msg_malformed = inner_results[1].as_ref().err().unwrap();
        assert!(err_msg_malformed.contains("Command at index 1 was malformed and could not be parsed:"));
        assert!(err_msg_malformed.contains("{\"invalid_field\":\"some_value\",\"action\":\"EXTRA_INVALID_FIELD\"}"));

        assert!(inner_results[2].is_err());
        assert!(inner_results[2].as_ref().err().unwrap().contains("DOM Operation Failed: ElementNotFound: No element found for selector 'css:#anotherValid'"));
    }

    // Helper to setup a simple element for testing directly in lib.rs tests
    // This avoids needing dom_utils::setup_element if it's not exposed or convenient
    fn setup_html_element_for_lib_test(id: &str, tag: &str, text_content: Option<&str>) -> web_sys::Element {
        let window = web_sys::window().expect("no global `window` exists");
        let document = window.document().expect("should have a document on window");
        let element = document.create_element(tag).unwrap();
        element.set_id(id);
        if let Some(text) = text_content {
            element.set_text_content(Some(text));
        }
        document.body().unwrap().append_child(&element).unwrap();
        element
    }

    fn cleanup_html_element_for_lib_test(element: web_sys::Element) {
        element.remove();
    }

    #[wasm_bindgen_test]
    async fn test_automate_hover_command() {
        let agent = setup_agent();
        let element_id = "hoverTestElementLib";
        let _el = setup_html_element_for_lib_test(element_id, "div", None);

        // Test HOVER on existing element
        let tasks_hover_exists_json = serde_json::to_string(&vec![format!("HOVER css:#{}", element_id)]).unwrap();
        let result_hover_exists_js = agent.automate(tasks_hover_exists_json).await.unwrap();
        let results_hover_exists: Vec<Result<String, LibError>> = serde_json::from_str(&result_hover_exists_js.as_string().unwrap()).unwrap();

        assert_eq!(results_hover_exists.len(), 1);
        assert!(results_hover_exists[0].is_ok(), "HOVER command failed for existing element: {:?}", results_hover_exists[0].as_ref().err());
        assert!(results_hover_exists[0].as_ref().unwrap().contains(&format!("Successfully hovered over element 'css:#{}'", element_id)));

        cleanup_html_element_for_lib_test(_el);

        // Test HOVER on non-existent element
        let tasks_hover_nonexistent_json = serde_json::to_string(&vec!["HOVER css:#nonExistentHoverLib"]).unwrap();
        let result_hover_nonexistent_js = agent.automate(tasks_hover_nonexistent_json).await.unwrap();
        let results_hover_nonexistent: Vec<Result<String, LibError>> = serde_json::from_str(&result_hover_nonexistent_js.as_string().unwrap()).unwrap();

        assert_eq!(results_hover_nonexistent.len(), 1);
        assert!(results_hover_nonexistent[0].is_err());
        match results_hover_nonexistent[0].as_ref().err().unwrap() {
            LibError::DomOperation { kind, details } => {
                assert_eq!(kind, "ElementNotFound");
                assert!(details.contains("No element found for selector 'css:#nonExistentHoverLib'"));
            }
            _ => panic!("Incorrect error type for HOVER on non-existent element"),
        }
    }

    #[wasm_bindgen_test]
    async fn test_automate_get_all_text_command() {
        let agent = setup_agent();
        let parent_id = "getAllTextParentLib";
        let item_class = "myTestItemsLib";

        let parent_el = setup_html_element_for_lib_test(parent_id, "div", None);
        let item1 = setup_html_element_for_lib_test("item1Lib", "p", Some("Text 1"));
        item1.set_class_name(item_class);
        parent_el.append_child(&item1).unwrap();

        let item2 = setup_html_element_for_lib_test("item2Lib", "p", Some("More Text 2"));
        item2.set_class_name(item_class);
        parent_el.append_child(&item2).unwrap();

        let item3_empty = setup_html_element_for_lib_test("item3LibEmpty", "p", Some("")); // Empty text
        item3_empty.set_class_name(item_class);
        parent_el.append_child(&item3_empty).unwrap();

        // Test with default separator (newline)
        let tasks_default_sep_json = serde_json::to_string(&vec![format!("GET_ALL_TEXT css:#{} .{}", parent_id, item_class)]).unwrap();
        let result_default_sep_js = agent.automate(tasks_default_sep_json).await.unwrap();
        let results_default_sep: Vec<Result<String, LibError>> = serde_json::from_str(&result_default_sep_js.as_string().unwrap()).unwrap();
        assert_eq!(results_default_sep.len(), 1);
        assert!(results_default_sep[0].is_ok(), "GET_ALL_TEXT (default sep) failed: {:?}", results_default_sep[0].as_ref().err());
        assert!(results_default_sep[0].as_ref().unwrap().contains("Retrieved text from elements matching 'css:#getAllTextParentLib .myTestItemsLib' (separated by '\\n'): \"Text 1\nMore Text 2\""), "Actual: {}", results_default_sep[0].as_ref().unwrap());


        // Test with custom separator "---"
        let tasks_custom_sep_json = serde_json::to_string(&vec![format!("GET_ALL_TEXT css:#{} .{} \"---\"", parent_id, item_class)]).unwrap();
        let result_custom_sep_js = agent.automate(tasks_custom_sep_json).await.unwrap();
        let results_custom_sep: Vec<Result<String, LibError>> = serde_json::from_str(&result_custom_sep_js.as_string().unwrap()).unwrap();
        assert_eq!(results_custom_sep.len(), 1);
        assert!(results_custom_sep[0].is_ok(), "GET_ALL_TEXT (custom sep) failed: {:?}", results_custom_sep[0].as_ref().err());
        assert!(results_custom_sep[0].as_ref().unwrap().contains("Retrieved text from elements matching 'css:#getAllTextParentLib .myTestItemsLib' (separated by '---'): \"Text 1---More Text 2\""));

        // Test with custom separator including spaces (quoted)
        let tasks_quoted_sep_json = serde_json::to_string(&vec![format!("GET_ALL_TEXT css:#{} .{} \" | \"", parent_id, item_class)]).unwrap();
        let result_quoted_sep_js = agent.automate(tasks_quoted_sep_json).await.unwrap();
        let results_quoted_sep: Vec<Result<String, LibError>> = serde_json::from_str(&result_quoted_sep_js.as_string().unwrap()).unwrap();
        assert_eq!(results_quoted_sep.len(), 1);
        assert!(results_quoted_sep[0].is_ok(), "GET_ALL_TEXT (quoted sep) failed: {:?}", results_quoted_sep[0].as_ref().err());
        assert!(results_quoted_sep[0].as_ref().unwrap().contains("Retrieved text from elements matching 'css:#getAllTextParentLib .myTestItemsLib' (separated by ' | '): \"Text 1 | More Text 2\""));


        cleanup_html_element_for_lib_test(parent_el); // item1, item2, item3_empty are children

        // Test no elements found
        let tasks_no_elements_json = serde_json::to_string(&vec!["GET_ALL_TEXT css:.nonExistentItemsLib"]).unwrap();
        let result_no_elements_js = agent.automate(tasks_no_elements_json).await.unwrap();
        let results_no_elements: Vec<Result<String, LibError>> = serde_json::from_str(&result_no_elements_js.as_string().unwrap()).unwrap();
        assert_eq!(results_no_elements.len(), 1);
        assert!(results_no_elements[0].is_ok());
        assert!(results_no_elements[0].as_ref().unwrap().contains("Retrieved text from elements matching 'css:.nonExistentItemsLib' (separated by '\\n'): \"\""));

        // Test elements found but no text content (setup new elements for this)
        let parent_no_text_id = "noTextParentLib";
        let parent_no_text_el = setup_html_element_for_lib_test(parent_no_text_id, "div", None);
        let item_no_text1 = setup_html_element_for_lib_test("itemNoText1Lib", "p", Some(""));
        item_no_text1.set_class_name("noTestItemsLib");
        parent_no_text_el.append_child(&item_no_text1).unwrap();
        let item_no_text2 = setup_html_element_for_lib_test("itemNoText2Lib", "p", None); // No text content at all
        item_no_text2.set_class_name("noTestItemsLib");
        parent_no_text_el.append_child(&item_no_text2).unwrap();

        let tasks_no_text_json = serde_json::to_string(&vec![format!("GET_ALL_TEXT css:#{} .noTestItemsLib", parent_no_text_id)]).unwrap();
        let result_no_text_js = agent.automate(tasks_no_text_json).await.unwrap();
        let results_no_text: Vec<Result<String, LibError>> = serde_json::from_str(&result_no_text_js.as_string().unwrap()).unwrap();
        assert_eq!(results_no_text.len(), 1);
        assert!(results_no_text[0].is_ok());
        assert!(results_no_text[0].as_ref().unwrap().contains(&format!("Retrieved text from elements matching 'css:#{} .noTestItemsLib' (separated by '\\n'): \"\"", parent_no_text_id)));

        cleanup_html_element_for_lib_test(parent_no_text_el);


        // Test invalid selector
        let tasks_invalid_selector_json = serde_json::to_string(&vec!["GET_ALL_TEXT css:[[["]).unwrap();
        let result_invalid_selector_js = agent.automate(tasks_invalid_selector_json).await.unwrap();
        let results_invalid_selector: Vec<Result<String, LibError>> = serde_json::from_str(&result_invalid_selector_js.as_string().unwrap()).unwrap();
        assert_eq!(results_invalid_selector.len(), 1);
        assert!(results_invalid_selector[0].is_err());
        match results_invalid_selector[0].as_ref().err().unwrap() {
            LibError::DomOperation { kind, details } => {
                assert_eq!(kind, "InvalidSelector");
                assert!(details.contains("Invalid selector 'css:[[['"));
            }
            _ => panic!("Incorrect error type for GET_ALL_TEXT with invalid selector"),
        }
    }
}