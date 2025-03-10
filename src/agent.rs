use crate::llm::call_llm;

pub struct Agent {
    id: u32,
    role: String, // e.g., "navigator", "form_filler"
}

pub struct AgentSystem {
    agents: Vec<Agent>,
}

impl AgentSystem {
    pub fn new() -> Self {
        let mut agents = Vec::new();
        agents.push(Agent { id: 1, role: "navigator".to_string() });
        agents.push(Agent { id: 2, role: "form_filler".to_string() });
        AgentSystem { agents }
    }

    pub fn run_task(&mut self, task: &str) -> String {
        // Simple logic: delegate task to an agent and call LLM
        let agent = &self.agents[0]; // For demo, use first agent
        let llm_response = call_llm(&format!("Agent {} ({}): {}", agent.id, agent.role, task));
        
        format!("Agent {} completed task: {}", agent.id, llm_response)
    }
}