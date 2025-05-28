use crate::llm::call_llm_async; // Changed from call_llm
use crate::dom_utils::{self, *}; // Import DOM utility functions, ensure dom_utils is accessible
use web_sys::console; // For logging unexpected parsing issues
use serde::Deserialize; // For JSON deserialization

// 1. Define AgentRole Enum
#[derive(Debug, Clone, PartialEq)]
pub enum AgentRole {
    Navigator,
    FormFiller,
    Generic,
}

// 2. Update Agent Struct
pub struct Agent {
    id: u32,
    role: AgentRole, // Changed from String to AgentRole
}

// Define DomCommandAction enum and DomCommand struct
#[derive(Debug, Clone, PartialEq, Deserialize)] // Added Deserialize
#[serde(rename_all = "UPPERCASE")] // To match action strings like "CLICK"
enum DomCommandAction {
    Click,
    Type,
    Read,
    GetValue,
    GetAttribute,
    SetAttribute,
    SelectOption,
    GetAllAttributes, // Added new action
}

#[derive(Debug, Clone)] // DomCommand remains for internal use
struct DomCommand {
    action: DomCommandAction,
    selector: String,
    value: Option<String>, // For TYPE, SETATTRIBUTE, SELECTOPTION
    attribute_name: Option<String>, // For GETATTRIBUTE, SETATTRIBUTE
}

// Struct for deserializing LLM's JSON command
#[derive(Deserialize, Debug)]
struct LlmDomCommandRequest {
    action: String, // Keep as String for flexibility, then map to DomCommandAction
    selector: String,
    value: Option<String>,
    attribute_name: Option<String>,
}

// Implement parse_dom_command function
const AVAILABLE_DOM_COMMANDS: [&str; 8] = [ // This could be Vec<String> or other types too
    "CLICK <selector>",
    "TYPE <selector> <text>",
    "READ <selector>",
    "GETVALUE <selector>",
    "GETATTRIBUTE <selector> <attribute_name>",
    "SETATTRIBUTE <selector> <attribute_name> <value>",
    "SELECTOPTION <selector> <option_value>",
    "GET_ALL_ATTRIBUTES <selector> <attribute_name> (returns a JSON array of attribute values)",
];

// Helper function to generate the structured LLM prompt
fn generate_structured_llm_prompt(
    agent_id: u32,
    agent_role: &AgentRole,
    original_task: &str,
    _available_commands_list: &[&str] // Parameter kept for signature compatibility, but not used directly in new prompt
) -> String {
    // Define the available actions from DomCommandAction enum
    let actions = [
        "CLICK",
        "TYPE",
        "READ",
        "GETVALUE",
        "GETATTRIBUTE",
        "SETATTRIBUTE",
        "SELECTOPTION",
        "GET_ALL_ATTRIBUTES",
    ];
    let action_list_str = actions.join(", ");

    format!(
        "You are Agent {} ({:?}).\n\
        The user wants to perform the following task: \"{}\"\n\n\
        Analyze the task. If it can be broken down into a sequence of specific DOM actions, \
        respond with a JSON array of command objects. Each object must have an \"action\" and a \"selector\". \
        The \"value\" field is required for TYPE, SETATTRIBUTE, and SELECTOPTION actions. \
        The \"attribute_name\" field is required for GETATTRIBUTE and SETATTRIBUTE actions, and for GET_ALL_ATTRIBUTES. \
        Ensure selectors are valid CSS selectors (e.g., \"css:#elementId\", \"css:.className\") or XPath expressions (e.g., \"xpath://div[@id='example']\").\n\n\
        Available actions are: {}.\n\n\
        JSON schema for commands:\n\
        - Click: {{\"action\": \"CLICK\", \"selector\": \"<selector>\"}}\n\
        - Type: {{\"action\": \"TYPE\", \"selector\": \"<selector>\", \"value\": \"<text_to_type>\"}}\n\
        - Read: {{\"action\": \"READ\", \"selector\": \"<selector>\"}} (gets text content)\n\
        - Get Value: {{\"action\": \"GETVALUE\", \"selector\": \"<selector>\"}} (gets value of form elements like input, textarea, select)\n\
        - Get Attribute: {{\"action\": \"GETATTRIBUTE\", \"selector\": \"<selector>\", \"attribute_name\": \"<attr_name>\"}}\n\
        - Set Attribute: {{\"action\": \"SETATTRIBUTE\", \"selector\": \"<selector>\", \"attribute_name\": \"<attr_name>\", \"value\": \"<attr_value>\"}}\n\
        - Select Option: {{\"action\": \"SELECTOPTION\", \"selector\": \"<selector>\", \"value\": \"<option_value>\"}}\n\
        - Get All Attributes: {{\"action\": \"GET_ALL_ATTRIBUTES\", \"selector\": \"<selector>\", \"attribute_name\": \"<attr_name>\"}} (returns a JSON array of attribute values for all matching elements)\n\n\
        Example of a JSON array response:\n\
        [\n\
          {{\"action\": \"TYPE\", \"selector\": \"css:#username\", \"value\": \"testuser\"}},\n\
          {{\"action\": \"CLICK\", \"selector\": \"xpath://button[@type='submit']\"}}\n\
        ]\n\n\
        If the task is a general question, a request for information not obtainable through DOM actions (e.g., current URL, page title if not in DOM, or a summary), \
        or if it cannot be mapped to the defined DOM commands, respond with a natural language text answer. Do not attempt to create new DOM command structures not listed.",
        agent_id, agent_role, original_task, action_list_str
    )
}

