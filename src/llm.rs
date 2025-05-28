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
    console::log_1(&format!("call_llm_async called (MOCK) for prompt: {}", prompt).into());
    
    if prompt.contains("error_test_prompt") {
        Ok("Mocked LLM response: This prompt triggers a success for error testing.".to_string())
    } else if prompt.contains("fail_llm_call") {
        Err(JsValue::from_str("Mocked LLM Error: LLM call failed as requested by prompt."))
    } else if prompt.contains("Agent 1 (Navigator): navigate to example.com") {
        Ok("Mocked LLM response for 'navigate to example.com'".to_string())
    } else if prompt.contains("Agent 2 (FormFiller): fill the login form") {
        Ok("Mocked LLM response for 'fill the login form'".to_string())
    } else if prompt.contains("Agent 3 (Generic): summarize this document") {
        Ok("Mocked LLM response for 'summarize this document'".to_string())
    } else {
        Ok(format!("Mocked LLM response for prompt: {}", prompt))
    }
}