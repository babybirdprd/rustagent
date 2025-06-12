use crate::llm::call_llm_async; // Changed from call_llm
use crate::dom_utils::{self, DomError}; // Import DOM utility functions and DomError
use web_sys::console; // For logging unexpected parsing issues
use serde::Deserialize; // For JSON deserialization
use std::error::Error;
use std::fmt;

// Define AgentError enum
#[derive(Debug)]
pub enum AgentError {
    DomOperationFailed(DomError),
    LlmCallFailed(String),
    InvalidLlmResponse(String),
    CommandParseError(String), // For errors during the parsing of direct string commands
    SerializationError(String), // For errors during serialization of results
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::DomOperationFailed(e) => write!(f, "DOM Operation Failed: {}", e),
            AgentError::LlmCallFailed(s) => write!(f, "LLM Call Failed: {}", s),
            AgentError::InvalidLlmResponse(s) => write!(f, "Invalid LLM Response: {}", s),
            AgentError::CommandParseError(s) => write!(f, "Command Parse Error: {}", s),
            AgentError::SerializationError(s) => write!(f, "Serialization Error: {}", s),
        }
    }
}

impl Error for AgentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AgentError::DomOperationFailed(e) => Some(e),
            _ => None,
        }
    }
}

impl From<DomError> for AgentError {
    fn from(err: DomError) -> Self {
        AgentError::DomOperationFailed(err)
    }
}


// 1. Define AgentRole Enum
/// Defines the specialized roles an `Agent` can take on.
/// This helps in selecting the most appropriate agent for a given task,
/// especially when the task is not a direct DOM command and requires LLM interpretation.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentRole {
    /// Specializes in navigation tasks (e.g., going to URLs).
    Navigator,
    /// Specializes in filling out forms (e.g., typing text, selecting options).
    FormFiller,
    /// A general-purpose agent that can handle a variety of tasks or when a more specific agent isn't available/matched.
    Generic,
}

// 2. Update Agent Struct
/// Represents an agent with a specific ID, role, keywords for task matching, and a priority.
pub struct Agent {
    id: u32,
    role: AgentRole,
    /// Keywords or simple patterns that this agent is specialized for.
    keywords: Vec<String>,
    /// Priority of the agent. Higher numbers indicate higher priority.
    priority: u32,
}

/// Defines the set of specific actions an agent can perform on DOM elements.
///
/// This enum is used internally to represent the type of operation for a `DomCommand`.
/// It's also used in deserializing commands from an LLM response, where the LLM is
/// expected to provide action strings that match these variants in uppercase.
///
/// The `#[serde(rename_all = "UPPERCASE")]` attribute is crucial for robust deserialization
/// from JSON. It ensures that incoming JSON strings like `"CLICK"`, `"TYPE"`, etc.,
/// are correctly mapped to the corresponding enum variants (e.g., `DomCommandAction::Click`),
/// regardless of the case used in the Rust code for the variant names themselves.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
enum DomCommandAction {
    /// Represents a click action on a DOM element.
    Click,
    /// Represents a typing action into a DOM element (e.g., an input field).
    Type,
    /// Represents reading the text content of a DOM element.
    Read,
    /// Represents getting the value of a form element (e.g., input, textarea, select).
    GetValue,
    /// Represents getting the value of a specified attribute from a DOM element.
    GetAttribute,
    /// Represents setting the value of a specified attribute on a DOM element.
    SetAttribute,
    /// Represents selecting an option within a dropdown (`<select>`) element.
    SelectOption,
    /// Represents getting a specified attribute from all elements matching a selector.
    GetAllAttributes,
    /// Represents getting the current URL of the page.
    GetUrl,
    /// Represents checking if an element exists on the page.
    ElementExists,
    /// Represents waiting for an element to appear on the page within a timeout.
    WaitForElement,
    /// Represents checking if an element is currently visible on the page.
    IsVisible,
    /// Represents scrolling the page to make a specific element visible.
    ScrollTo,
    /// Represents hovering over a DOM element.
    Hover,
    /// Represents getting all text from elements matching a selector, joined by a separator.
    GetAllText,
}

/// Represents a fully parsed and validated command, ready for direct execution by an agent.
///
/// This struct is created either by `parse_dom_command` when processing a raw string task
/// that matches a known direct command format, or by converting an `LlmDomCommandRequest`
/// after an LLM has proposed a command. It signifies that the command's action type
/// is recognized and its essential components (like selector, and value/attribute_name
/// if required by the action) are present in a structured way.
#[derive(Debug, Clone)]
struct DomCommand {
    /// The specific DOM operation to be performed (e.g., Click, Type).
    action: DomCommandAction,
    /// The CSS selector (e.g., `css:#id`, `css:.class`) or XPath expression
    /// (e.g., `xpath://div[@id='example']`) used to target the DOM element(s) for the action.
    selector: String,
    /// An optional value associated with the action.
    /// This is used for commands like:
    /// - `TYPE`: The text to be typed into an element.
    /// - `SELECTOPTION`: The value of the option to be selected in a dropdown.
    /// - `SETATTRIBUTE`: The value to set for a specified attribute.
    /// - `WAIT_FOR_ELEMENT`: Optionally, the timeout in milliseconds.
    /// For actions that do not require an explicit value (e.g., `CLICK`, `READ`, `GET_URL`), this is `None`.
    value: Option<String>,
    /// An optional attribute name.
    /// This is used for commands like:
    /// - `GETATTRIBUTE`: The name of the attribute whose value is to be read.
    /// - `SETATTRIBUTE`: The name of the attribute whose value is to be set.
    /// - `GET_ALL_ATTRIBUTES`: The name of the attribute to retrieve from all matching elements.
    /// For actions not operating on specific attributes (e.g., `CLICK`, `TYPE`, `READ`), this is `None`.
    attribute_name: Option<String>,
}

/// Represents a command request as deserialized from an LLM's JSON output.
///
/// This struct is used as an intermediate representation when parsing JSON that is
/// expected to contain DOM commands, typically from an LLM. Its fields are more flexible
/// (e.g., `action` is a `String` rather than `DomCommandAction`) to accommodate variations
/// in LLM output format (like case differences or minor structural deviations) before
/// rigorous validation and conversion into a `DomCommand`.
#[derive(Deserialize, Debug)]
struct LlmDomCommandRequest {
    /// The action to perform, represented as a string (e.g., "CLICK", "type", "readAttribute").
    /// This string will be parsed and validated to map to a specific `DomCommandAction`.
    action: String,
    /// The CSS selector or XPath expression provided by the LLM to target the DOM element(s).
    selector: String,
    /// An optional value associated with the command, as provided by the LLM.
    /// Similar in purpose to `DomCommand::value`.
    value: Option<String>,
    /// An optional attribute name, as provided by the LLM.
    /// Similar in purpose to `DomCommand::attribute_name`.
    attribute_name: Option<String>,
}

/// A list of available direct DOM command strings with their expected arguments.
/// This is used for generating prompts for the LLM and for user reference.
// Array size should be updated if new commands are added.
const AVAILABLE_DOM_COMMANDS: [&str; 15] = [
    "CLICK <selector>",
    "TYPE <selector> <text>",
    "READ <selector>",
    "GETVALUE <selector>",
    "GETATTRIBUTE <selector> <attribute_name>",
    "SETATTRIBUTE <selector> <attribute_name> <value>",
    "SELECTOPTION <selector> <option_value>",
    "GET_ALL_ATTRIBUTES <selector> <attribute_name> (returns a JSON array of attribute values)",
    "GET_URL",
    "ELEMENT_EXISTS <selector>",
    "WAIT_FOR_ELEMENT <selector> [timeout_ms]",
    "IS_VISIBLE <selector>",
    "SCROLL_TO <selector>",
    "HOVER <selector>",
    "GET_ALL_TEXT <selector> [separator]",
];

