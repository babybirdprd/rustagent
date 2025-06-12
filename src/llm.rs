use wasm_bindgen::prelude::*;
use serde_json::json; // Used by both real and mock
use web_sys::console; // Used by both real and mock

#[cfg(not(feature = "mock-llm"))]
use reqwest::Client; // Only used by the real implementation

#[cfg(not(feature = "mock-llm"))]
#[wasm_bindgen]
pub async fn call_llm_async(prompt: String, api_key: String, api_url: String, model_name: String) -> Result<String, JsValue> {
    console::log_1(&"call_llm_async called (REAL)".into());

    let client = Client::new();
    
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

    // --- New prompts for JSON command responses ---
    if prompt.contains("click the submit button") {
        return Ok("[{\"action\": \"CLICK\", \"selector\": \"css:#submitBtn\"}]".to_string());
    } else if prompt.contains("login with testuser and click login") {
        return Ok("[{\"action\": \"TYPE\", \"selector\": \"css:#username\", \"value\": \"testuser\"}, {\"action\": \"CLICK\", \"selector\": \"css:#loginBtn\"}]".to_string());
    } else if prompt.contains("get logo src") { // For GETATTRIBUTE
        return Ok("[{\"action\": \"GETATTRIBUTE\", \"selector\": \"css:img#logo\", \"attribute_name\": \"src\"}]".to_string());
    } else if prompt.contains("set alt text for myImage") { // For SETATTRIBUTE
        return Ok("[{\"action\": \"SETATTRIBUTE\", \"selector\": \"id=myImage\", \"attribute_name\": \"alt\", \"value\": \"New alt text\"}]".to_string());
    } else if prompt.contains("task expected to return invalid json") {
        return Ok("This is not JSON.".to_string());
    } else if prompt.contains("task expected to return json object not array") {
        return Ok("{\"message\": \"This is a JSON object, not an array.\"}".to_string());
    } else if prompt.contains("task expected to return json array of non-commands") {
        return Ok("[{\"foo\": \"bar\"}]".to_string());
    } else if prompt.contains("task expected to return empty command array") {
        return Ok("[]".to_string());
    } else if prompt.contains("task with mixed valid and invalid commands") {
        return Ok("[{\"action\": \"CLICK\", \"selector\": \"css:#ok\"}, {\"action\": \"INVALID_ACTION\", \"selector\": \"css:#bad\"}, {\"action\": \"TYPE\", \"selector\": \"css:#missingValue\"}]".to_string());
    }
    // --- New Mocks for automate tests ---
    else if prompt.contains("get text from #element") { // For Scenario 2 (first task)
        // This task is specific enough that parse_dom_command might not pick it up, so LLM is called.
        // We want this to return a simple string that can be used by the next task.
        return Ok("Text from #element".to_string());
    }
    else if prompt.contains("TYPE css:#input Text from #element") { // For Scenario 2 (second task, after substitution)
        // LLM is asked to perform "TYPE css:#input Text from #element"
        // It should respond with a JSON command for the agent to execute.
        return Ok("[{\"action\": \"TYPE\", \"selector\": \"css:#input\", \"value\": \"Text from #element\"}]".to_string());
    }
    else if prompt.contains("TYPE css:#input ") && prompt.ends_with("{{PREVIOUS_RESULT}}") { // Scenario 3 (second task, placeholder becomes empty)
         // This case handles when {{PREVIOUS_RESULT}} was empty.
         // The prompt to LLM would be "TYPE css:#input " (with a trailing space if not trimmed)
        return Ok("[{\"action\": \"TYPE\", \"selector\": \"css:#input\", \"value\": \"\"}]".to_string());
    }
    else if prompt.contains("click #first_button") { // For Scenario 5 (Task A)
        return Ok("Clicked #first_button".to_string()); // Simple string output
    }
    else if prompt.contains("process Clicked #first_button for task B") { // For Scenario 5 (Task B, after substitution)
        return Ok("Processed Clicked #first_button".to_string()); // Simple string output
    }
    else if prompt.contains("process Processed Clicked #first_button for task C") { // For Scenario 5 (Task C, after substitution)
        return Ok("Final result from C".to_string());
    }
    else if prompt.contains("get simple id") { // For Scenario 6 (Task A)
        return Ok("element_id_123".to_string()); // Simple string output
    }
    else if prompt.contains("LLM_ACTION_EXPECTING_JSON_CMDS element_id_123") { // For Scenario 6 (Task B, after substitution)
        return Ok("[{\"action\": \"CLICK\", \"selector\": \"#element_id_123\"}, {\"action\": \"READ\", \"selector\": \"#another_element\"}]".to_string());
    }
    // --- New Mocks for integration_test.rs ---
    else if prompt.contains("fill username and password and click login") { // For integration test 1
        return Ok("[{\"action\": \"TYPE\", \"selector\": \"css:#testuser\", \"value\": \"testuser\"}, {\"action\": \"TYPE\", \"selector\": \"css:#testpass\", \"value\": \"testpass\"}, {\"action\": \"CLICK\", \"selector\": \"css:#testloginbtn\"}]".to_string());
    }
    // --- Mocks for new commands tested in agent.rs and lib.rs ---
    else if prompt.contains("llm_get_url_task") || prompt.contains("What is the current page URL?") {
        return Ok("[{\"action\": \"GET_URL\"}]".to_string());
    } else if prompt.contains("llm_element_exists_true_task") || prompt.contains("Is the button #llm-exists present?") { // Matches agent test
        return Ok("[{\"action\": \"ELEMENT_EXISTS\", \"selector\": \"css:#llm-exists\"}]".to_string());
    } else if prompt.contains("llm_element_exists_false_task") || prompt.contains("Is #llm-nonexistent present?") { // Matches agent test
        return Ok("[{\"action\": \"ELEMENT_EXISTS\", \"selector\": \"css:#llm-nonexistent\"}]".to_string());
    } else if prompt.contains("llm_wait_for_element_immediate_task") || prompt.contains("Wait for #llm-wait-immediate for 100ms") { // Matches agent test
        return Ok("[{\"action\": \"WAIT_FOR_ELEMENT\", \"selector\": \"css:#llm-wait-immediate\", \"value\": \"100\"}]".to_string());
    } else if prompt.contains("llm_wait_for_element_timeout_task") || prompt.contains("Wait for #llm-wait-timeout for 50ms") { // Matches agent test
        return Ok("[{\"action\": \"WAIT_FOR_ELEMENT\", \"selector\": \"css:#llm-wait-timeout\", \"value\": \"50\"}]".to_string());
    }
    // --- Maintain existing/updated behaviors ---
    else if prompt.contains("this task should fail_llm_call please") {
        return Err(JsValue::from_str("Mocked LLM Error: LLM call failed as requested by prompt."));
    } else if prompt.contains("navigate to example.com") {
        // Kept as plain text for now to test this fallback path
        return Ok("Mocked LLM response for 'navigate to example.com'".to_string());
    } else if prompt.contains("fill the login form with my details") {
        // Kept as plain text due to generic nature of prompt
        return Ok("Mocked LLM response for 'fill the login form with my details'".to_string());
    } else if prompt.contains("summarize this document for me") {
        return Ok("Mocked LLM response for 'summarize this document for me'".to_string());
    } else if prompt.contains("navigate then CLICK xpath://button[@id='specificButtonXpath']") {
         return Ok("Mocked LLM response for 'navigate then CLICK xpath://button[@id='specificButtonXpath']'".to_string());
    } else if prompt.contains("navigate then CLICK #myButtonDirect") {
        return Ok("Mocked LLM response for 'navigate then CLICK #myButtonDirect'".to_string());
    }
    // Fallback for other structured prompts not matching specific test cases
    else if prompt.contains("The user wants to perform the following task:") {
        let task_content = extract_task_from_prompt(&prompt);
        return Ok(format!("Mocked LLM response regarding task: '{}'", task_content));
    }
    // Legacy fallback
    else if prompt.contains("error_test_prompt") {
        return Ok("Mocked LLM response: This prompt triggers a success for error testing.".to_string());
    }
    // Default fallback
    else {
        return Ok(format!("Default Mocked LLM response for unhandled prompt structure: {}", prompt));
    }
}

// Helper function to extract task from the full prompt for logging or generic responses
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