fn parse_dom_command(task: &str) -> Option<DomCommand> {
    let parts: Vec<&str> = task.splitn(2, ' ').collect();
    let command_str = parts.get(0).unwrap_or(&"").to_uppercase();
    let args_str = parts.get(1).unwrap_or(&"");

    match command_str.as_str() {
        "CLICK" => {
            if args_str.is_empty() { return None; }
            Some(DomCommand {
                action: DomCommandAction::Click,
                selector: args_str.to_string(),
                value: None,
                attribute_name: None,
            })
        }
        "TYPE" => {
            let sub_parts: Vec<&str> = args_str.splitn(2, ' ').collect();
            let selector = sub_parts.get(0).unwrap_or(&"");
            let text_to_type = sub_parts.get(1).unwrap_or(&"");
            if selector.is_empty() || text_to_type.is_empty() { return None; }
            Some(DomCommand {
                action: DomCommandAction::Type,
                selector: selector.to_string(),
                value: Some(text_to_type.to_string()),
                attribute_name: None,
            })
        }
        "READ" => {
            if args_str.is_empty() { return None; }
            Some(DomCommand {
                action: DomCommandAction::Read,
                selector: args_str.to_string(),
                value: None,
                attribute_name: None,
            })
        }
        "GETVALUE" => {
            if args_str.is_empty() { return None; }
            Some(DomCommand {
                action: DomCommandAction::GetValue,
                selector: args_str.to_string(),
                value: None,
                attribute_name: None,
            })
        }
        "GETATTRIBUTE" => {
            let sub_parts: Vec<&str> = args_str.splitn(2, ' ').collect();
            let selector = sub_parts.get(0).unwrap_or(&"");
            let attribute_name = sub_parts.get(1).unwrap_or(&"");
            if selector.is_empty() || attribute_name.is_empty() { return None; }
            Some(DomCommand {
                action: DomCommandAction::GetAttribute,
                selector: selector.to_string(),
                value: None,
                attribute_name: Some(attribute_name.to_string()),
            })
        }
        "SETATTRIBUTE" => {
            let sub_parts: Vec<&str> = args_str.splitn(3, ' ').collect();
            let selector = sub_parts.get(0).unwrap_or(&"");
            let attribute_name = sub_parts.get(1).unwrap_or(&"");
            let attribute_value = sub_parts.get(2).unwrap_or(&"");
            if selector.is_empty() || attribute_name.is_empty() || attribute_value.is_empty() { return None; }
            Some(DomCommand {
                action: DomCommandAction::SetAttribute,
                selector: selector.to_string(),
                value: Some(attribute_value.to_string()),
                attribute_name: Some(attribute_name.to_string()),
            })
        }
        "SELECTOPTION" => {
            let sub_parts: Vec<&str> = args_str.splitn(2, ' ').collect();
            let selector = sub_parts.get(0).unwrap_or(&"");
            let value = sub_parts.get(1).unwrap_or(&"");
            if selector.is_empty() || value.is_empty() { return None; }
            Some(DomCommand {
                action: DomCommandAction::SelectOption,
                selector: selector.to_string(),
                value: Some(value.to_string()),
                attribute_name: None,
            })
        }
        "GET_ALL_ATTRIBUTES" => { // Renamed from GETALLATTRIBUTES to GET_ALL_ATTRIBUTES for consistency
            let sub_parts: Vec<&str> = args_str.splitn(2, ' ').collect();
            let selector = sub_parts.get(0).unwrap_or(&"");
            let attribute_name = sub_parts.get(1).unwrap_or(&"");
            if selector.is_empty() || attribute_name.is_empty() { return None; }
            Some(DomCommand {
                action: DomCommandAction::GetAllAttributes,
                selector: selector.to_string(),
                value: None, // Not used for this action
                attribute_name: Some(attribute_name.to_string()),
            })
        }
        _ => None,
    }
}

pub struct AgentSystem {
    agents: Vec<Agent>,
}

impl AgentSystem {
    // 3. Update AgentSystem::new()
    pub fn new() -> Self {
        let mut agents = Vec::new();
        agents.push(Agent { id: 1, role: AgentRole::Navigator });
        agents.push(Agent { id: 2, role: AgentRole::FormFiller });
        agents.push(Agent { id: 3, role: AgentRole::Generic }); // Added a Generic agent
        AgentSystem { agents }
    }

