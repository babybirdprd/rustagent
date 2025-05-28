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
        .post(api_url)
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
        console::error_1(&error_message.into());
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
    console::log_1(&format!("call_llm_async called (MOCK) for prompt:\n{}", prompt).into());

    // Define task descriptions for clarity in matching
    let task_nav_example = "navigate to example.com";
    let task_fill_form = "fill the login form with my details";
    let task_summarize_doc = "summarize this document for me";
    let task_fail_llm = "this task should fail_llm_call please";
    let task_nav_click_xpath = "navigate then CLICK xpath://button[@id='specificButtonXpath']";
    let task_nav_click_direct = "navigate then CLICK #myButtonDirect";

    // Constructing expected prompt substrings based on the new structured format
    let nav_prompt_substring = format!("You are Agent 1 (Navigator).\n\
                The user wants to perform the following task: \"{}\"", task_nav_example);
    let form_prompt_substring = format!("You are Agent 2 (FormFiller).\n\
                The user wants to perform the following task: \"{}\"", task_fill_form);
    let generic_summary_prompt_substring = format!("You are Agent 3 (Generic).\n\
                The user wants to perform the following task: \"{}\"", task_summarize_doc);
    let generic_fail_prompt_substring = format!("You are Agent 3 (Generic).\n\
                The user wants to perform the following task: \"{}\"", task_fail_llm);
    let nav_click_xpath_prompt_substring = format!("You are Agent 1 (Navigator).\n\
                The user wants to perform the following task: \"{}\"", task_nav_click_xpath);
    let nav_click_direct_prompt_substring = format!("You are Agent 1 (Navigator).\n\
                The user wants to perform the following task: \"{}\"", task_nav_click_direct);

    if prompt.contains(&generic_fail_prompt_substring) {
        Err(JsValue::from_str("Mocked LLM Error: LLM call failed as requested by prompt."))
    } else if prompt.contains(&nav_prompt_substring) {
        Ok(format!("Mocked LLM response for '{}'", task_nav_example))
    } else if prompt.contains(&form_prompt_substring) {
        Ok(format!("Mocked LLM response for '{}'", task_fill_form))
    } else if prompt.contains(&generic_summary_prompt_substring) {
        Ok(format!("Mocked LLM response for '{}'", task_summarize_doc))
    } else if prompt.contains(&nav_click_xpath_prompt_substring) {
        Ok(format!("Mocked LLM response for '{}'", task_nav_click_xpath))
    } else if prompt.contains(&nav_click_direct_prompt_substring) {
        Ok(format!("Mocked LLM response for '{}'", task_nav_click_direct))
    }
    // Fallback for prompts not matching specific test cases but are structured
    else if prompt.contains("The user wants to perform the following task:") {
        // Extract the original task for a more dynamic generic response
        let task_marker = "The user wants to perform the following task: \"";
        if let Some(start_index) = prompt.find(task_marker) {
            let actual_task_start = start_index + task_marker.len();
            if let Some(end_index) = prompt[actual_task_start..].find("\"") {
                let task_content = &prompt[actual_task_start .. actual_task_start + end_index];
                return Ok(format!("Mocked LLM response regarding task: '{}'", task_content));
            }
        }
        // If task extraction fails, return a generic structured response
        Ok("Generic Mocked LLM response for a structured prompt.".to_string())
    }
    // Legacy fallback for any old-style prompts if they still exist in some tests
    else if prompt.contains("error_test_prompt") {
        Ok("Mocked LLM response: This prompt triggers a success for error testing.".to_string())
    }
    else {
         // This is a true fallback if none of the above conditions are met.
        Ok(format!("Default Mocked LLM response for unhandled prompt structure: {}", prompt))
    }
}