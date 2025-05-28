use crate::llm::call_llm;
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

    // run_task now takes an api_key and uses &self
    pub fn run_task(&self, task: &str, api_key: &str) -> String {
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
                    return format!("Agent {} ({:?}): Error - CLICK command requires a selector.", selected_agent.id, selected_agent.role);
                }
                console::log_1(&format!("Agent {} ({:?}): Executing CLICK: selector='{}'", selected_agent.id, selected_agent.role, selector).into());
                match click_element(selector) {
                    Ok(_) => format!("Agent {} ({:?}): Successfully clicked element with selector: '{}'", selected_agent.id, selected_agent.role, selector),
                    Err(e) => format!("Agent {} ({:?}): Error clicking element: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string())),
                }
            }
            "TYPE" => {
                let sub_parts: Vec<&str> = args_str.splitn(2, ' ').collect();
                let selector = sub_parts.get(0).unwrap_or(&"");
                let text_to_type = sub_parts.get(1).unwrap_or(&"");

                if selector.is_empty() || text_to_type.is_empty() {
                    return format!("Agent {} ({:?}): Error - TYPE command requires a selector and text.", selected_agent.id, selected_agent.role);
                }
                console::log_1(&format!("Agent {} ({:?}): Executing TYPE: selector='{}', text='{}'", selected_agent.id, selected_agent.role, selector, text_to_type).into());
                match type_in_element(selector, text_to_type) {
                    Ok(_) => format!("Agent {} ({:?}): Successfully typed '{}' in element with selector: '{}'", selected_agent.id, selected_agent.role, text_to_type, selector),
                    Err(e) => format!("Agent {} ({:?}): Error typing in element: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string())),
                }
            }
            "READ" => {
                let selector = args_str;
                if selector.is_empty() {
                    return format!("Agent {} ({:?}): Error - READ command requires a selector.", selected_agent.id, selected_agent.role);
                }
                console::log_1(&format!("Agent {} ({:?}): Executing READ: selector='{}'", selected_agent.id, selected_agent.role, selector).into());
                match get_element_text(selector) {
                    Ok(text) => format!("Agent {} ({:?}): Text from element '{}': {}", selected_agent.id, selected_agent.role, selector, text),
                    Err(e) => format!("Agent {} ({:?}): Error reading text from element: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string())),
                }
            }
            "GETVALUE" => {
                let selector = args_str;
                if selector.is_empty() {
                    return format!("Agent {} ({:?}): Error - GETVALUE command requires a selector.", selected_agent.id, selected_agent.role);
                }
                console::log_1(&format!("Agent {} ({:?}): Executing GETVALUE: selector='{}'", selected_agent.id, selected_agent.role, selector).into());
                match get_element_value(selector) {
                    Ok(value) => format!("Agent {} ({:?}): Value from element '{}': {}", selected_agent.id, selected_agent.role, selector, value),
                    Err(e) => format!("Agent {} ({:?}): Error getting value from element: {:?}", selected_agent.id, selected_agent.role, e.as_string().unwrap_or_else(|| "Unknown error".to_string())),
                }
            }
            _ => {
                // Fallback to LLM call logic
                console::log_1(&format!("Agent {} ({:?}): No DOM command matched. Defaulting to LLM for task: {}", selected_agent.id, selected_agent.role, task).into());
                
                let prompt_for_llm = format!("Agent {} ({:?}): {}", selected_agent.id, selected_agent.role, task);
                // In a test environment, call_llm might return a dummy or error string.
                // This is acceptable as we are not testing the LLM's output itself here.
                let llm_response = call_llm(&prompt_for_llm, api_key); 
                
                format!("Agent {} ({:?}) completed task via LLM: {}", selected_agent.id, selected_agent.role, llm_response)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_system_new() {
        let agent_system = AgentSystem::new();
        assert_eq!(agent_system.agents.len(), 3, "AgentSystem should initialize with 3 agents.");
        
        assert_eq!(agent_system.agents[0].id, 1);
        assert_eq!(agent_system.agents[0].role, AgentRole::Navigator, "Agent 1 should be Navigator.");
        
        assert_eq!(agent_system.agents[1].id, 2);
        assert_eq!(agent_system.agents[1].role, AgentRole::FormFiller, "Agent 2 should be FormFiller.");
        
        assert_eq!(agent_system.agents[2].id, 3);
        assert_eq!(agent_system.agents[2].role, AgentRole::Generic, "Agent 3 should be Generic.");
    }

    #[test]
    fn test_run_task_agent_selection_and_dom_command_format() {
        let agent_system = AgentSystem::new();
        let dummy_api_key = "test_api_key";

        // Test case 1: CLICK command - should select Navigator or FormFiller based on keywords.
        // Current logic: "click" is not an explicit keyword for Navigator/FormFiller, so Generic is selected.
        // Let's refine this test based on actual selection. "CLICK" as a DOM command itself doesn't have
        // keywords like "navigate" or "fill". So, it should default to Generic agent for the overall task context
        // if no other keywords are present in the *full task string*.
        // If the task was "navigate and CLICK #button", Navigator would be chosen.
        // If the task was "fill form and CLICK #submit", FormFiller would be chosen.
        // For just "CLICK #myButton", the Generic agent (id 3) is selected first.
        let task1 = "CLICK #myButton";
        let result1 = agent_system.run_task(task1, dummy_api_key);
        // The agent selection for DOM commands uses selected_agent, which defaults to Generic if no keywords.
        assert!(result1.contains("Agent 3 (Generic)"), "Task '{}' should be handled by Generic Agent. Got: {}", task1, result1);
        assert!(result1.contains("Error clicking element:") || result1.contains("Successfully clicked element"), "Output for CLICK incorrect. Got: {}", result1);

        // Test case 2: TYPE command - should select FormFiller based on "type" keyword.
        let task2 = "TYPE #user an_email@example.com"; // "type" keyword for FormFiller
        let result2 = agent_system.run_task(task2, dummy_api_key);
        assert!(result2.contains("Agent 2 (FormFiller)"), "Task '{}' should be handled by FormFiller. Got: {}", task2, result2);
        assert!(result2.contains("Error typing in element:") || result2.contains("Successfully typed"), "Output for TYPE incorrect. Got: {}", result2);

        // Test case 3: READ command - should select Generic agent.
        let task3 = "READ #message";
        let result3 = agent_system.run_task(task3, dummy_api_key);
        assert!(result3.contains("Agent 3 (Generic)"), "Task '{}' should be handled by Generic Agent. Got: {}", task3, result3);
        assert!(result3.contains("Error reading text from element:") || result3.contains("Text from element"), "Output for READ incorrect. Got: {}", result3);
        
        // Test case 4: GETVALUE command - should select Generic agent.
        let task4 = "GETVALUE #inputField";
        let result4 = agent_system.run_task(task4, dummy_api_key);
        assert!(result4.contains("Agent 3 (Generic)"), "Task '{}' should be handled by Generic Agent. Got: {}", task4, result4);
        assert!(result4.contains("Error getting value from element:") || result4.contains("Value from element"), "Output for GETVALUE incorrect. Got: {}", result4);

        // Test case 5: Task with "navigate" keyword, but still a DOM command (e.g. a complex CLICK)
        let task5 = "navigate then CLICK #specificButton"; // "navigate" keyword for Navigator
        let result5 = agent_system.run_task(task5, dummy_api_key);
        assert!(result5.contains("Agent 1 (Navigator)"), "Task '{}' should be handled by Navigator. Got: {}", task5, result5);
        // The command is still CLICK, processed by the selected Navigator agent.
        assert!(result5.contains("Error clicking element:") || result5.contains("Successfully clicked element"), "Output for CLICK by Navigator incorrect. Got: {}", result5);
    }

    #[test]
    fn test_run_task_llm_fallback_agent_selection() {
        let agent_system = AgentSystem::new();
        let dummy_api_key = "test_api_key_llm";

        // Task for Navigator
        let task_nav = "navigate to example.com";
        let result_nav = agent_system.run_task(task_nav, dummy_api_key);
        assert!(result_nav.contains("Agent 1 (Navigator) completed task via LLM:"), "LLM fallback for '{}' should use Navigator. Got: {}", task_nav, result_nav);

        // Task for FormFiller
        let task_form = "fill the login form with my details";
        let result_form = agent_system.run_task(task_form, dummy_api_key);
        assert!(result_form.contains("Agent 2 (FormFiller) completed task via LLM:"), "LLM fallback for '{}' should use FormFiller. Got: {}", task_form, result_form);

        // Task for Generic
        let task_generic = "summarize this document for me";
        let result_generic = agent_system.run_task(task_generic, dummy_api_key);
        assert!(result_generic.contains("Agent 3 (Generic) completed task via LLM:"), "LLM fallback for '{}' should use Generic. Got: {}", task_generic, result_generic);
    }
}