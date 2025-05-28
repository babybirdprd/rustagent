use crate::llm::call_llm_async; // Changed from call_llm
use crate::dom_utils::*; // Import DOM utility functions
use web_sys::console; // For logging unexpected parsing issues

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
        let parts: Vec<&str> = task.splitn(2, ' ').collect();
        let command = parts.get(0).unwrap_or(&"").to_uppercase();
        let args_str = parts.get(1).unwrap_or(&"");

        match command.as_str() {
            "CLICK" => {
                let selector = args_str;
                if selector.is_empty() {
                    return Err(format!("Agent {} ({:?}): Error - CLICK command requires a selector.", selected_agent.id, selected_agent.role));
                }
                console::log_1(&format!("Agent {} ({:?}): Executing CLICK: selector='{}'", selected_agent.id, selected_agent.role, selector).into());
                match click_element(selector) {
                    Ok(_) => Ok(format!("Agent {} ({:?}): Successfully clicked element with selector: '{}'", selected_agent.id, selected_agent.role, selector)),
                    Err(e) => Err(format!("Agent {} ({:?}): Error clicking element: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                }
            }
            "TYPE" => {
                let sub_parts: Vec<&str> = args_str.splitn(2, ' ').collect();
                let selector = sub_parts.get(0).unwrap_or(&"");
                let text_to_type = sub_parts.get(1).unwrap_or(&"");

                if selector.is_empty() || text_to_type.is_empty() {
                    return Err(format!("Agent {} ({:?}): Error - TYPE command requires a selector and text.", selected_agent.id, selected_agent.role));
                }
                console::log_1(&format!("Agent {} ({:?}): Executing TYPE: selector='{}', text='{}'", selected_agent.id, selected_agent.role, selector, text_to_type).into());
                match type_in_element(selector, text_to_type) {
                    Ok(_) => Ok(format!("Agent {} ({:?}): Successfully typed '{}' in element with selector: '{}'", selected_agent.id, selected_agent.role, text_to_type, selector)),
                    Err(e) => Err(format!("Agent {} ({:?}): Error typing in element: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                }
            }
            "READ" => {
                let selector = args_str;
                if selector.is_empty() {
                    return Err(format!("Agent {} ({:?}): Error - READ command requires a selector.", selected_agent.id, selected_agent.role));
                }
                console::log_1(&format!("Agent {} ({:?}): Executing READ: selector='{}'", selected_agent.id, selected_agent.role, selector).into());
                match get_element_text(selector) {
                    Ok(text) => Ok(format!("Agent {} ({:?}): Text from element '{}': {}", selected_agent.id, selected_agent.role, selector, text)),
                    Err(e) => Err(format!("Agent {} ({:?}): Error reading text from element: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                }
            }
            "GETVALUE" => {
                let selector = args_str;
                if selector.is_empty() {
                    return Err(format!("Agent {} ({:?}): Error - GETVALUE command requires a selector.", selected_agent.id, selected_agent.role));
                }
                console::log_1(&format!("Agent {} ({:?}): Executing GETVALUE: selector='{}'", selected_agent.id, selected_agent.role, selector).into());
                match get_element_value(selector) {
                    Ok(value) => Ok(format!("Agent {} ({:?}): Value from element '{}': {}", selected_agent.id, selected_agent.role, selector, value)),
                    Err(e) => Err(format!("Agent {} ({:?}): Error getting value from element: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                }
            }
            "GETATTRIBUTE" => {
                let parts: Vec<&str> = args_str.splitn(2, ' ').collect();
                let selector = parts.get(0).unwrap_or(&"");
                let attribute_name = parts.get(1).unwrap_or(&"");
                if selector.is_empty() || attribute_name.is_empty() {
                    return Err(format!("Agent {} ({:?}): Error - GETATTRIBUTE command requires a selector and an attribute name.", selected_agent.id, selected_agent.role));
                }
                console::log_1(&format!("Agent {} ({:?}): Executing GETATTRIBUTE: selector='{}', attribute_name='{}'", selected_agent.id, selected_agent.role, selector, attribute_name).into());
                match get_element_attribute(selector, attribute_name) {
                    Ok(value) => Ok(format!("Agent {} ({:?}): Attribute '{}' from element '{}': {}", selected_agent.id, selected_agent.role, attribute_name, selector, value)),
                    Err(e) => Err(format!("Agent {} ({:?}): Error getting attribute: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                }
            }
            "SETATTRIBUTE" => {
                let parts: Vec<&str> = args_str.splitn(3, ' ').collect();
                let selector = parts.get(0).unwrap_or(&"");
                let attribute_name = parts.get(1).unwrap_or(&"");
                let attribute_value = parts.get(2).unwrap_or(&"");
                if selector.is_empty() || attribute_name.is_empty() || attribute_value.is_empty() {
                    return Err(format!("Agent {} ({:?}): Error - SETATTRIBUTE command requires a selector, an attribute name, and a value.", selected_agent.id, selected_agent.role));
                }
                console::log_1(&format!("Agent {} ({:?}): Executing SETATTRIBUTE: selector='{}', attribute_name='{}', value='{}'", selected_agent.id, selected_agent.role, selector, attribute_name, attribute_value).into());
                match set_element_attribute(selector, attribute_name, attribute_value) {
                    Ok(_) => Ok(format!("Agent {} ({:?}): Successfully set attribute '{}' to '{}' for element '{}'", selected_agent.id, selected_agent.role, attribute_name, attribute_value, selector)),
                    Err(e) => Err(format!("Agent {} ({:?}): Error setting attribute: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                }
            }
            "SELECTOPTION" => {
                let parts: Vec<&str> = args_str.splitn(2, ' ').collect();
                let selector = parts.get(0).unwrap_or(&"");
                let value = parts.get(1).unwrap_or(&"");
                if selector.is_empty() || value.is_empty() {
                    return Err(format!("Agent {} ({:?}): Error - SELECTOPTION command requires a selector and a value.", selected_agent.id, selected_agent.role));
                }
                console::log_1(&format!("Agent {} ({:?}): Executing SELECTOPTION: selector='{}', value='{}'", selected_agent.id, selected_agent.role, selector, value).into());
                match select_dropdown_option(selector, value) {
                    Ok(_) => Ok(format!("Agent {} ({:?}): Successfully selected option '{}' for dropdown '{}'", selected_agent.id, selected_agent.role, value, selector)),
                    Err(e) => Err(format!("Agent {} ({:?}): Error selecting option: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string()))),
                }
            }
            _ => {
                // Fallback to LLM call logic
                console::log_1(&format!("Agent {} ({:?}): No DOM command matched. Defaulting to LLM for task: {}", selected_agent.id, selected_agent.role, task).into());
                
                let prompt_for_llm = format!("Agent {} ({:?}): {}", selected_agent.id, selected_agent.role, task);
                
                // Call the async LLM function and await its result
                match call_llm_async(prompt_for_llm, api_key.to_string(), api_url.to_string(), model_name.to_string()).await {
                    Ok(llm_response) => Ok(format!("Agent {} ({:?}) completed task via LLM: {}", selected_agent.id, selected_agent.role, llm_response)),
                    Err(js_err) => Err(format!("Agent {} ({:?}): LLM Error: {}", selected_agent.id, selected_agent.role, js_err.as_string().unwrap_or_else(|| "Unknown LLM error".to_string()))),
                }
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

        // Example of agent selection still working (FormFiller for "TYPE" command with XPath)
        if let Err(e) = res_type_xpath { // Re-check res_type_xpath for agent role
             assert!(e.contains("Agent 2 (FormFiller)"), "Task '{}' error should mention FormFiller Agent. Got: {}", task_type_xpath, e);
        } else {
            panic!("res_type_xpath should have been an error");
        }
        
        // Example of agent selection for Navigator with XPath
        let task_nav_click_xpath = "navigate then CLICK xpath://button[@id='specificButtonXpath']";
        let result_nav_click_xpath = agent_system.run_task(task_nav_click_xpath, dummy_api_key, dummy_api_url, dummy_model_name).await;
        assert!(result_nav_click_xpath.is_err(), "navigate then CLICK with XPath should result in an error in test env. Got: {:?}", result_nav_click_xpath);
        if let Err(e) = result_nav_click_xpath {
            assert!(e.contains("Agent 1 (Navigator)"), "Task '{}' error should mention Navigator Agent. Got: {}", task_nav_click_xpath, e);
            assert!(e.contains("Error clicking element:"), "Error for CLICK by Navigator incorrect. Got: {}", e);
            assert!(e.contains("XPath '//button[@id='specificButtonXpath']' not found"));
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
        // When 'mock-llm' feature is active, we expect Ok results with specific mock strings.
        // If 'mock-llm' is not active, these would make network calls and likely fail in test env,
        // so this test suite is primarily for the 'mock-llm' feature.

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
            assert!(response_text.contains("Agent 2 (FormFiller) completed task via LLM: Mocked LLM response for 'fill the login form'"), "Unexpected mock response for FORM: {}", response_text);
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
            assert!(response_text.contains("Agent 3 (Generic) completed task via LLM: Mocked LLM response for 'summarize this document'"), "Unexpected mock response for GENERIC: {}", response_text);
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
}