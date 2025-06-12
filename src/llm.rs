use wasm_bindgen::prelude::*;
use serde_json::json; // Used by both real and mock
use web_sys::console; // Used by both real and mock

#[cfg(not(feature = "mock-llm"))]
use reqwest::Client; // Only used by the real (non-mock) implementation

/// Calls a Large Language Model (LLM) API with the given prompt.
///
/// This function has two implementations based on the "mock-llm" feature flag:
/// 1.  **Real Implementation (default):** Makes an actual HTTP POST request to the specified LLM API.
///     It constructs a JSON payload with the model name and prompt, sends it, and parses
///     the expected response structure to extract the LLM's content.
/// 2.  **Mock Implementation (`#[cfg(feature = "mock-llm")]`):** Does not make any network requests.
///     Instead, it returns predefined string responses based on keywords found in the `prompt`.
///     This is used for testing to simulate various LLM behaviors predictably and offline.
///
/// # Arguments
/// * `prompt`: The prompt string to send to the LLM.
/// * `api_key`: The API key for authentication with the LLM service. (Ignored if "mock-llm" is enabled).
/// * `api_url`: The URL of the LLM API endpoint. (Ignored if "mock-llm" is enabled).
/// * `model_name`: The specific LLM model to use. (Ignored if "mock-llm" is enabled).
///
/// # Returns
/// * `Ok(String)`: Contains the LLM's response content if the call is successful (or a matching mock is found).
/// * `Err(JsValue)`: Contains an error message if:
///     - (Real) The HTTP request fails (e.g., network error).
///     - (Real) The LLM API returns a non-successful status code.
///     - (Real) The LLM API response cannot be parsed as expected.
///     - (Mock) The prompt triggers a specific mocked error scenario.
#[cfg(not(feature = "mock-llm"))]
#[wasm_bindgen]
pub async fn call_llm_async(prompt: String, api_key: String, api_url: String, model_name: String) -> Result<String, JsValue> {
    console::log_1(&"call_llm_async called (REAL)".into()); // Log that the real function is called

    let client = Client::new(); // Create a new reqwest client
    
    let payload = json!({
        "model": model_name,
        "messages": [
            {
                "role": "user",
                "content": prompt
            }
        ]
    });

    console::log_1(&format!("Payload (REAL): {}", payload.to_string()).into());

    let res = client
        .post(&api_url) // Changed api_url to &api_url
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            console::error_1(&format!("Request error (REAL): {}", e).into());
            JsValue::from_str(&format!("Request error: {}", e.to_string()))
        })?;

    console::log_1(&format!("Response status (REAL): {}", res.status()).into());

    if !res.status().is_success() {
        let error_text = res.text().await.unwrap_or_else(|_| "Failed to get error text".to_string());
        console::error_1(&format!("API error (REAL): {}", error_text).into());
        return Err(JsValue::from_str(&format!("API error: {}", error_text)));
    }

    let response_body: serde_json::Value = res.json().await.map_err(|e| {
        let error_message = format!("JSON parsing error (REAL): {}", e);
        console::error_1(&error_message.clone().into()); // Clone error_message for console
        JsValue::from_str(&error_message)
    })?;

    console::log_1(&format!("Response body (REAL raw): {}", response_body.to_string()).into());

    let content = response_body
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|choices_array| choices_array.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content_value| content_value.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            let error_message = "Failed to extract content from LLM response (REAL): structure was not as expected.";
            console::error_1(&error_message.into());
            console::error_1(&format!("Full response body for debugging (REAL): {}", response_body.to_string()).into());
            JsValue::from_str(error_message)
        })?;

    Ok(content)
}