/// Generates a structured prompt for the LLM, instructing it on how to respond
/// with either a JSON array of DOM commands or a natural language answer.
///
/// The prompt includes:
/// - The agent's persona (ID and role).
/// - The user's original task.
/// - Instructions for formatting commands as JSON objects.
/// - A list of available actions and their specific JSON schemas.
/// - An example of a valid JSON array response.
/// - Guidance on when to respond with natural language instead of commands.
///
/// # Arguments
/// * `agent_id`: The ID of the agent making the request.
/// * `agent_role`: The role of the agent.
/// * `original_task`: The user's task string.
/// * `_available_commands_list`: (Currently unused, but kept for potential future use where
///   the list of commands might be dynamically passed or filtered).
///
/// # Returns
/// A formatted string to be used as the prompt for the LLM.
fn generate_structured_llm_prompt(
    agent_id: u32,
    agent_role: &AgentRole,
    original_task: &str,
    _available_commands_list: &[&str] // Parameter kept for signature compatibility
) -> String {
    // The list of actions should ideally be derived directly from DomCommandAction variants
    // or a single source of truth to avoid discrepancies. For now, it's manually listed.
    let actions = [
        "CLICK",
        "TYPE",
        "READ",
        "GETVALUE",
        "GETATTRIBUTE",
        "SETATTRIBUTE",
        "SELECTOPTION",
        "GET_ALL_ATTRIBUTES",
        "GET_URL",
        "ELEMENT_EXISTS",
        "WAIT_FOR_ELEMENT",
        "IS_VISIBLE",
        "SCROLL_TO",
        "HOVER",
        "GET_ALL_TEXT",
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
        - Get All Attributes: {{\"action\": \"GET_ALL_ATTRIBUTES\", \"selector\": \"<selector>\", \"attribute_name\": \"<attr_name>\"}} (returns a JSON array of attribute values for all matching elements)\n\
        - Get URL: {{\"action\": \"GET_URL\"}} (gets the current page URL)\n\
        - Element Exists: {{\"action\": \"ELEMENT_EXISTS\", \"selector\": \"<selector>\"}} (checks if an element exists on the page, returns true or false)\n\
        - Wait For Element: {{\"action\": \"WAIT_FOR_ELEMENT\", \"selector\": \"<selector>\", \"value\": <timeout_in_milliseconds_optional>}} (waits for an element to exist, returns nothing on success or error on timeout/failure)\n\
        - Is Visible: {{\"action\": \"IS_VISIBLE\", \"selector\": \"<selector>\"}} (checks if an element is currently visible on the page, returns true or false)\n\
        - Scroll To: {{\"action\": \"SCROLL_TO\", \"selector\": \"<selector>\"}} (scrolls the page to make the element visible)\n\
        - Hover: {{\"action\": \"HOVER\", \"selector\": \"<selector>\"}}\n\
        - Get All Text: {{\"action\": \"GET_ALL_TEXT\", \"selector\": \"<selector>\", \"value\": \"<separator_optional>\"}} (gets text from all matching elements, joined by separator; value is the separator string)\n\n\
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

/// Parses a raw task string to determine if it represents a direct, predefined DOM command.
///
/// This function attempts to match the beginning of the `task` string (case-insensitively)
/// against a set of known command keywords (e.g., "CLICK", "TYPE", "READ"). If a keyword
/// is matched, the remainder of the string is parsed to extract the arguments expected
/// by that specific command (such as CSS selectors, text values, attribute names).
///
/// The parsing logic is tailored to each command:
/// - Commands like `CLICK`, `READ`, `GETVALUE`, `ELEMENT_EXISTS`, `IS_VISIBLE`, `SCROLL_TO`
///   expect a single argument: the selector.
/// - `GET_URL` expects no arguments.
/// - `TYPE` expects a selector and the text to type.
/// - `GETATTRIBUTE` expects a selector and an attribute name.
/// - `SETATTRIBUTE` expects a selector, an attribute name, and a value for the attribute.
/// - `SELECTOPTION` expects a selector and the value of the option to select.
/// - `GET_ALL_ATTRIBUTES` expects a selector and an attribute name.
/// - `WAIT_FOR_ELEMENT` expects a selector and an optional timeout value (in milliseconds).
///
/// If the command keyword is recognized and the subsequent arguments can be successfully
/// parsed according to the command's requirements, a `DomCommand` struct is constructed
/// and returned.
///
/// # Arguments
/// * `task`: A `&str` representing the raw task string input by the user or from a task list.
///   For example, "CLICK css:#submitButton" or "TYPE css:#username testuser".
///
/// # Returns
/// * `Some(DomCommand)`: If the `task` string is successfully parsed into a known direct
///   DOM command structure with its required arguments. The returned `DomCommand` is
///   a validated, structured representation ready for execution.
/// * `None`: If the `task` string does not match any recognized direct command keyword,
///   or if the arguments provided are insufficient or malformed for the identified command
///   (e.g., "CLICK" with no selector, "TYPE selector" with no text to type).
///   A `None` result typically signifies that the task is not a direct command and
///   should be passed to an LLM for more sophisticated interpretation.
fn parse_dom_command(task: &str) -> Option<DomCommand> {
    let parts: Vec<&str> = task.splitn(2, ' ').collect();
    let command_str = parts.get(0).unwrap_or(&"").to_uppercase(); // Command matching is case-insensitive
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
        "GET_URL" => {
            if !args_str.is_empty() { 
                console::warn_1(&format!("GET_URL command received with unexpected arguments: '{}'. Arguments will be ignored.", args_str).into());
            }
            Some(DomCommand {
                action: DomCommandAction::GetUrl,
                selector: "".to_string(), 
                value: None,
                attribute_name: None,
            })
        }
        "ELEMENT_EXISTS" => {
            if args_str.is_empty() { 
                return None; 
            }
            Some(DomCommand {
                action: DomCommandAction::ElementExists,
                selector: args_str.to_string(),
                value: None,
                attribute_name: None,
            })
        }
        "WAIT_FOR_ELEMENT" => {
            let parts: Vec<&str> = args_str.splitn(2, ' ').collect();
            let selector_str = parts.get(0).unwrap_or(&"");
            if selector_str.is_empty() { return None; }

            let timeout_val = parts.get(1).and_then(|s| s.parse::<u32>().ok());

            Some(DomCommand {
                action: DomCommandAction::WaitForElement,
                selector: selector_str.to_string(),
                value: timeout_val.map(|v| v.to_string()), 
                attribute_name: None,
            })
        }
        "IS_VISIBLE" => {
            if args_str.is_empty() { return None; }
            Some(DomCommand {
                action: DomCommandAction::IsVisible,
                selector: args_str.to_string(),
                value: None,
                attribute_name: None,
            })
        }
        "SCROLL_TO" => {
            if args_str.is_empty() { return None; }
            Some(DomCommand {
                action: DomCommandAction::ScrollTo,
                selector: args_str.to_string(),
                value: None,
                attribute_name: None,
            })
        }
        "HOVER" => {
            if args_str.is_empty() { return None; }
            Some(DomCommand {
                action: DomCommandAction::Hover,
                selector: args_str.to_string(),
                value: None,
                attribute_name: None,
            })
        }
        "GET_ALL_TEXT" => {
            let mut parts = args_str.splitn(2, ' ');
            let selector = parts.next().unwrap_or("");
            let rest = parts.next().unwrap_or("").trim();

            if selector.is_empty() { return None; }

            let separator_val: Option<String>;
            if rest.starts_with('"') && rest.ends_with('"') {
                if rest.len() >= 2 { // Ensure there are characters to strip
                    separator_val = Some(rest[1..rest.len()-1].to_string());
                } else { // Just quotes like ""
                    separator_val = Some("".to_string());
                }
            } else if !rest.is_empty() {
                separator_val = Some(rest.to_string());
            } else {
                separator_val = None; // No separator provided, will use default later
            }

            Some(DomCommand {
                action: DomCommandAction::GetAllText,
                selector: selector.to_string(),
                value: separator_val, // Store separator in value field
                attribute_name: None,
            })
        }
        _ => None,
    }
}

pub struct AgentSystem {
    agents: Vec<Agent>,
}

// Private helper function for direct DOM command execution
async fn execute_direct_dom_command(
    selected_agent: &Agent,
    dom_command: &DomCommand,
) -> Result<String, AgentError> {
    console::log_1(
        &format!(
            "Agent {} ({:?}): Executing direct DOM command: {:?}",
            selected_agent.id, selected_agent.role, dom_command
        )
        .into(),
    );
    match dom_command.action {
        DomCommandAction::Click => {
            dom_utils::click_element(&dom_command.selector)?;
            Ok(format!(
                "Agent {} ({:?}): Successfully clicked element with selector: '{}'",
                selected_agent.id, selected_agent.role, dom_command.selector
            ))
        }
        DomCommandAction::Type => {
            let text_to_type = dom_command.value.as_deref().ok_or_else(|| {
                AgentError::CommandParseError("TYPE command requires text value".to_string())
            })?;
            dom_utils::type_in_element(&dom_command.selector, text_to_type)?;
            Ok(format!(
                "Agent {} ({:?}): Successfully typed '{}' in element with selector: '{}'",
                selected_agent.id, selected_agent.role, text_to_type, dom_command.selector
            ))
        }
        DomCommandAction::Read => {
            let text = dom_utils::get_element_text(&dom_command.selector)?;
            Ok(format!(
                "Agent {} ({:?}): Text from element '{}': {}",
                selected_agent.id, selected_agent.role, dom_command.selector, text
            ))
        }
        DomCommandAction::GetValue => {
            let value = dom_utils::get_element_value(&dom_command.selector)?;
            Ok(format!(
                "Agent {} ({:?}): Value from element '{}': {}",
                selected_agent.id, selected_agent.role, dom_command.selector, value
            ))
        }
        DomCommandAction::GetAttribute => {
            let attribute_name = dom_command.attribute_name.as_deref().ok_or_else(|| {
                AgentError::CommandParseError(
                    "GETATTRIBUTE command requires attribute name".to_string(),
                )
            })?;
            let value = dom_utils::get_element_attribute(&dom_command.selector, attribute_name)?;
            Ok(format!(
                "Agent {} ({:?}): Attribute '{}' from element '{}': {}",
                selected_agent.id,
                selected_agent.role,
                attribute_name,
                dom_command.selector,
                value
            ))
        }
        DomCommandAction::SetAttribute => {
            let attribute_name = dom_command.attribute_name.as_deref().ok_or_else(|| {
                AgentError::CommandParseError(
                    "SETATTRIBUTE command requires attribute name".to_string(),
                )
            })?;
            let attribute_value = dom_command.value.as_deref().ok_or_else(|| {
                AgentError::CommandParseError(
                    "SETATTRIBUTE command requires attribute value".to_string(),
                )
            })?;
            dom_utils::set_element_attribute(
                &dom_command.selector,
                attribute_name,
                attribute_value,
            )?;
            Ok(format!(
                "Agent {} ({:?}): Successfully set attribute '{}' to '{}' for element '{}'",
                selected_agent.id,
                selected_agent.role,
                attribute_name,
                attribute_value,
                dom_command.selector
            ))
        }
        DomCommandAction::SelectOption => {
            let value = dom_command.value.as_deref().ok_or_else(|| {
                AgentError::CommandParseError("SELECTOPTION command requires option value".to_string())
            })?;
            dom_utils::select_dropdown_option(&dom_command.selector, value)?;
            Ok(format!(
                "Agent {} ({:?}): Successfully selected option '{}' for dropdown '{}'",
                selected_agent.id, selected_agent.role, value, dom_command.selector
            ))
        }
        DomCommandAction::GetAllAttributes => {
            let attribute_name = dom_command.attribute_name.as_deref().ok_or_else(|| {
                AgentError::CommandParseError(
                    "GET_ALL_ATTRIBUTES command requires attribute name".to_string(),
                )
            })?;
            let json_string =
                dom_utils::get_all_elements_attributes(&dom_command.selector, attribute_name)?;
            Ok(format!(
                "Agent {} ({:?}): Successfully retrieved attributes '{}' for elements matching selector '{}': {}",
                selected_agent.id, selected_agent.role, attribute_name, dom_command.selector, json_string
            ))
        }
        DomCommandAction::GetUrl => {
            let url = dom_utils::get_current_url()?;
            Ok(format!(
                "Agent {} ({:?}): Current URL is: {}",
                selected_agent.id, selected_agent.role, url
            ))
        }
        DomCommandAction::ElementExists => {
            let exists = dom_utils::element_exists(&dom_command.selector)?;
            Ok(format!(
                "Agent {} ({:?}): Element '{}' exists: {}",
                selected_agent.id, selected_agent.role, dom_command.selector, exists
            ))
        }
        DomCommandAction::WaitForElement => {
            let timeout_ms = dom_command.value.as_ref().and_then(|s| s.parse::<u32>().ok());
            dom_utils::wait_for_element(&dom_command.selector, timeout_ms).await?;
            Ok(format!(
                "Agent {} ({:?}): Element '{}' appeared.",
                selected_agent.id, selected_agent.role, dom_command.selector
            ))
        }
        DomCommandAction::IsVisible => {
            let visible = dom_utils::is_visible(&dom_command.selector)?;
            Ok(format!(
                "Agent {} ({:?}): Element '{}' is visible: {}",
                selected_agent.id, selected_agent.role, dom_command.selector, visible
            ))
        }
        DomCommandAction::ScrollTo => {
            dom_utils::scroll_to(&dom_command.selector)?;
            Ok(format!(
                "Agent {} ({:?}): Successfully scrolled to element '{}'",
                selected_agent.id, selected_agent.role, dom_command.selector
            ))
        }
        DomCommandAction::Hover => {
            dom_utils::hover_element(&dom_command.selector)?;
            Ok(format!(
                "Agent {} ({:?}): Successfully hovered over element '{}'",
                selected_agent.id, selected_agent.role, dom_command.selector
            ))
        }
        DomCommandAction::GetAllText => {
            let separator = dom_command.value.as_deref().unwrap_or("\n"); // Default to newline if not provided
            let text_content = dom_utils::get_all_text_from_elements(&dom_command.selector, separator)?;
            Ok(format!(
                "Agent {} ({:?}): Retrieved text from elements matching '{}' (separated by '{}'): \"{}\"",
                selected_agent.id, selected_agent.role, dom_command.selector, separator.replace("\n", "\\n"), text_content
            ))
        }
    }
}

// Private helper function for executing a list of LLM-derived commands
async fn execute_llm_commands(
    selected_agent: &Agent,
    command_array: &[serde_json::Value],
) -> Result<String, AgentError> {
    let mut results: Vec<Result<String, String>> = Vec::new();

    console::log_1(
        &format!(
            "Agent {} ({:?}): LLM returned {} commands. Executing...",
            selected_agent.id,
            selected_agent.role,
            command_array.len()
        )
        .into(),
    );

    for (index, cmd_json_obj) in command_array.iter().enumerate() {
        match serde_json::from_value::<LlmDomCommandRequest>(cmd_json_obj.clone()) {
            Ok(llm_cmd_req) => {
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
                    "GET_URL" => DomCommandAction::GetUrl,
                    "ELEMENT_EXISTS" => DomCommandAction::ElementExists,
                    "WAIT_FOR_ELEMENT" => DomCommandAction::WaitForElement,
                    "IS_VISIBLE" => DomCommandAction::IsVisible,
                    "SCROLL_TO" => DomCommandAction::ScrollTo,
                    "HOVER" => DomCommandAction::Hover,
                    "GET_ALL_TEXT" => DomCommandAction::GetAllText,
                    _ => {
                        let err_msg = format!(
                            "Invalid action '{}' from LLM at index {}.",
                            llm_cmd_req.action, index
                        );
                        console::warn_1(&err_msg.clone().into());
                        results.push(Err(err_msg));
                        continue;
                    }
                };

                let validation_error: Option<String> = match dom_action {
                    DomCommandAction::Type
                    | DomCommandAction::SetAttribute
                    | DomCommandAction::SelectOption => {
                        if llm_cmd_req.value.is_none() {
                            Some(format!(
                                "Action {:?} requires 'value'. Command index: {}. Request: {:?}",
                                dom_action, index, llm_cmd_req
                            ))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(err_msg) = validation_error {
                    console::warn_1(&err_msg.clone().into());
                    results.push(Err(err_msg));
                    continue;
                }

                let validation_error_attr: Option<String> = match dom_action {
                    DomCommandAction::GetAttribute
                    | DomCommandAction::SetAttribute
                    | DomCommandAction::GetAllAttributes => {
                        if llm_cmd_req.attribute_name.is_none() {
                            Some(format!("Action {:?} requires 'attribute_name'. Command index: {}. Request: {:?}", dom_action, index, llm_cmd_req))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(err_msg) = validation_error_attr {
                    console::warn_1(&err_msg.clone().into());
                    results.push(Err(err_msg));
                    continue;
                }

                let dom_command = DomCommand {
                    action: dom_action,
                    selector: llm_cmd_req.selector,
                    value: llm_cmd_req.value,
                    attribute_name: llm_cmd_req.attribute_name,
                };

                let cmd_representation = format!(
                    "Action: {:?}, Selector: '{}', Value: {:?}, AttrName: {:?}",
                    dom_command.action,
                    dom_command.selector,
                    dom_command.value,
                    dom_command.attribute_name
                );

                let cmd_result_str: Result<String, String> = match &dom_command.action {
                    DomCommandAction::Click => dom_utils::click_element(&dom_command.selector)
                        .map(|_| {
                            format!(
                                "Successfully clicked element with selector: '{}'",
                                dom_command.selector
                            )
                        })
                        .map_err(|e| {
                            format!("Command {} ('{}') failed: {}", index, cmd_representation, e)
                        }),
                    DomCommandAction::Type => {
                        let text_to_type = dom_command.value.as_deref().unwrap_or_default();
                        dom_utils::type_in_element(&dom_command.selector, text_to_type)
                            .map(|_| {
                                format!(
                                    "Successfully typed '{}' in element with selector: '{}'",
                                    text_to_type, dom_command.selector
                                )
                            })
                            .map_err(|e| {
                                format!("Command {} ('{}') failed: {}", index, cmd_representation, e)
                            })
                    }
                    DomCommandAction::Read => dom_utils::get_element_text(&dom_command.selector)
                        .map(|text| format!("Text from element '{}': {}", dom_command.selector, text))
                        .map_err(|e| {
                            format!("Command {} ('{}') failed: {}", index, cmd_representation, e)
                        }),
                    DomCommandAction::GetValue => {
                        dom_utils::get_element_value(&dom_command.selector)
                            .map(|value| {
                                format!("Value from element '{}': {}", dom_command.selector, value)
                            })
                            .map_err(|e| {
                                format!("Command {} ('{}') failed: {}", index, cmd_representation, e)
                            })
                    }
                    DomCommandAction::GetAttribute => {
                        let attribute_name =
                            dom_command.attribute_name.as_deref().unwrap_or_default();
                        dom_utils::get_element_attribute(&dom_command.selector, attribute_name)
                            .map(|value| {
                                format!(
                                    "Attribute '{}' from element '{}': {}",
                                    attribute_name, dom_command.selector, value
                                )
                            })
                            .map_err(|e| {
                                format!("Command {} ('{}') failed: {}", index, cmd_representation, e)
                            })
                    }
                    DomCommandAction::SetAttribute => {
                        let attribute_name =
                            dom_command.attribute_name.as_deref().unwrap_or_default();
                        let attribute_value = dom_command.value.as_deref().unwrap_or_default();
                        dom_utils::set_element_attribute(
                            &dom_command.selector,
                            attribute_name,
                            attribute_value,
                        )
                        .map(|_| {
                            format!(
                                "Successfully set attribute '{}' to '{}' for element '{}'",
                                attribute_name, attribute_value, dom_command.selector
                            )
                        })
                        .map_err(|e| {
                            format!("Command {} ('{}') failed: {}", index, cmd_representation, e)
                        })
                    }
                    DomCommandAction::SelectOption => {
                        let value = dom_command.value.as_deref().unwrap_or_default();
                        dom_utils::select_dropdown_option(&dom_command.selector, value)
                            .map(|_| {
                                format!(
                                    "Successfully selected option '{}' for dropdown '{}'",
                                    value, dom_command.selector
                                )
                            })
                            .map_err(|e| {
                                format!("Command {} ('{}') failed: {}", index, cmd_representation, e)
                            })
                    }
                    DomCommandAction::GetAllAttributes => {
                        let attribute_name =
                            dom_command.attribute_name.as_deref().unwrap_or_default();
                        dom_utils::get_all_elements_attributes(
                            &dom_command.selector,
                            attribute_name,
                        )
                        .map(|json_string| {
                            format!(
                                "Successfully retrieved attributes '{}' for elements matching selector '{}': {}",
                                attribute_name, dom_command.selector, json_string
                            )
                        })
                        .map_err(|e| {
                            format!("Command {} ('{}') failed: {}", index, cmd_representation, e)
                        })
                    }
                    DomCommandAction::GetUrl => dom_utils::get_current_url()
                        .map(|url| format!("Current URL is: {}", url))
                        .map_err(|e| {
                            format!("Command {} ('{}') failed: {}", index, cmd_representation, e)
                        }),
                    DomCommandAction::ElementExists => {
                        dom_utils::element_exists(&dom_command.selector)
                            .map(|exists| {
                                format!("Element '{}' exists: {}", dom_command.selector, exists)
                            })
                            .map_err(|e| {
                                format!("Command {} ('{}') failed: {}", index, cmd_representation, e)
                            })
                    }
                    DomCommandAction::WaitForElement => {
                        let timeout_ms =
                            dom_command.value.as_ref().and_then(|s| s.parse::<u32>().ok());
                        match dom_utils::wait_for_element(&dom_command.selector, timeout_ms).await
                        {
                            Ok(()) => Ok(format!("Element '{}' appeared.", dom_command.selector)),
                            Err(e) => Err(format!(
                                "Command {} ('{}') failed: {}",
                                index, cmd_representation, e
                            )),
                        }
                    }
                    DomCommandAction::IsVisible => {
                        dom_utils::is_visible(&dom_command.selector)
                            .map(|visible| {
                                format!("Element '{}' is visible: {}", dom_command.selector, visible)
                            })
                            .map_err(|e| {
                                format!("Command {} ('{}') failed: {}", index, cmd_representation, e)
                            })
                    }
                    DomCommandAction::ScrollTo => dom_utils::scroll_to(&dom_command.selector)
                        .map(|_| {
                            format!(
                                "Successfully scrolled to element '{}'",
                                dom_command.selector
                            )
                        })
                        .map_err(|e| {
                            format!("Command {} ('{}') failed: {}", index, cmd_representation, e)
                        }),
                        DomCommandAction::Hover => dom_utils::hover_element(&dom_command.selector)
                            .map(|_| {
                                format!(
                                    "Successfully hovered over element '{}'",
                                    dom_command.selector
                                )
                            })
                            .map_err(|e| {
                                format!("Command {} ('{}') failed: {}", index, cmd_representation, e)
                            }),
                        DomCommandAction::GetAllText => {
                            let separator = dom_command.value.as_deref().unwrap_or("\n");
                            dom_utils::get_all_text_from_elements(&dom_command.selector, separator)
                                .map(|text_content| {
                                    format!(
                                        "Retrieved text from elements matching '{}' (separated by '{}'): \"{}\"",
                                        dom_command.selector, separator.replace("\n", "\\n"), text_content
                                    )
                                })
                                .map_err(|e| {
                                    format!("Command {} ('{}') failed: {}", index, cmd_representation, e)
                                })
                        }
                };
                results.push(cmd_result_str);
            }
            Err(e) => {
                let err_msg = format!(
                    "Command at index {} was malformed and could not be parsed: {}. Object: {}",
                    index, e, cmd_json_obj
                );
                console::warn_1(&err_msg.clone().into());
                results.push(Err(err_msg));
            }
        }
    }
    serde_json::to_string(&results)
        .map_err(|e| AgentError::SerializationError(format!("Error serializing LLM command results: {}", e)))
}

// Private helper function for LLM interaction and response processing
async fn handle_llm_task(
    selected_agent: &Agent,
    task: &str,
    api_key: &str,
    api_url: &str,
    model_name: &str,
) -> Result<String, AgentError> {
    console::log_1(
        &format!(
            "Agent {} ({:?}): No direct DOM command parsed. Defaulting to LLM for task: {}",
            selected_agent.id, selected_agent.role, task
        )
        .into(),
    );

    let prompt_for_llm = generate_structured_llm_prompt(
        selected_agent.id,
        &selected_agent.role,
        task,
        &AVAILABLE_DOM_COMMANDS,
    );

    match call_llm_async(
        prompt_for_llm,
        api_key.to_string(),
        api_url.to_string(),
        model_name.to_string(),
    )
    .await
    {
        Ok(llm_response) => {
            match serde_json::from_str::<serde_json::Value>(&llm_response) {
                Ok(json_value) => {
                    if json_value.is_array() {
                        let command_array = json_value.as_array().ok_or_else(|| {
                            AgentError::InvalidLlmResponse(
                                "LLM response is JSON but not an array.".to_string(),
                            )
                        })?;

                        if command_array.is_empty() {
                            console::log_1(
                                &format!(
                                    "Agent {} ({:?}): LLM returned an empty command array. Treating as natural language response: {}",
                                    selected_agent.id, selected_agent.role, llm_response
                                )
                                .into(),
                            );
                            return Ok(format!(
                                "Agent {} ({:?}) completed task via LLM: {}",
                                selected_agent.id, selected_agent.role, llm_response
                            ));
                        }
                        execute_llm_commands(selected_agent, command_array).await
                    } else {
                        console::log_1(
                            &format!(
                                "Agent {} ({:?}): LLM response was valid JSON but not an array. Treating as natural language: {}",
                                selected_agent.id, selected_agent.role, llm_response
                            )
                            .into(),
                        );
                        Ok(format!(
                            "Agent {} ({:?}) completed task via LLM: {}",
                            selected_agent.id, selected_agent.role, llm_response
                        ))
                    }
                }
                Err(e) => {
                    let trimmed_response = llm_response.trim();
                    if trimmed_response.starts_with('{') || trimmed_response.starts_with('[') {
                        Err(AgentError::InvalidLlmResponse(format!(
                            "LLM response started like JSON but failed to parse: {}. Error: {}",
                            llm_response, e
                        )))
                    } else {
                        console::log_1(
                            &format!(
                                "Agent {} ({:?}): LLM response was not JSON (Error: {}). Treating as natural language: {}",
                                selected_agent.id, selected_agent.role, e, llm_response
                            )
                            .into(),
                        );
                        Ok(format!(
                            "Agent {} ({:?}) completed task via LLM: {}",
                            selected_agent.id, selected_agent.role, llm_response
                        ))
                    }
                }
            }
        }
        Err(js_err) => Err(AgentError::LlmCallFailed(
            js_err.as_string().unwrap_or_else(|| "Unknown LLM error".to_string()),
        )),
    }
}


impl AgentSystem {
    /// Creates a new `AgentSystem` and initializes a predefined set of agents
    /// with different roles (Navigator, FormFiller, Generic).
    pub fn new() -> Self {
        let agents = vec![
            Agent {
                id: 1,
                role: AgentRole::Navigator,
                keywords: vec!["navigate".to_string(), "go to".to_string(), "url".to_string(), "open".to_string()],
                priority: 10,
            },
            Agent {
                id: 2,
                role: AgentRole::FormFiller,
                keywords: vec!["fill".to_string(), "type".to_string(), "input".to_string(), "form".to_string(), "enter".to_string(), "select".to_string()],
                priority: 10,
            },
            Agent {
                id: 3,
                role: AgentRole::Generic,
                keywords: vec![], // Generic agent has no specific keywords by default
                priority: 0,     // Lowest priority
            },
        ];
        AgentSystem { agents }
    }

    /// Runs a given task, either by parsing it as a direct DOM command or by
    /// sending it to an LLM for interpretation into DOM commands or a natural language response.
    pub async fn run_task(
        &self,
        task: &str,
        api_key: &str,
        api_url: &str,
        model_name: &str,
    ) -> Result<String, AgentError> {
        let task_lowercase = task.to_lowercase();
        let mut matching_agents: Vec<&Agent> = self
            .agents
            .iter()
            .filter(|agent| {
                agent.keywords.iter().any(|keyword| task_lowercase.contains(keyword))
            })
            .collect();

        let selected_agent: &Agent;

        if matching_agents.is_empty() {
            // Default to Generic agent if no keywords match
            selected_agent = self.agents.iter()
                .find(|a| a.role == AgentRole::Generic)
                .unwrap_or_else(|| {
                    console::warn_1(&"Generic agent not found, defaulting to first agent in list.".into());
                    &self.agents[0] // Should always find Generic, but as a robust fallback
                });
        } else {
            // Sort matching agents by priority (descending)
            matching_agents.sort_by(|a, b| b.priority.cmp(&a.priority));
            
            let highest_priority = matching_agents[0].priority;
            let top_priority_agents: Vec<&&Agent> = matching_agents
                .iter()
                .filter(|a| a.priority == highest_priority)
                .collect();

            if top_priority_agents.len() == 1 {
                selected_agent = top_priority_agents[0];
            } else {
                // Tie-breaking: if Generic is not among the tied, prefer the first specialized one.
                // If Generic is among the tied, and there's another specialized one, prefer specialized.
                // If all tied are specialized, or all tied are Generic (or only Generic is tied), pick the first one encountered.
                if let Some(non_generic_tied_agent) = top_priority_agents.iter().find(|a| a.role != AgentRole::Generic) {
                    selected_agent = non_generic_tied_agent;
                } else {
                    // All tied agents are Generic, or only Generic was tied.
                    // Or, all tied agents are specialized (non-Generic) - pick the first from sorted list.
                    selected_agent = top_priority_agents[0];
                }
            }
        }

        console::log_1(
            &format!(
                "Task received: '{}'. Selected Agent ID: {}, Role: {:?}, Priority: {}",
                task, selected_agent.id, selected_agent.role, selected_agent.priority
            )
            .into(),
        );

        if let Some(dom_command) = parse_dom_command(task) {
            execute_direct_dom_command(selected_agent, &dom_command).await
        } else {
            handle_llm_task(selected_agent, task, api_key, api_url, model_name).await
        }
    }
}

// #[cfg(test)] attribute will be applied to the entire module below
#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*; // For async tests in WASM
    use crate::dom_utils::DomError; // Make sure DomError is in scope for tests
    wasm_bindgen_test_configure!(run_in_browser); // Allows tests to run in a browser-like environment

    // Helper to assert AgentError equality, focusing on variants and key parts of messages
    fn assert_agent_error_variant(result: Result<String, AgentError>, expected_variant: AgentError) {
        match result {
            Ok(s) => panic!("Expected AgentError {:?}, but got Ok({})", expected_variant, s),
            Err(e) => match (e, expected_variant) {
                (AgentError::DomOperationFailed(actual_dom_err), AgentError::DomOperationFailed(expected_dom_err)) => {
                    // For DomError, we might only check the variant or a substring of the message
                    // due to potential complexities in full DomError equality (e.g. JsValue inside)
                    assert_eq!(std::mem::discriminant(&actual_dom_err), std::mem::discriminant(&expected_dom_err));
                    // Example: if expected_dom_err has a selector, check if actual_dom_err's string form contains it
                    // This is a basic check; more specific checks might be needed depending on DomError structure
                    if let DomError::ElementNotFound { selector: expected_selector, .. } = expected_dom_err {
                         assert!(actual_dom_err.to_string().contains(&expected_selector));
                    }
                    // Add more specific checks for other DomError variants if necessary
                }
                (AgentError::LlmCallFailed(actual_msg), AgentError::LlmCallFailed(expected_msg)) => {
                    assert!(actual_msg.contains(&expected_msg), "LlmCallFailed message mismatch. Actual: '{}', Expected to contain: '{}'", actual_msg, expected_msg);
                }
                (AgentError::InvalidLlmResponse(actual_msg), AgentError::InvalidLlmResponse(expected_msg)) => {
                    assert!(actual_msg.contains(&expected_msg), "InvalidLlmResponse message mismatch. Actual: '{}', Expected to contain: '{}'", actual_msg, expected_msg);
                }
                (AgentError::CommandParseError(actual_msg), AgentError::CommandParseError(expected_msg)) => {
                    assert!(actual_msg.contains(&expected_msg), "CommandParseError message mismatch. Actual: '{}', Expected to contain: '{}'", actual_msg, expected_msg);
                }
                (AgentError::SerializationError(actual_msg), AgentError::SerializationError(expected_msg)) => {
                    assert!(actual_msg.contains(&expected_msg), "SerializationError message mismatch. Actual: '{}', Expected to contain: '{}'", actual_msg, expected_msg);
                }
                (actual, expected) => panic!("AgentError variant mismatch. Actual: {:?}, Expected: {:?}", actual, expected),
            },
        }
    }


    #[test]
    fn test_parse_dom_command_get_url() {
        let cmd = parse_dom_command("GET_URL").expect("GET_URL should parse");
        assert_eq!(cmd.action, DomCommandAction::GetUrl);
        assert_eq!(cmd.selector, ""); // Selector is not used

        // With unexpected args (should be ignored by parser, logged by GET_URL itself if needed)
        let cmd_with_args = parse_dom_command("GET_URL some_arg").expect("GET_URL with args should parse");
        assert_eq!(cmd_with_args.action, DomCommandAction::GetUrl);
        assert_eq!(cmd_with_args.selector, ""); // Selector is not used
    }

    #[test]
    fn test_parse_dom_command_element_exists() {
        let cmd = parse_dom_command("ELEMENT_EXISTS css:#myId").expect("ELEMENT_EXISTS should parse");
        assert_eq!(cmd.action, DomCommandAction::ElementExists);
        assert_eq!(cmd.selector, "css:#myId");

        assert!(parse_dom_command("ELEMENT_EXISTS").is_none(), "ELEMENT_EXISTS should require a selector");
    }

    #[test]
    fn test_parse_dom_command_wait_for_element() {
        let cmd_no_timeout = parse_dom_command("WAIT_FOR_ELEMENT css:#myId").expect("WAIT_FOR_ELEMENT no timeout should parse");
        assert_eq!(cmd_no_timeout.action, DomCommandAction::WaitForElement);
        assert_eq!(cmd_no_timeout.selector, "css:#myId");
        assert_eq!(cmd_no_timeout.value, None);

        let cmd_with_timeout = parse_dom_command("WAIT_FOR_ELEMENT xpath://div 1000").expect("WAIT_FOR_ELEMENT with timeout should parse");
        assert_eq!(cmd_with_timeout.action, DomCommandAction::WaitForElement);
        assert_eq!(cmd_with_timeout.selector, "xpath://div");
        assert_eq!(cmd_with_timeout.value, Some("1000".to_string()));

        assert!(parse_dom_command("WAIT_FOR_ELEMENT").is_none(), "WAIT_FOR_ELEMENT should require a selector");

        let cmd_invalid_timeout = parse_dom_command("WAIT_FOR_ELEMENT css:#myId abc").expect("WAIT_FOR_ELEMENT invalid timeout should parse");
        assert_eq!(cmd_invalid_timeout.action, DomCommandAction::WaitForElement);
        assert_eq!(cmd_invalid_timeout.selector, "css:#myId");
        assert_eq!(cmd_invalid_timeout.value, None); // Invalid timeout 'abc' results in None
    }

    #[test]
    fn test_parse_dom_command_is_visible() {
        let cmd = parse_dom_command("IS_VISIBLE css:#myId").expect("IS_VISIBLE should parse");
        assert_eq!(cmd.action, DomCommandAction::IsVisible);
        assert_eq!(cmd.selector, "css:#myId");
        assert!(parse_dom_command("IS_VISIBLE").is_none(), "IS_VISIBLE should require a selector");
    }

    #[test]
    fn test_parse_dom_command_scroll_to() {
        let cmd = parse_dom_command("SCROLL_TO css:#myId").expect("SCROLL_TO should parse");
        assert_eq!(cmd.action, DomCommandAction::ScrollTo);
        assert_eq!(cmd.selector, "css:#myId");
        assert!(parse_dom_command("SCROLL_TO").is_none(), "SCROLL_TO should require a selector");
    }

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

    #[test]
    fn test_generate_structured_llm_prompt_includes_new_commands() {
        let prompt = generate_structured_llm_prompt(1, &AgentRole::Generic, "test task", &AVAILABLE_DOM_COMMANDS);

        // Check for GET_URL
        assert!(prompt.contains("\"action\": \"GET_URL\""));
        assert!(prompt.contains("- Get URL: {{\"action\": \"GET_URL\"}} (gets the current page URL)"));

        // Check for ELEMENT_EXISTS
        assert!(prompt.contains("\"action\": \"ELEMENT_EXISTS\""));
        assert!(prompt.contains("- Element Exists: {{\"action\": \"ELEMENT_EXISTS\", \"selector\": \"<selector>\"}} (checks if an element exists on the page, returns true or false)"));
        
        // Check for WAIT_FOR_ELEMENT
        assert!(prompt.contains("\"action\": \"WAIT_FOR_ELEMENT\""));
        assert!(prompt.contains("- Wait For Element: {{\"action\": \"WAIT_FOR_ELEMENT\", \"selector\": \"<selector>\", \"value\": <timeout_in_milliseconds_optional>}} (waits for an element to exist, returns nothing on success or error on timeout/failure)"));

        // Check for IS_VISIBLE
        assert!(prompt.contains("\"action\": \"IS_VISIBLE\""));
        assert!(prompt.contains("- Is Visible: {{\"action\": \"IS_VISIBLE\", \"selector\": \"<selector>\"}} (checks if an element is currently visible on the page, returns true or false)"));

        // Check for SCROLL_TO
        assert!(prompt.contains("\"action\": \"SCROLL_TO\""));
        assert!(prompt.contains("- Scroll To: {{\"action\": \"SCROLL_TO\", \"selector\": \"<selector>\"}} (scrolls the page to make the element visible)"));
    }

    #[wasm_bindgen_test]
    async fn test_run_task_agent_selection_and_dom_command_format() {
        let agent_system = AgentSystem::new();
        let dummy_api_key = "test_api_key";
        let dummy_api_url = "http://localhost/dummy_url_if_network_active";
        let dummy_model_name = "dummy_model";

        // Task: "CLICK #myButton" - No specific keywords, should use Generic Agent (ID 3)
        let task_click_default_css = "CLICK #myButton";
        let res_click_default_css = agent_system.run_task(task_click_default_css, dummy_api_key, dummy_api_url, dummy_model_name).await;
        let err_msg_click_default = res_click_default_css.expect_err("Expected error for CLICK #myButton");
        assert!(err_msg_click_default.to_string().contains("DOM Operation Failed: ElementNotFound: No element found for selector '#myButton'"), "Error message: {}", err_msg_click_default);
        // We check the selected agent by looking at the console log through other tests, or by trusting the logic.
        // For direct DOM commands, the agent info is in the Ok() part, which we don't hit here.
        // For this test, we're primarily ensuring the DOM logic still works. Agent selection for direct commands is implicitly Generic if no keywords.

        // Task: "TYPE css:#userCss an_email@example.com" - "type" keyword matches FormFiller (ID 2)
        let task_type_css = "TYPE css:#userCss an_email@example.com";
        let res_type_css = agent_system.run_task(task_type_css, dummy_api_key, dummy_api_url, dummy_model_name).await;
        let err_msg_type_css = res_type_css.expect_err("Expected error for TYPE css:#userCss");
        assert!(err_msg_type_css.to_string().contains("DOM Operation Failed: ElementNotFound: No element found for selector 'css:#userCss'"), "Error message: {}", err_msg_type_css);
        // If execute_direct_dom_command included agent info in its error (it does in Ok), we could check Agent 2.
        // For now, we assume if it's a DOM error, the command reached that stage.
        // The actual agent selection logging "Selected Agent ID: 2, Role: FormFiller" would confirm.

        // Task: "GET_URL" - "url" keyword matches Navigator (ID 1)
        let task_get_url = "GET_URL"; // "url" is a Navigator keyword.
        let res_get_url = agent_system.run_task(task_get_url, dummy_api_key, dummy_api_url, dummy_model_name).await;
        let url_response = res_get_url.expect("GET_URL should succeed");
        assert!(url_response.contains("Agent 1 (Navigator): Current URL is:"), "GET_URL response format error: {}", url_response);


        // Task: "READ xpath://div" - No keywords for specialized agents, should use Generic.
        let task_read_xpath = "READ xpath://div[@id='messageXpath']";
        let res_read_xpath = agent_system.run_task(task_read_xpath, dummy_api_key, dummy_api_url, dummy_model_name).await;
        let err_msg_read_xpath = res_read_xpath.expect_err("Expected error for READ");
        assert!(err_msg_read_xpath.to_string().contains("DOM Operation Failed: ElementNotFound: No element found for selector 'xpath://div[@id='messageXpath']'"), "Error message: {}", err_msg_read_xpath);
        // Expected log: "Selected Agent ID: 3, Role: Generic"
    }


    #[wasm_bindgen_test]
    async fn test_run_task_llm_fallback_agent_selection() {
        let agent_system = AgentSystem::new();
        let dummy_api_key = "test_api_key_llm_will_fail_network";
        let dummy_api_url = "http://localhost:12345/nonexistent_endpoint"; // Ensure network call fails
        let dummy_model_name = "dummy_model_llm";

        // Task for Navigator (LLM fallback) - "navigate" keyword
        let task_nav = "navigate to example.com";
        let result_nav = agent_system.run_task(task_nav, dummy_api_key, dummy_api_url, dummy_model_name).await;
        #[cfg(feature = "mock-llm")]
        {
            let response_text = result_nav.expect("LLM fallback for NAV should be Ok with mock");
            assert!(response_text.contains("Agent 1 (Navigator) completed task via LLM: Mocked LLM response for 'navigate to example.com'"), "Unexpected mock response for NAV: {}", response_text);
        }
        #[cfg(not(feature = "mock-llm"))]
        {
            // Expect LlmCallFailed, agent info is part of the error display for LlmCallFailed
            let err = result_nav.expect_err("LLM fallback for NAV should error without mock");
            assert!(err.to_string().contains("LLM Call Failed"), "Error type mismatch for NAV: {}", err);
            // The agent info is not directly in LlmCallFailed string, but selection happened.
            // We rely on console log or more specific tests for exact selection proof if error occurs before agent info is in string.
        }

        // Task for FormFiller (LLM fallback) - "fill", "form" keywords
        let task_form = "fill the login form with my details";
        let result_form = agent_system.run_task(task_form, dummy_api_key, dummy_api_url, dummy_model_name).await;
        #[cfg(feature = "mock-llm")]
        {
            let response_text = result_form.expect("LLM fallback for FORM should be Ok with mock");
            assert!(response_text.contains("Agent 2 (FormFiller) completed task via LLM: Mocked LLM response for 'fill the login form with my details'"), "Unexpected mock response for FORM: {}", response_text);
        }
        #[cfg(not(feature = "mock-llm"))]
        {
            let err = result_form.expect_err("LLM fallback for FORM should error without mock");
            assert!(err.to_string().contains("LLM Call Failed"), "Error type mismatch for FORM: {}", err);
        }

        // Task for Generic (LLM fallback) - no specific keywords
        let task_generic = "summarize this document for me";
        let result_generic = agent_system.run_task(task_generic, dummy_api_key, dummy_api_url, dummy_model_name).await;
        #[cfg(feature = "mock-llm")]
        {
            let response_text = result_generic.expect("LLM fallback for GENERIC should be Ok with mock");
            assert!(response_text.contains("Agent 3 (Generic) completed task via LLM: Mocked LLM response for 'summarize this document for me'"), "Unexpected mock response for GENERIC: {}", response_text);
        }
        #[cfg(not(feature = "mock-llm"))]
        {
            let err = result_generic.expect_err("LLM fallback for GENERIC should error without mock");
            assert!(err.to_string().contains("LLM Call Failed"), "Error type mismatch for GENERIC: {}", err);
        }
    }

    #[wasm_bindgen_test]
    async fn test_new_agent_selection_logic() {
        let agent_system = AgentSystem::new();
        let dummy_api_key = "test_key";
        let dummy_api_url = "mock_url";
        let dummy_model_name = "mock_model";

        // Scenario 1: Navigator specific task
        let task_nav = "open example.com url"; // LLM fallback
        let result_nav = agent_system.run_task(task_nav, dummy_api_key, dummy_api_url, dummy_model_name).await;
        #[cfg(feature = "mock-llm")] {
            assert!(result_nav.unwrap().contains("Agent 1 (Navigator) completed task via LLM"));
        } #[cfg(not(feature = "mock-llm"))] {
            assert_agent_error_variant(result_nav, AgentError::LlmCallFailed("NetworkError".to_string()));
        }


        // Scenario 2: FormFiller specific task
        let task_form = "enter 'test' into the input field"; // LLM fallback
        let result_form = agent_system.run_task(task_form, dummy_api_key, dummy_api_url, dummy_model_name).await;
         #[cfg(feature = "mock-llm")] {
            assert!(result_form.unwrap().contains("Agent 2 (FormFiller) completed task via LLM"));
        } #[cfg(not(feature = "mock-llm"))] {
            assert_agent_error_variant(result_form, AgentError::LlmCallFailed("NetworkError".to_string()));
        }

        // Scenario 3: Generic task (no keywords)
        let task_generic = "tell me a joke"; // LLM fallback
        let result_generic = agent_system.run_task(task_generic, dummy_api_key, dummy_api_url, dummy_model_name).await;
        #[cfg(feature = "mock-llm")] {
            assert!(result_generic.unwrap().contains("Agent 3 (Generic) completed task via LLM"));
        } #[cfg(not(feature = "mock-llm"))] {
            assert_agent_error_variant(result_generic, AgentError::LlmCallFailed("NetworkError".to_string()));
        }

        // Scenario 4: Keyword Tie (Navigator & FormFiller, same priority)
        // "go to" -> Navigator, "type" -> FormFiller. Navigator is defined first.
        let task_tie = "go to the login form and type credentials"; // LLM fallback
        let result_tie = agent_system.run_task(task_tie, dummy_api_key, dummy_api_url, dummy_model_name).await;
        #[cfg(feature = "mock-llm")] {
            assert!(result_tie.unwrap().contains("Agent 1 (Navigator) completed task via LLM"));
        } #[cfg(not(feature = "mock-llm"))] {
            assert_agent_error_variant(result_tie, AgentError::LlmCallFailed("NetworkError".to_string()));
        }

        // Scenario 5: Direct DOM command with specific agent keywords
        // "TYPE" is a FormFiller keyword. "navigate" is Navigator. FormFiller should be chosen due to "TYPE" keyword.
        // However, the command is "TYPE css:#searchbox navigate to products page"
        // parse_dom_command will parse this as: action=TYPE, selector="css:#searchbox", value="navigate to products page"
        // Agent selection: "type" (FormFiller, P10), "navigate" (Navigator, P10). Tie, Navigator is first.
        // So, Agent 1 (Navigator) will be selected to execute this *direct* DOM command.
        let task_direct_keywords = "TYPE css:#searchbox navigate to products page";
        let result_direct_keywords = agent_system.run_task(task_direct_keywords, dummy_api_key, dummy_api_url, dummy_model_name).await;
        let err_direct = result_direct_keywords.expect_err("Expected error for direct command with keyword conflict");
        // The error message will be from the DOM operation, not an LLM call.
        // The agent responsible for the direct command execution (Agent 1) will be part of the success message if it succeeded.
        // Since it's an error, the AgentError::DomOperationFailed will be returned.
        // The Display impl for AgentError is "DOM Operation Failed: <DomError string>"
        // The DomError string itself doesn't include agent info.
        // To confirm Agent 1 was selected, we would rely on the console log: "Selected Agent ID: 1, Role: Navigator"
        match err_direct {
            AgentError::DomOperationFailed(dom_error) => {
                assert_eq!(dom_error.to_string(), "ElementNotFound: No element found for selector 'css:#searchbox'");
            }
            _ => panic!("Expected DomOperationFailed, got {:?}", err_direct),
        }
    }


    // New tests for LLM JSON response handling (ensure 'mock-llm' feature is active for these)
    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_json_single_valid_command() {
        let agent_system = AgentSystem::new();
        let task = "click the submit button"; // Triggers mock: [{"action": "CLICK", "selector": "css:#submitBtn"}]
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.as_ref().err().map(|e|e.to_string()));
        let result_str = result.unwrap();
        
        assert!(result_str.contains("Command 0 ('Action: Click, Selector: \\'css:#submitBtn\\', Value: None, AttrName: None') failed: DOM Operation Failed: ElementNotFound: No element found for selector 'css:#submitBtn'"), "Result string mismatch: {}", result_str);
        assert!(result_str.starts_with("[") && result_str.ends_with("]"), "Result string is not a JSON array: {}", result_str);

        match serde_json::from_str::<Vec<Result<String, String>>>(&result_str) {
            Ok(outer_array) => {
                assert_eq!(outer_array.len(), 1, "Expected one result in the outer array");
                assert!(outer_array[0].is_err(), "Expected inner result to be an error");
                let inner_error_msg = outer_array[0].as_ref().err().unwrap();
                assert!(inner_error_msg.contains("Command 0 ('Action: Click, Selector: \\'css:#submitBtn\\', Value: None, AttrName: None') failed: DOM Operation Failed: ElementNotFound: No element found for selector 'css:#submitBtn'"), "Inner error message mismatch: {}", inner_error_msg);
            }
            Err(e) => panic!("Failed to parse result_str as JSON array of results: {}, content: {}", e, result_str),
        }
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_json_multiple_valid_commands() {
        let agent_system = AgentSystem::new();
        let task = "login with testuser and click login"; // Triggers mock: [{"action": "TYPE", "selector": "css:#username", "value": "testuser"}, {"action": "CLICK", "selector": "css:#loginBtn"}]
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.as_ref().err().map(|e|e.to_string()));
        let result_str = result.unwrap();
        
        match serde_json::from_str::<Vec<Result<String, String>>>(&result_str) {
            Ok(results) => {
                assert_eq!(results.len(), 2, "Expected two results in the JSON array");
                assert!(results[0].is_err());
                let err_msg1 = results[0].as_ref().err().unwrap();
                assert!(err_msg1.contains("Command 0 ('Action: Type, Selector: \\'css:#username\\', Value: Some(\\\"testuser\\\"), AttrName: None') failed: DOM Operation Failed: ElementNotFound: No element found for selector 'css:#username'"), "Error message for TYPE incorrect: {}", err_msg1);

                assert!(results[1].is_err());
                let err_msg2 = results[1].as_ref().err().unwrap();
                assert!(err_msg2.contains("Command 1 ('Action: Click, Selector: \\'css:#loginBtn\\', Value: None, AttrName: None') failed: DOM Operation Failed: ElementNotFound: No element found for selector 'css:#loginBtn'"), "Error message for CLICK incorrect: {}", err_msg2);
            }
            Err(e) => panic!("Failed to parse result_str as JSON array of results: {}, content: {}", e, result_str),
        }
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_invalid_json_string() {
        let agent_system = AgentSystem::new();
        let task = "task expected to return invalid json"; // Triggers mock: "This is not JSON."
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        // This is now treated as a natural language response by the agent if not starting with { or [
        assert!(result.is_ok(), "Expected Ok for non-JSON string, got: {:?}", result.as_ref().err().map(|e|e.to_string()));
        assert_eq!(result.unwrap(), "Agent 3 (Generic) completed task via LLM: This is not JSON.");
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_malformed_json_string() {
        let agent_system = AgentSystem::new();
        let task = "task expected to return malformed json"; // Triggers mock: "{ \"action\": \"CLICK\", \"selector\": " // Malformed
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert_agent_error_variant(result, AgentError::InvalidLlmResponse("LLM response started like JSON but failed to parse".to_string()));
    }


    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_json_object_not_array() {
        let agent_system = AgentSystem::new();
        let task = "task expected to return json object not array"; // Triggers mock: {"message": "This is a JSON object, not an array."}
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.as_ref().err().map(|e|e.to_string()));
        let expected_response = "Agent 3 (Generic) completed task via LLM: {\"message\": \"This is a JSON object, not an array.\"}";
        assert_eq!(result.unwrap(), expected_response);
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_json_array_malformed_command_object() {
        let agent_system = AgentSystem::new();
        // Triggers mock: [{"foo": "bar"}] - valid JSON array, but object inside is not LlmDomCommandRequest
        let task = "task expected to return json array of non-commands";
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.as_ref().err().map(|e|e.to_string()));
        let result_str = result.unwrap();
        // The result will be a JSON array string containing the error from trying to parse this command
        match serde_json::from_str::<Vec<Result<String, String>>>(&result_str) {
            Ok(results_array) => {
                assert_eq!(results_array.len(), 1);
                assert!(results_array[0].is_err());
                assert!(results_array[0].as_ref().err().unwrap().contains("Command at index 0 was malformed and could not be parsed: missing field `action`"));
            }
            Err(e) => panic!("Result was not a parsable JSON array of results: {}. Content: {}", e, result_str)
        }
    }


    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_json_array_mixed_valid_invalid_commands() {
        let agent_system = AgentSystem::new();
        // Triggers mock: [{"action": "CLICK", "selector": "css:#ok"}, {"action": "INVALID_ACTION", "selector": "css:#bad"}, {"action": "TYPE", "selector": "css:#missingValue"}] (missing value for TYPE)
        let task = "task with mixed valid and invalid commands"; 
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.as_ref().err().map(|e|e.to_string()));
        let result_str = result.unwrap();

        match serde_json::from_str::<Vec<Result<String, String>>>(&result_str) {
            Ok(results) => {
                assert_eq!(results.len(), 3, "Expected three results in the JSON array");

                assert!(results[0].is_err()); // CLICK css:#ok (fails due to no element)
                assert!(results[0].as_ref().err().unwrap().contains("Command 0 ('Action: Click, Selector: \\'css:#ok\\', Value: None, AttrName: None') failed: DOM Operation Failed: ElementNotFound: No element found for selector 'css:#ok'"));
                
                assert!(results[1].is_err()); // INVALID_ACTION
                assert_eq!(results[1].as_ref().err().unwrap(), "Invalid action 'INVALID_ACTION' from LLM at index 1.");
                
                assert!(results[2].is_err()); // TYPE css:#missingValue (fails validation for missing value)
                assert!(results[2].as_ref().err().unwrap().contains("Action Type requires 'value'. Command index: 2. Request: LlmDomCommandRequest { action: \"TYPE\", selector: \"css:#missingValue\", value: None, attribute_name: None }"));
            }
            Err(e) => panic!("Failed to parse result_str as JSON array of results: {}, content: {}", e, result_str),
        }
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_json_empty_array() {
        let agent_system = AgentSystem::new();
        let task = "task expected to return empty command array"; // Triggers mock: []
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.as_ref().err().map(|e|e.to_string()));
        let expected_response = "Agent 3 (Generic) completed task via LLM: []";
        assert_eq!(result.unwrap(), expected_response);
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_get_url() {
        let agent_system = AgentSystem::new();
        let task = "llm_get_url_task"; // Mock in llm.rs returns: [{"action": "GET_URL"}]
        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "LLM GET_URL failed: {:?}", result.as_ref().err().map(|e|e.to_string()));
        let result_str = result.unwrap();
        let results: Vec<Result<String, String>> = serde_json::from_str(&result_str).expect("Failed to parse JSON result array");
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        assert!(results[0].as_ref().unwrap().contains("Current URL is:"));
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_element_exists() {
        let agent_system = AgentSystem::new();
        let (_window, document) = dom_utils::get_window_document().unwrap();
        let el = dom_utils::setup_element(&document, "llm-exists", "div", None);

        let task_exists_true = "llm_element_exists_true_task"; // Mock: [{"action": "ELEMENT_EXISTS", "selector": "css:#llm-exists"}]
        let result_true = agent_system.run_task(task_exists_true, "dummy", "dummy", "dummy").await.unwrap();
        let results_true: Vec<Result<String, String>> = serde_json::from_str(&result_true).unwrap();
        assert_eq!(results_true.len(), 1);
        assert_eq!(results_true[0].as_ref().unwrap(), "Element 'css:#llm-exists' exists: true");
        
        dom_utils::cleanup_element(el);

        let task_exists_false = "llm_element_exists_false_task"; // Mock: [{"action": "ELEMENT_EXISTS", "selector": "css:#llm-nonexistent"}]
        let result_false = agent_system.run_task(task_exists_false, "dummy", "dummy", "dummy").await.unwrap();
        let results_false: Vec<Result<String, String>> = serde_json::from_str(&result_false).unwrap();
        assert_eq!(results_false.len(), 1);
        assert_eq!(results_false[0].as_ref().unwrap(), "Element 'css:#llm-nonexistent' exists: false");
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_wait_for_element() {
        let agent_system = AgentSystem::new();
        let (_window, document) = dom_utils::get_window_document().unwrap();
        
        let el_immediate = dom_utils::setup_element(&document, "llm-wait-immediate", "div", None);
        let task_wait_immediate = "llm_wait_for_element_immediate_task"; // Mock: [{"action": "WAIT_FOR_ELEMENT", "selector": "css:#llm-wait-immediate", "value": "100"}]
        let result_immediate = agent_system.run_task(task_wait_immediate, "dummy", "dummy", "dummy").await.unwrap();
        let results_immediate: Vec<Result<String, String>> = serde_json::from_str(&result_immediate).unwrap();
        assert_eq!(results_immediate.len(), 1);
        assert_eq!(results_immediate[0].as_ref().unwrap(), "Element 'css:#llm-wait-immediate' appeared.");
        dom_utils::cleanup_element(el_immediate);

        let task_wait_timeout = "llm_wait_for_element_timeout_task"; // Mock: [{"action": "WAIT_FOR_ELEMENT", "selector": "css:#llm-wait-timeout", "value": "50"}]
        let result_timeout = agent_system.run_task(task_wait_timeout, "dummy", "dummy", "dummy").await.unwrap();
        let results_timeout: Vec<Result<String, String>> = serde_json::from_str(&result_timeout).unwrap();
        assert_eq!(results_timeout.len(), 1);
        assert!(results_timeout[0].is_err());
        assert!(results_timeout[0].as_ref().err().unwrap().contains("Command 0 ('Action: WaitForElement, Selector: \\'css:#llm-wait-timeout\\', Value: Some(\\\"50\\\"), AttrName: None') failed: DOM Operation Failed: Element 'css:#llm-wait-timeout' not found after 50ms timeout"));
    }

    #[cfg(feature = "mock-llm")]
    #[wasm_bindgen_test]
    async fn test_run_task_llm_json_mixed_validity_commands() {
        let agent_system = AgentSystem::new();
        let task = "task with mixed valid and malformed json commands";
        // Mock response: [{"action": "CLICK", "selector": "css:#valid"}, {"invalid_field": "some_value", "action": "EXTRA_INVALID_FIELD"}, {"action": "TYPE", "selector": "css:#anotherValid", "value": "test"}]

        let result = agent_system.run_task(task, "dummy_key", "dummy_url", "dummy_model").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.as_ref().err().map(|e|e.to_string()));
        let result_str = result.unwrap();

        match serde_json::from_str::<Vec<Result<String, String>>>(&result_str) {
            Ok(results) => {
                assert_eq!(results.len(), 3, "Expected three results in the JSON array");

                assert!(results[0].is_err());
                assert!(results[0].as_ref().err().unwrap().contains("Command 0 ('Action: Click, Selector: \\'css:#valid\\', Value: None, AttrName: None') failed: DOM Operation Failed: ElementNotFound: No element found for selector 'css:#valid'"));

                assert!(results[1].is_err());
                let err_msg_malformed = results[1].as_ref().err().unwrap();
                assert!(err_msg_malformed.contains("Command at index 1 was malformed and could not be parsed:"), "Malformed command error message mismatch: {}", err_msg_malformed);
                assert!(err_msg_malformed.contains("invalid_field"), "Malformed command error did not contain original object snippet: {}", err_msg_malformed);
                assert!(err_msg_malformed.contains("missing field `selector`"), "Malformed command error should mention missing selector: {}", err_msg_malformed);


                assert!(results[2].is_err());
                assert!(results[2].as_ref().err().unwrap().contains("Command 2 ('Action: Type, Selector: \\'css:#anotherValid\\', Value: Some(\\\"test\\\"), AttrName: None') failed: DOM Operation Failed: ElementNotFound: No element found for selector 'css:#anotherValid'"));
            }
            Err(e) => panic!("Failed to parse result_str as JSON array of results: {}, content: {}", e, result_str),
        }
    }
}