    // run_task is now async and returns Result<String, String>
    pub async fn run_task(&self, task: &str, api_key: &str, api_url: &str, model_name: &str) -> Result<String, String> {
        // 1. Agent Selection Logic
        let task_lowercase = task.to_lowercase();
        let mut selected_agent = self.agents.iter().find(|a| a.role == AgentRole::Generic)
            .unwrap_or_else(|| &self.agents[0]); // Default to Generic or first agent

        if task_lowercase.contains("navigate") || task_lowercase.contains("go to") || task_lowercase.contains("url") {
            if let Some(agent) = self.agents.iter().find(|a| a.role == AgentRole::Navigator) {
                selected_agent = agent;
            }
        } else if task_lowercase.contains("fill") || task_lowercase.contains("type") || task_lowercase.contains("input") || task_lowercase.contains("form") {
            // Note: "type" keyword is also in the DOM command, this agent selection is for LLM fallback or general context.
            if let Some(agent) = self.agents.iter().find(|a| a.role == AgentRole::FormFiller) {
                selected_agent = agent;
            }
        }
        // Add more rules here if other roles are introduced

        console::log_1(&format!("Task received: '{}'. Selected Agent ID: {}, Role: {:?}", task, selected_agent.id, selected_agent.role).into());

        // 2. Task Execution with Selected Agent
        if let Some(dom_command) = parse_dom_command(task) {
            // Execute DOM command
            console::log_1(&format!("Agent {} ({:?}): Executing DOM command: {:?}", selected_agent.id, selected_agent.role, dom_command).into());
            match dom_command.action {
                DomCommandAction::Click => {
                    match click_element(&dom_command.selector) {
                        Ok(_) => Ok(format!("Agent {} ({:?}): Successfully clicked element with selector: '{}'", selected_agent.id, selected_agent.role, dom_command.selector)),
                        Err(e) => Err(format!("Agent {} ({:?}): Error clicking element: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                    }
                }
                DomCommandAction::Type => {
                    let text_to_type = dom_command.value.unwrap_or_default(); // Should be Some by parse_dom_command logic
                    match type_in_element(&dom_command.selector, &text_to_type) {
                        Ok(_) => Ok(format!("Agent {} ({:?}): Successfully typed '{}' in element with selector: '{}'", selected_agent.id, selected_agent.role, text_to_type, dom_command.selector)),
                        Err(e) => Err(format!("Agent {} ({:?}): Error typing in element: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                    }
                }
                DomCommandAction::Read => {
                    match get_element_text(&dom_command.selector) {
                        Ok(text) => Ok(format!("Agent {} ({:?}): Text from element '{}': {}", selected_agent.id, selected_agent.role, dom_command.selector, text)),
                        Err(e) => Err(format!("Agent {} ({:?}): Error reading text from element: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                    }
                }
                DomCommandAction::GetValue => {
                    match get_element_value(&dom_command.selector) {
                        Ok(value) => Ok(format!("Agent {} ({:?}): Value from element '{}': {}", selected_agent.id, selected_agent.role, dom_command.selector, value)),
                        Err(e) => Err(format!("Agent {} ({:?}): Error getting value from element: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                    }
                }
                DomCommandAction::GetAttribute => {
                    let attribute_name = dom_command.attribute_name.unwrap_or_default(); // Should be Some
                    match get_element_attribute(&dom_command.selector, &attribute_name) {
                        Ok(value) => Ok(format!("Agent {} ({:?}): Attribute '{}' from element '{}': {}", selected_agent.id, selected_agent.role, attribute_name, dom_command.selector, value)),
                        Err(e) => Err(format!("Agent {} ({:?}): Error getting attribute: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                    }
                }
                DomCommandAction::SetAttribute => {
                    let attribute_name = dom_command.attribute_name.unwrap_or_default(); // Should be Some
                    let attribute_value = dom_command.value.unwrap_or_default(); // Should be Some
                    match set_element_attribute(&dom_command.selector, &attribute_name, &attribute_value) {
                        Ok(_) => Ok(format!("Agent {} ({:?}): Successfully set attribute '{}' to '{}' for element '{}'", selected_agent.id, selected_agent.role, attribute_name, attribute_value, dom_command.selector)),
                        Err(e) => Err(format!("Agent {} ({:?}): Error setting attribute: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                    }
                }
                DomCommandAction::SelectOption => {
                    let value = dom_command.value.unwrap_or_default(); // Should be Some
                    match select_dropdown_option(&dom_command.selector, &value) {
                        Ok(_) => Ok(format!("Agent {} ({:?}): Successfully selected option '{}' for dropdown '{}'", selected_agent.id, selected_agent.role, value, dom_command.selector)),
                        Err(e) => Err(format!("Agent {} ({:?}): Error selecting option: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                    }
                }
                DomCommandAction::GetAllAttributes => {
                    let attribute_name = dom_command.attribute_name.as_ref().ok_or_else(|| "Attribute name missing for GetAllAttributes".to_string()).unwrap(); // Should be Some by parse_dom_command
                    match dom_utils::get_all_elements_attributes(&dom_command.selector, attribute_name) {
                        Ok(json_string) => Ok(format!("Agent {} ({:?}): Successfully retrieved attributes '{}' for elements matching selector '{}': {}", selected_agent.id, selected_agent.role, attribute_name, dom_command.selector, json_string)),
                        Err(e) => Err(format!("Agent {} ({:?}): Error getting all attributes: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                    }
                }
            }
        } else {
            // Fallback to LLM call logic
            console::log_1(&format!("Agent {} ({:?}): No DOM command parsed or task is not a DOM command. Defaulting to LLM for task: {}", selected_agent.id, selected_agent.role, task).into());
            
            let prompt_for_llm = generate_structured_llm_prompt(
                selected_agent.id, 
                &selected_agent.role, 
                task, 
                &AVAILABLE_DOM_COMMANDS // Pass as a slice
            );
            
            match call_llm_async(prompt_for_llm, api_key.to_string(), api_url.to_string(), model_name.to_string()).await {
                Ok(llm_response) => {
                    // Attempt to parse the llm_response as a JSON array of LlmDomCommandRequest
                    match serde_json::from_str::<Vec<LlmDomCommandRequest>>(&llm_response) {
                        Ok(llm_commands) => {
                            let mut results = Vec::new();
                            if llm_commands.is_empty() {
                                // If LLM returns empty array, treat as natural language response or a specific non-action.
                                console::log_1(&format!("Agent {} ({:?}): LLM returned an empty command array. Treating as natural language response: {}", selected_agent.id, selected_agent.role, llm_response).into());
                                return Ok(format!("Agent {} ({:?}) completed task via LLM: {}", selected_agent.id, selected_agent.role, llm_response));
                            }

                            console::log_1(&format!("Agent {} ({:?}): LLM returned {} commands. Executing...", selected_agent.id, selected_agent.role, llm_commands.len()).into());

                            for llm_cmd_req in llm_commands {
                                // Validate and convert LlmDomCommandRequest to DomCommand
                                let action_upper = llm_cmd_req.action.to_uppercase();
                                let dom_action = match action_upper.as_str() {
                                    "CLICK" => DomCommandAction::Click,
                                    "TYPE" => DomCommandAction::Type,
                                    "READ" => DomCommandAction::Read,
                                    "GETVALUE" => DomCommandAction::GetValue,
                                    "GETATTRIBUTE" => DomCommandAction::GetAttribute,
                                    "SETATTRIBUTE" => DomCommandAction::SetAttribute,
                                    "SELECTOPTION" => DomCommandAction::SelectOption,
                                    "GET_ALL_ATTRIBUTES" => DomCommandAction::GetAllAttributes,
                                    _ => {
                                        let err_msg = format!("Invalid action '{}' from LLM.", llm_cmd_req.action);
                                        console::log_1(&err_msg.clone().into());
                                        results.push(Err(err_msg));
                                        continue; // Skip this invalid command
                                    }
                                };

                                // Validate required fields based on action
                                match dom_action {
                                    DomCommandAction::Type | DomCommandAction::SetAttribute | DomCommandAction::SelectOption => {
                                        if llm_cmd_req.value.is_none() {
                                            let err_msg = format!("Action {:?} requires 'value'. Command: {:?}", dom_action, llm_cmd_req);
                                            console::log_1(&err_msg.clone().into());
                                            results.push(Err(err_msg));
                                            continue;
                                        }
                                    }
                                    _ => {}
                                }
                                match dom_action {
                                    DomCommandAction::GetAttribute | DomCommandAction::SetAttribute | DomCommandAction::GetAllAttributes => {
                                        if llm_cmd_req.attribute_name.is_none() {
                                            let err_msg = format!("Action {:?} requires 'attribute_name'. Command: {:?}", dom_action, llm_cmd_req);
                                            console::log_1(&err_msg.clone().into());
                                            results.push(Err(err_msg));
                                            continue;
                                        }
                                    }
                                    _ => {}
                                }


                                let dom_command = DomCommand {
                                    action: dom_action,
                                    selector: llm_cmd_req.selector,
                                    value: llm_cmd_req.value,
                                    attribute_name: llm_cmd_req.attribute_name,
                                };

                                // Execute the dom_command
                                let cmd_result = match &dom_command.action {
                                    DomCommandAction::Click => match click_element(&dom_command.selector) {
                                        Ok(_) => Ok(format!("Successfully clicked element with selector: '{}'", dom_command.selector)),
                                        Err(e) => Err(format!("Error clicking element: {:?}", e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                                    },
                                    DomCommandAction::Type => {
                                        let text_to_type = dom_command.value.as_deref().unwrap_or_default();
                                        match type_in_element(&dom_command.selector, text_to_type) {
                                            Ok(_) => Ok(format!("Successfully typed '{}' in element with selector: '{}'", text_to_type, dom_command.selector)),
                                            Err(e) => Err(format!("Error typing in element: {:?}", e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                                        }
                                    }
                                    DomCommandAction::Read => match get_element_text(&dom_command.selector) {
                                        Ok(text) => Ok(format!("Text from element '{}': {}", dom_command.selector, text)),
                                        Err(e) => Err(format!("Error reading text from element: {:?}", e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                                    },
                                    DomCommandAction::GetValue => match get_element_value(&dom_command.selector) {
                                        Ok(value) => Ok(format!("Value from element '{}': {}", dom_command.selector, value)),
                                        Err(e) => Err(format!("Error getting value from element: {:?}", e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                                    },
                                    DomCommandAction::GetAttribute => {
                                        let attribute_name = dom_command.attribute_name.as_deref().unwrap_or_default();
                                        match get_element_attribute(&dom_command.selector, attribute_name) {
                                            Ok(value) => Ok(format!("Attribute '{}' from element '{}': {}", attribute_name, dom_command.selector, value)),
                                            Err(e) => Err(format!("Error getting attribute: {:?}", e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                                        }
                                    }
                                    DomCommandAction::SetAttribute => {
                                        let attribute_name = dom_command.attribute_name.as_deref().unwrap_or_default();
                                        let attribute_value = dom_command.value.as_deref().unwrap_or_default();
                                        match set_element_attribute(&dom_command.selector, attribute_name, attribute_value) {
                                            Ok(_) => Ok(format!("Successfully set attribute '{}' to '{}' for element '{}'", attribute_name, attribute_value, dom_command.selector)),
                                            Err(e) => Err(format!("Error setting attribute: {:?}", e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                                        }
                                    }
                                    DomCommandAction::SelectOption => {
                                        let value = dom_command.value.as_deref().unwrap_or_default();
                                        match select_dropdown_option(&dom_command.selector, value) {
                                            Ok(_) => Ok(format!("Successfully selected option '{}' for dropdown '{}'", value, dom_command.selector)),
                                            Err(e) => Err(format!("Error selecting option: {:?}", e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                                        }
                                    }
                                    DomCommandAction::GetAllAttributes => {
                                        let attribute_name = dom_command.attribute_name.as_deref().unwrap_or_default();
                                        match dom_utils::get_all_elements_attributes(&dom_command.selector, attribute_name) {
                                            Ok(json_string) => Ok(format!("Successfully retrieved attributes '{}' for elements matching selector '{}': {}", attribute_name, dom_command.selector, json_string)),
                                            Err(e) => Err(format!("Error getting all attributes: {:?}", e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                                        }
                                    }
                                };
                                results.push(cmd_result);
                            }
                            // Serialize the results vector into a JSON string
                            match serde_json::to_string(&results) {
                                Ok(json_results) => Ok(json_results),
                                Err(e) => {
                                    console::log_1(&format!("Error serializing command results: {:?}", e).into());
                                    Err(format!("Agent {} ({:?}): Error serializing LLM command results: {}", selected_agent.id, selected_agent.role, e.to_string()))
                                }
                            }
                        }
                        Err(e) => {
                            // JSON parsing failed, treat as plain text response
                            console::log_1(&format!("Agent {} ({:?}): LLM response was not valid JSON array of commands (Error: {}). Treating as natural language: {}", selected_agent.id, selected_agent.role, e, llm_response).into());
                            Ok(format!("Agent {} ({:?}) completed task via LLM: {}", selected_agent.id, selected_agent.role, llm_response))
                        }
                    }
                }
                Err(js_err) => Err(format!("Agent {} ({:?}): LLM Error: {}", selected_agent.id, selected_agent.role, js_err.as_string().unwrap_or_else(|| "Unknown LLM error".to_string()))),
            }
        }
    }
}

use wasm_bindgen_test::*; // For async tests in WASM
wasm_bindgen_test_configure!(run_in_browser); // Allows tests to run in a browser-like environment

#[cfg(test)]
mod tests {
    use super::*;
    // Use wasm_bindgen_test for async tests
    #[wasm_bindgen_test]
    async fn test_agent_system_new() { // Renamed to async, though not strictly necessary for this test
        let agent_system = AgentSystem::new();
        assert_eq!(agent_system.agents.len(), 3, "AgentSystem should initialize with 3 agents.");
        
        assert_eq!(agent_system.agents[0].id, 1);
        assert_eq!(agent_system.agents[0].role, AgentRole::Navigator, "Agent 1 should be Navigator.");
        
        assert_eq!(agent_system.agents[1].id, 2);
        assert_eq!(agent_system.agents[1].role, AgentRole::FormFiller, "Agent 2 should be FormFiller.");
        
        assert_eq!(agent_system.agents[2].id, 3);
        assert_eq!(agent_system.agents[2].role, AgentRole::Generic, "Agent 3 should be Generic.");
    }

    #[wasm_bindgen_test]
    async fn test_run_task_agent_selection_and_dom_command_format() {
        let agent_system = AgentSystem::new();
        let dummy_api_key = "test_api_key";
        let dummy_api_url = "http://localhost/dummy_url_if_network_active"; // URL won't be hit if DOM commands are valid
        let dummy_model_name = "dummy_model";

        // Test cases using default CSS selectors (no prefix)
        let task_click_default_css = "CLICK #myButton";
        let res_click_default_css = agent_system.run_task(task_click_default_css, dummy_api_key, dummy_api_url, dummy_model_name).await;
        assert!(res_click_default_css.is_err() && res_click_default_css.unwrap_err().contains("CSS selector '#myButton' not found"));

        // Test cases using "css:" prefix
        let task_click_css = "CLICK css:#myButtonCss";
        let res_click_css = agent_system.run_task(task_click_css, dummy_api_key, dummy_api_url, dummy_model_name).await;
        assert!(res_click_css.is_err() && res_click_css.unwrap_err().contains("CSS selector '#myButtonCss' not found"));

        let task_type_css = "TYPE css:#userCss an_email@example.com";
        let res_type_css = agent_system.run_task(task_type_css, dummy_api_key, dummy_api_url, dummy_model_name).await;
        assert!(res_type_css.is_err() && res_type_css.unwrap_err().contains("CSS selector '#userCss' not found"));

        // Test cases using "xpath:" prefix
        let task_click_xpath = "CLICK xpath://button[@id='myButtonXpath']";
        let res_click_xpath = agent_system.run_task(task_click_xpath, dummy_api_key, dummy_api_url, dummy_model_name).await;
        assert!(res_click_xpath.is_err() && res_click_xpath.unwrap_err().contains("XPath '//button[@id='myButtonXpath']' not found"));

        let task_type_xpath = "TYPE xpath://input[@id='userXpath'] an_email@example.com";
        let res_type_xpath = agent_system.run_task(task_type_xpath, dummy_api_key, dummy_api_url, dummy_model_name).await;
        assert!(res_type_xpath.is_err() && res_type_xpath.unwrap_err().contains("XPath '//input[@id='userXpath']' not found"));

        let task_read_xpath = "READ xpath://div[@id='messageXpath']";
        let res_read_xpath = agent_system.run_task(task_read_xpath, dummy_api_key, dummy_api_url, dummy_model_name).await;
        assert!(res_read_xpath.is_err() && res_read_xpath.unwrap_err().contains("XPath '//div[@id='messageXpath']' not found"));

        let task_getvalue_xpath = "GETVALUE xpath://input[@id='inputFieldXpath']";
        let res_getvalue_xpath = agent_system.run_task(task_getvalue_xpath, dummy_api_key, dummy_api_url, dummy_model_name).await;
        assert!(res_getvalue_xpath.is_err() && res_getvalue_xpath.unwrap_err().contains("XPath '//input[@id='inputFieldXpath']' not found"));

        let task_getattribute_xpath = "GETATTRIBUTE xpath://a[@id='myLinkXpath'] href";
        let res_getattribute_xpath = agent_system.run_task(task_getattribute_xpath, dummy_api_key, dummy_api_url, dummy_model_name).await;
        assert!(res_getattribute_xpath.is_err() && res_getattribute_xpath.unwrap_err().contains("XPath '//a[@id='myLinkXpath']' not found"));

        let task_setattribute_xpath = "SETATTRIBUTE xpath://img[@id='myImageXpath'] alt New Alt Text";
        let res_setattribute_xpath = agent_system.run_task(task_setattribute_xpath, dummy_api_key, dummy_api_url, dummy_model_name).await;
        assert!(res_setattribute_xpath.is_err() && res_setattribute_xpath.unwrap_err().contains("XPath '//img[@id='myImageXpath']' not found"));

        let task_selectoption_xpath = "SELECTOPTION xpath://select[@id='myDropdownXpath'] option2";
        let res_selectoption_xpath = agent_system.run_task(task_selectoption_xpath, dummy_api_key, dummy_api_url, dummy_model_name).await;
        assert!(res_selectoption_xpath.is_err() && res_selectoption_xpath.unwrap_err().contains("XPath '//select[@id='myDropdownXpath']' not found"));

        // Test for GET_ALL_ATTRIBUTES
        let task_get_all_attrs_css = "GET_ALL_ATTRIBUTES .myClass data-value";
        let res_get_all_attrs_css = agent_system.run_task(task_get_all_attrs_css, dummy_api_key, dummy_api_url, dummy_model_name).await;
        // Since no elements ".myClass" exist, dom_utils::get_all_elements_attributes should return Ok("[]")
        assert!(res_get_all_attrs_css.is_ok(), "Expected Ok for GET_ALL_ATTRIBUTES on non-existent class, got: {:?}", res_get_all_attrs_css.err());
        let ok_response_css = res_get_all_attrs_css.unwrap();
        assert!(ok_response_css.contains("Successfully retrieved attributes 'data-value' for elements matching selector '.myClass': []"), "Unexpected response: {}", ok_response_css);

        let task_get_all_attrs_xpath = "GET_ALL_ATTRIBUTES xpath://div[@class='myXPathClass'] data-id";
        let res_get_all_attrs_xpath = agent_system.run_task(task_get_all_attrs_xpath, dummy_api_key, dummy_api_url, dummy_model_name).await;
        assert!(res_get_all_attrs_xpath.is_ok(), "Expected Ok for GET_ALL_ATTRIBUTES on non-existent xpath, got: {:?}", res_get_all_attrs_xpath.err());
        let ok_response_xpath = res_get_all_attrs_xpath.unwrap();
        assert!(ok_response_xpath.contains("Successfully retrieved attributes 'data-id' for elements matching selector 'xpath://div[@class='myXPathClass']': []"), "Unexpected response: {}", ok_response_xpath);

        let task_get_all_attrs_invalid_selector = "GET_ALL_ATTRIBUTES css:[[[ data-value";
        let res_get_all_attrs_invalid = agent_system.run_task(task_get_all_attrs_invalid_selector, dummy_api_key, dummy_api_url, dummy_model_name).await;
        assert!(res_get_all_attrs_invalid.is_err(), "Expected Err for invalid selector, got: {:?}", res_get_all_attrs_invalid);
        assert!(res_get_all_attrs_invalid.unwrap_err().contains("InvalidSelector: Invalid CSS selector"));


        // Example of agent selection still working (FormFiller for "TYPE" command with XPath)
        // The agent selection happens BEFORE parse_dom_command, so this test should still reflect the correct agent role.
        if let Err(e) = res_type_xpath { // Re-check res_type_xpath for agent role
             assert!(e.contains("Agent 2 (FormFiller)"), "Task '{}' error should mention FormFiller Agent. Got: {}", task_type_xpath, e);
        } else {
            panic!("res_type_xpath should have been an error");
        }

        // Example of agent selection for Navigator with a task that WILL FALLBACK to LLM
        // because "navigate then CLICK" is not a single DOM command.
        // The parse_dom_command will return None, leading to LLM fallback.
        // The agent selection for "navigate" tasks should still pick the Navigator agent.
        let task_nav_click_xpath_fallback = "navigate then CLICK xpath://button[@id='specificButtonXpath']";
        let result_nav_click_xpath_fallback = agent_system.run_task(task_nav_click_xpath_fallback, dummy_api_key, dummy_api_url, dummy_model_name).await;

        // This will now go to LLM. If mock-llm is enabled, it will be an Ok result.
        #[cfg(feature = "mock-llm")]
        {
            assert!(result_nav_click_xpath_fallback.is_ok(), "Task '{}' should fallback to LLM and be Ok with mock. Got: {:?}", task_nav_click_xpath_fallback, result_nav_click_xpath_fallback);
            let response_text = result_nav_click_xpath_fallback.unwrap();
            assert!(response_text.contains("Agent 1 (Navigator) completed task via LLM: Mocked LLM response for 'navigate then CLICK xpath://button[@id='specificButtonXpath']'"), "Task '{}' response incorrect. Got: {}", task_nav_click_xpath_fallback, response_text);
        }
        #[cfg(not(feature = "mock-llm"))]
        {
            // Without mock-llm, it will be an Err because of the network call attempt.
            assert!(result_nav_click_xpath_fallback.is_err(), "Task '{}' should fallback to LLM and be Err without mock. Got: {:?}", task_nav_click_xpath_fallback, result_nav_click_xpath_fallback);
            if let Err(e) = result_nav_click_xpath_fallback {
                 assert!(e.contains("Agent 1 (Navigator): LLM Error:"), "Task '{}' error should mention Navigator Agent and LLM Error. Got: {}", task_nav_click_xpath_fallback, e);
            }
        }

        // Test a pure CLICK command that should be handled by Navigator due to "navigate" keyword in task context
        // for agent selection, but "CLICK" itself is a valid DOM command.
        // This tests if agent selection correctly picks Navigator, and then parse_dom_command correctly parses "CLICK".
        let task_nav_then_direct_click = "navigate then CLICK #myButtonDirect"; // "navigate" for agent, "CLICK" for command
        let result_nav_direct_click = agent_system.run_task(task_nav_then_direct_click, dummy_api_key, dummy_api_url, dummy_model_name).await;
        // This should be parsed as a CLICK command by parse_dom_command, but the task string for agent selection contains "navigate"
        // The current agent selection logic looks at the whole task string.
        // parse_dom_command looks at the whole string.
        // The task "navigate then CLICK #myButtonDirect" will be given to parse_dom_command.
        // parse_dom_command will see "NAVIGATE" as the command, which is not a DOM command, so it will return None.
        // So this will also go to LLM.

        #[cfg(feature = "mock-llm")]
        {
            assert!(result_nav_direct_click.is_ok(), "Task '{}' should fallback to LLM and be Ok with mock. Got: {:?}", task_nav_then_direct_click, result_nav_direct_click);
            let response_text = result_nav_direct_click.unwrap();
            assert!(response_text.contains("Agent 1 (Navigator) completed task via LLM: Mocked LLM response for 'navigate then CLICK #myButtonDirect'"), "Task '{}' response incorrect. Got: {}", task_nav_then_direct_click, response_text);
        }
        #[cfg(not(feature = "mock-llm"))]
        {
            assert!(result_nav_direct_click.is_err(), "Task '{}' should fallback to LLM and be Err without mock. Got: {:?}", task_nav_then_direct_click, result_nav_direct_click);
             if let Err(e) = result_nav_direct_click {
                 assert!(e.contains("Agent 1 (Navigator): LLM Error:"), "Task '{}' error should mention Navigator Agent and LLM Error. Got: {}", task_nav_then_direct_click, e);
            }
        }
    }

    #[wasm_bindgen_test]
    async fn test_run_task_llm_fallback_agent_selection() {
        let agent_system = AgentSystem::new();
        let dummy_api_key = "test_api_key_llm_will_fail_network";
        let dummy_api_url = "http://localhost:12345/nonexistent_endpoint"; // Ensure network call fails
        let dummy_model_name = "dummy_model_llm";

        // Task for Navigator (LLM fallback)
        let task_nav = "navigate to example.com";
        let result_nav = agent_system.run_task(task_nav, dummy_api_key, dummy_api_url, dummy_model_name).await;
        #[cfg(feature = "mock-llm")]
        {
            assert!(result_nav.is_ok(), "LLM fallback for NAV should be Ok with mock. Got: {:?}", result_nav);
            let response_text = result_nav.unwrap();
            assert!(response_text.contains("Agent 1 (Navigator) completed task via LLM: Mocked LLM response for 'navigate to example.com'"), "Unexpected mock response for NAV: {}", response_text);
        }
        #[cfg(not(feature = "mock-llm"))]
        {
            assert!(result_nav.is_err(), "LLM fallback for NAV should error without mock. Got: {:?}", result_nav);
        }


        // Task for FormFiller (LLM fallback)
        let task_form = "fill the login form with my details";
        let result_form = agent_system.run_task(task_form, dummy_api_key, dummy_api_url, dummy_model_name).await;
        #[cfg(feature = "mock-llm")]
        {
            assert!(result_form.is_ok(), "LLM fallback for FORM should be Ok with mock. Got: {:?}", result_form);
            let response_text = result_form.unwrap();
            assert!(response_text.contains("Agent 2 (FormFiller) completed task via LLM: Mocked LLM response for 'fill the login form with my details'"), "Unexpected mock response for FORM: {}", response_text);
        }
        #[cfg(not(feature = "mock-llm"))]
        {
            assert!(result_form.is_err(), "LLM fallback for FORM should error without mock. Got: {:?}", result_form);
        }

        // Task for Generic (LLM fallback)
        let task_generic = "summarize this document for me";
        let result_generic = agent_system.run_task(task_generic, dummy_api_key, dummy_api_url, dummy_model_name).await;
        #[cfg(feature = "mock-llm")]
        {
            assert!(result_generic.is_ok(), "LLM fallback for GENERIC should be Ok with mock. Got: {:?}", result_generic);
            let response_text = result_generic.unwrap();
            assert!(response_text.contains("Agent 3 (Generic) completed task via LLM: Mocked LLM response for 'summarize this document for me'"), "Unexpected mock response for GENERIC: {}", response_text);
        }
        #[cfg(not(feature = "mock-llm"))]
        {
            assert!(result_generic.is_err(), "LLM fallback for GENERIC should error without mock. Got: {:?}", result_generic);
        }

        // Test for LLM error mock
        let task_fail_llm = "this task should fail_llm_call please";
         let result_fail = agent_system.run_task(task_fail_llm, dummy_api_key, dummy_api_url, dummy_model_name).await;
        #[cfg(feature = "mock-llm")]
        {
            assert!(result_fail.is_err(), "LLM call should have failed with mock. Got: {:?}", result_fail);
            let error_text = result_fail.unwrap_err();
            assert!(error_text.contains("Agent 3 (Generic): LLM Error: Mocked LLM Error: LLM call failed as requested by prompt."), "Unexpected mock error message: {}", error_text);
        }
         #[cfg(not(feature = "mock-llm"))]
        {
            assert!(result_fail.is_err(), "LLM call should also fail without mock (network or parsing). Got: {:?}", result_fail);
        }
    }

    // New tests for LLM JSON response handling (ensure 'mock-llm' feature is active for these)
    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_json_single_valid_command() {
        let agent_system = AgentSystem::new();
        let task = "click the submit button"; // Triggers mock: [{"action": "CLICK", "selector": "css:#submitBtn"}]
        // This task is generic, so Agent 3 (Generic) should be selected.
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
        let result_str = result.unwrap();
        // Expected: JSON array of results. DOM command will fail as no element exists.
        let expected_inner_json = r#"[{"error":"Error clicking element: \"JsValue(TypeError: Document.querySelector: '#submitBtn' is not a valid selector)\""}]"#; // Exact error message may vary slightly based on browser/wasm-env
        
        // We need to check if the result_str contains the core parts of the expected message,
        // as the exact JsValue error string can be tricky.
        assert!(result_str.contains("Error clicking element"), "Result string does not contain 'Error clicking element': {}", result_str);
        assert!(result_str.contains("#submitBtn"), "Result string does not contain selector '#submitBtn': {}", result_str);
        assert!(result_str.starts_with("[") && result_str.ends_with("]"), "Result string is not a JSON array: {}", result_str);

        // To make it more robust, let's parse the outer and inner JSON if possible
        match serde_json::from_str::<Vec<Result<String, String>>>(&result_str) {
            Ok(outer_array) => {
                assert_eq!(outer_array.len(), 1, "Expected one result in the outer array");
                assert!(outer_array[0].is_err(), "Expected inner result to be an error");
                let inner_error_msg = outer_array[0].as_ref().err().unwrap();
                assert!(inner_error_msg.contains("Error clicking element:"), "Inner error message mismatch: {}", inner_error_msg);
                assert!(inner_error_msg.contains("#submitBtn"), "Inner error message should contain selector: {}", inner_error_msg);
            }
            Err(e) => panic!("Failed to parse result_str as JSON array of results: {}, content: {}", e, result_str),
        }
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_json_multiple_valid_commands() {
        let agent_system = AgentSystem::new();
        let task = "login with testuser and click login"; // Triggers mock: [{"action": "TYPE", "selector": "css:#username", "value": "testuser"}, {"action": "CLICK", "selector": "css:#loginBtn"}]
        // This task contains "type", so Agent 2 (FormFiller) should be selected.
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
        let result_str = result.unwrap();
        
        match serde_json::from_str::<Vec<Result<String, String>>>(&result_str) {
            Ok(results) => {
                assert_eq!(results.len(), 2, "Expected two results in the JSON array");
                // First command: TYPE
                assert!(results[0].is_err(), "Expected first command (TYPE) to result in an error (element not found)");
                let err_msg1 = results[0].as_ref().err().unwrap();
                assert!(err_msg1.contains("Error typing in element:"), "Error message for TYPE incorrect: {}", err_msg1);
                assert!(err_msg1.contains("css:#username"), "Error message for TYPE should contain selector: {}", err_msg1);

                // Second command: CLICK
                assert!(results[1].is_err(), "Expected second command (CLICK) to result in an error (element not found)");
                 let err_msg2 = results[1].as_ref().err().unwrap();
                assert!(err_msg2.contains("Error clicking element:"), "Error message for CLICK incorrect: {}", err_msg2);
                assert!(err_msg2.contains("css:#loginBtn"), "Error message for CLICK should contain selector: {}", err_msg2);
            }
            Err(e) => panic!("Failed to parse result_str as JSON array of results: {}, content: {}", e, result_str),
        }
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_invalid_json_string() {
        let agent_system = AgentSystem::new();
        let task = "task expected to return invalid json"; // Triggers mock: "This is not JSON."
        // Generic task, Agent 3
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
        let expected_response = "Agent 3 (Generic) completed task via LLM: This is not JSON.";
        assert_eq!(result.unwrap(), expected_response);
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_json_object_not_array() {
        let agent_system = AgentSystem::new();
        let task = "task expected to return json object not array"; // Triggers mock: {"message": "This is a JSON object, not an array."}
        // Generic task, Agent 3
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
        let expected_response = "Agent 3 (Generic) completed task via LLM: {\"message\": \"This is a JSON object, not an array.\"}";
        assert_eq!(result.unwrap(), expected_response);
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_json_array_not_commands() {
        let agent_system = AgentSystem::new();
        let task = "task expected to return json array of non-commands"; // Triggers mock: [{"foo": "bar"}]
        // Generic task, Agent 3
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
        // This case should fall back to plain text because the inner objects don't match LlmDomCommandRequest
        let expected_response = "Agent 3 (Generic) completed task via LLM: [{\"foo\": \"bar\"}]";
        assert_eq!(result.unwrap(), expected_response);
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_json_array_mixed_valid_invalid_commands() {
        let agent_system = AgentSystem::new();
        // Triggers mock: [{"action": "CLICK", "selector": "css:#ok"}, {"action": "INVALID_ACTION", "selector": "css:#bad"}, {"action": "TYPE", "selector": "css:#missingValue"}]
        let task = "task with mixed valid and invalid commands"; 
        // Generic task, Agent 3
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
        let result_str = result.unwrap();

        match serde_json::from_str::<Vec<Result<String, String>>>(&result_str) {
            Ok(results) => {
                assert_eq!(results.len(), 3, "Expected three results in the JSON array");

                // 1. Valid CLICK (will fail due to missing element, which is fine)
                assert!(results[0].is_err(), "Expected first command (CLICK) to result in an error");
                assert!(results[0].as_ref().err().unwrap().contains("Error clicking element:"));
                assert!(results[0].as_ref().err().unwrap().contains("css:#ok"));
                
                // 2. Invalid action "INVALID_ACTION"
                assert!(results[1].is_err(), "Expected second command (INVALID_ACTION) to be an error");
                assert_eq!(results[1].as_ref().err().unwrap(), "Invalid action 'INVALID_ACTION' from LLM.");
                
                // 3. Valid action "TYPE" but missing "value"
                assert!(results[2].is_err(), "Expected third command (TYPE missing value) to be an error");
                assert!(results[2].as_ref().err().unwrap().contains("Action Type requires 'value'. Command: LlmDomCommandRequest { action: \"TYPE\", selector: \"css:#missingValue\", value: None, attribute_name: None }"));
            }
            Err(e) => panic!("Failed to parse result_str as JSON array of results: {}, content: {}", e, result_str),
        }
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_json_empty_array() {
        let agent_system = AgentSystem::new();
        let task = "task expected to return empty command array"; // Triggers mock: []
        // Generic task, Agent 3
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
        // If LLM returns empty array, it's treated as a natural language response of "[]"
        let expected_response = "Agent 3 (Generic) completed task via LLM: []";
        assert_eq!(result.unwrap(), expected_response);
    }
}