#[cfg(feature = "mock-llm")]
#[wasm_bindgen]
pub async fn call_llm_async(prompt: String, _api_key: String, _api_url: String, _model_name: String) -> Result<String, JsValue> {
    console::log_1(&format!("call_llm_async called (MOCK) for prompt containing task:\n\"{}\"", extract_task_from_prompt(&prompt)).into());

    // --- Group: Mocks for specific DOM command JSON responses ---
    // These simulate the LLM successfully translating a natural language query into one or more structured DOM commands.
    if prompt.contains("click the submit button") {
        return Ok("[{\"action\": \"CLICK\", \"selector\": \"css:#submitBtn\"}]".to_string());
    } else if prompt.contains("login with testuser and click login") { // Example of a multi-command sequence
        return Ok("[{\"action\": \"TYPE\", \"selector\": \"css:#username\", \"value\": \"testuser\"}, {\"action\": \"CLICK\", \"selector\": \"css:#loginBtn\"}]".to_string());
    } else if prompt.contains("get logo src") { // Example for GETATTRIBUTE
        return Ok("[{\"action\": \"GETATTRIBUTE\", \"selector\": \"css:img#logo\", \"attribute_name\": \"src\"}]".to_string());
    } else if prompt.contains("set alt text for myImage") { // Example for SETATTRIBUTE
        return Ok("[{\"action\": \"SETATTRIBUTE\", \"selector\": \"id=myImage\", \"attribute_name\": \"alt\", \"value\": \"New alt text\"}]".to_string());

    // --- Group: Mocks for testing robust JSON parsing and error handling in agent.rs ---
    // These simulate various ways the LLM might return malformed or unexpected JSON.
    } else if prompt.contains("task expected to return invalid json") { // Not a valid JSON string.
        return Ok("This is not JSON.".to_string());
    } else if prompt.contains("task expected to return json object not array") { // Valid JSON, but not an array.
        return Ok("{\"message\": \"This is a JSON object, not an array.\"}".to_string());
    } else if prompt.contains("task expected to return json array of non-commands") { // Valid JSON array, but objects don't match LlmDomCommandRequest.
        return Ok("[{\"foo\": \"bar\"}]".to_string());
    } else if prompt.contains("task expected to return empty command array") { // Valid empty JSON array.
        return Ok("[]".to_string());
    } else if prompt.contains("task with mixed valid and invalid commands") { // Array with one command having an unrecognized "action".
        return Ok("[{\"action\": \"CLICK\", \"selector\": \"css:#ok\"}, {\"action\": \"INVALID_ACTION\", \"selector\": \"css:#bad\"}, {\"action\": \"TYPE\", \"selector\": \"css:#missingValue\"}]".to_string());
    } else if prompt.contains("task with mixed valid and malformed json commands") { // Array with one command object being structurally malformed.
        return Ok("[{\"action\": \"CLICK\", \"selector\": \"css:#valid\"}, {\"invalid_field\": \"some_value\", \"action\": \"EXTRA_INVALID_FIELD\"}, {\"action\": \"TYPE\", \"selector\": \"css:#anotherValid\", \"value\": \"test\"}]".to_string());
    }
    // --- Group: Mocks for testing placeholder substitution and sequential task execution in lib.rs (automate function) ---
    else if prompt.contains("get text from #element") { // Simulates LLM returning a simple string value.
        return Ok("Text from #element".to_string());
    }
    else if prompt.contains("TYPE css:#input Text from #element") { // Task after {{PREVIOUS_RESULT}} substitution.
        return Ok("[{\"action\": \"TYPE\", \"selector\": \"css:#input\", \"value\": \"Text from #element\"}]".to_string());
    }
    else if prompt.contains("TYPE css:#input ") && prompt.ends_with("{{PREVIOUS_RESULT}}") { // Task if {{PREVIOUS_RESULT}} was empty.
        return Ok("[{\"action\": \"TYPE\", \"selector\": \"css:#input\", \"value\": \"\"}]".to_string());
    }
    else if prompt.contains("click #first_button") {
        return Ok("Clicked #first_button".to_string());
    }
    else if prompt.contains("process Clicked #first_button for task B") {
        return Ok("Processed Clicked #first_button".to_string());
    }
    else if prompt.contains("process Processed Clicked #first_button for task C") {
        return Ok("Final result from C".to_string());
    }
    else if prompt.contains("get simple id") {
        return Ok("element_id_123".to_string());
    }
    else if prompt.contains("LLM_ACTION_EXPECTING_JSON_CMDS element_id_123") { // Task after {{PREVIOUS_RESULT}} substitution.
        return Ok("[{\"action\": \"CLICK\", \"selector\": \"#element_id_123\"}, {\"action\": \"READ\", \"selector\": \"#another_element\"}]".to_string());
    }
    // --- Group: Mocks for specific commands, often used in integration tests (lib.rs) and agent.rs tests ---
    else if prompt.contains("fill username and password and click login") { // Used in integration_test.rs (if it existed separately)
        return Ok("[{\"action\": \"TYPE\", \"selector\": \"css:#testuser\", \"value\": \"testuser\"}, {\"action\": \"TYPE\", \"selector\": \"css:#testpass\", \"value\": \"testpass\"}, {\"action\": \"CLICK\", \"selector\": \"css:#testloginbtn\"}]".to_string());
    }
    else if prompt.contains("llm_get_url_task") || prompt.contains("What is the current page URL?") {
        return Ok("[{\"action\": \"GET_URL\"}]".to_string());
    } else if prompt.contains("llm_element_exists_true_task") || prompt.contains("Is the button #llm-exists present?") {
        return Ok("[{\"action\": \"ELEMENT_EXISTS\", \"selector\": \"css:#llm-exists\"}]".to_string());
    } else if prompt.contains("llm_element_exists_false_task") || prompt.contains("Is #llm-nonexistent present?") {
        return Ok("[{\"action\": \"ELEMENT_EXISTS\", \"selector\": \"css:#llm-nonexistent\"}]".to_string());
    } else if prompt.contains("llm_wait_for_element_immediate_task") || prompt.contains("Wait for #llm-wait-immediate for 100ms") {
        return Ok("[{\"action\": \"WAIT_FOR_ELEMENT\", \"selector\": \"css:#llm-wait-immediate\", \"value\": \"100\"}]".to_string());
    } else if prompt.contains("llm_wait_for_element_timeout_task") || prompt.contains("Wait for #llm-wait-timeout for 50ms") {
        return Ok("[{\"action\": \"WAIT_FOR_ELEMENT\", \"selector\": \"css:#llm-wait-timeout\", \"value\": \"50\"}]".to_string());
    }
    // --- Group: Mocks for IS_VISIBLE and SCROLL_TO commands ---
    else if prompt.contains("Is the #mainContent visible?") {
        return Ok("[{\"action\": \"IS_VISIBLE\", \"selector\": \"css:#mainContent\"}]".to_string());
    } else if prompt.contains("Is #sidebar hidden?") { // Example to test IS_VISIBLE, result depends on actual element state.
        return Ok("[{\"action\": \"IS_VISIBLE\", \"selector\": \"css:#sidebar\"}]".to_string());
    } else if prompt.contains("Scroll to the footer") {
        return Ok("[{\"action\": \"SCROLL_TO\", \"selector\": \"css:footer\"}]".to_string());
    } else if prompt.contains("scroll to #detailsSection") {
        return Ok("[{\"action\": \"SCROLL_TO\", \"selector\": \"css:#detailsSection\"}]".to_string());
    }
    // --- Group: General Fallbacks & Error Simulation ---
    else if prompt.contains("this task should fail_llm_call please") { // Simulates an LLM API error.
        return Err(JsValue::from_str("Mocked LLM Error: LLM call failed as requested by prompt."));
    } else if prompt.contains("navigate to example.com") { // Simulates a simple natural language response.
        return Ok("Mocked LLM response for 'navigate to example.com'".to_string());
    } else if prompt.contains("fill the login form with my details") { // Simulates a natural language response.
        return Ok("Mocked LLM response for 'fill the login form with my details'".to_string());
    } else if prompt.contains("summarize this document for me") { // Simulates a natural language response.
        return Ok("Mocked LLM response for 'summarize this document for me'".to_string());
    } else if prompt.contains("navigate then CLICK xpath://button[@id='specificButtonXpath']") { // Simulates a more complex natural language query.
         return Ok("Mocked LLM response for 'navigate then CLICK xpath://button[@id='specificButtonXpath']'".to_string());
    } else if prompt.contains("navigate then CLICK #myButtonDirect") { // Simulates another complex natural language query.
        return Ok("Mocked LLM response for 'navigate then CLICK #myButtonDirect'".to_string());
    }
    // Fallback for other structured prompts (containing the task marker) not matching specific test cases.
    else if prompt.contains("The user wants to perform the following task:") {
        let task_content = extract_task_from_prompt(&prompt);
        return Ok(format!("Mocked LLM response regarding task: '{}'", task_content));
    }
    // Legacy fallback for a specific error test prompt (might be obsolete or used in older tests).
    else if prompt.contains("error_test_prompt") {
        return Ok("Mocked LLM response: This prompt triggers a success for error testing.".to_string());
    }
    // Default fallback for any unhandled prompt, helps in identifying missing mock conditions during testing.
    else {
        return Ok(format!("Default Mocked LLM response for unhandled prompt structure: {}", prompt));
    }
}

/// Helper function to extract the core task description from the full LLM prompt string.
/// This is useful for logging and for creating generic mock responses.
/// It looks for the pattern `The user wants to perform the following task: "{task}"`.
fn extract_task_from_prompt(prompt_str: &str) -> String {
    let task_marker = "The user wants to perform the following task: \"";
    if let Some(start_index) = prompt_str.find(task_marker) {
        let actual_task_start = start_index + task_marker.len();
        if let Some(end_index) = prompt_str[actual_task_start..].find("\"") {
            return prompt_str[actual_task_start .. actual_task_start + end_index].to_string();
        }
    }
    "Unknown or malformed task".to_string()
}