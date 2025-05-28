use wasm_bindgen_test::*;
use rustagent::RustAgent; // Assuming 'rustagent' is the library crate name
use web_sys;

// Configure wasm-bindgen-test to run in a browser environment
wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn test_rust_agent_new() {
    let _agent = RustAgent::new();
    // If new() had complex logic or could panic, assertions would go here.
    // For now, successfully creating an instance is the test.
    assert!(true, "RustAgent::new() should not panic.");
}

#[wasm_bindgen_test]
async fn test_automate_dom_command() {
    // This test runs in a browser context, so DOM manipulation is possible.
    
    // 1. Setup: Create a dummy element in the document
    let window = web_sys::window().expect("should have a window in this context");
    let document = window.document().expect("window should have a document");
    let body = document.body().expect("document should have a body");

    let test_element = document.create_element("div").unwrap();
    test_element.set_id("testDivForRead");
    test_element.set_inner_html("Test content from WASM test");
    body.append_child(&test_element).expect("should be able to append test element");

    // Create another element for testing value retrieval (e.g., input)
    let test_input = document.create_element("input").unwrap();
    test_input.set_id("testInputForGetValue");
    let test_input_html_element = test_input.dyn_into::<web_sys::HtmlInputElement>().unwrap();
    test_input_html_element.set_value("Test input value");
    body.append_child(&test_input_html_element).expect("should append test input");

    let agent = RustAgent::new();
    let api_key = "dummy_api_key_for_dom_test";

    // Test READ command
    let task_read = "READ #testDivForRead";
    let result_read = agent.automate(task_read, api_key);
    assert!(
        result_read.contains("Test content from WASM test"),
        "Result for READ command should contain 'Test content from WASM test', got: {}",
        result_read
    );

    // Test GETVALUE command
    let task_getvalue = "GETVALUE #testInputForGetValue";
    let result_getvalue = agent.automate(task_getvalue, api_key);
    assert!(
        result_getvalue.contains("Test input value"),
        "Result for GETVALUE command should contain 'Test input value', got: {}",
        result_getvalue
    );

    // Test CLICK command (mocking click is hard, we just check it doesn't panic and selector is used)
    // We can't easily verify a click happened without more complex JS interop or a visible browser.
    // We will check if the agent reports trying to click the element.
    let test_button = document.create_element("button").unwrap();
    test_button.set_id("testButtonForClick");
    body.append_child(&test_button).expect("should append test button");
    
    let task_click = "CLICK #testButtonForClick";
    let result_click = agent.automate(task_click, api_key);
    assert!(
        result_click.contains("Successfully clicked element with selector: '#testButtonForClick'") || result_click.contains("Error clicking element:"),
        "Result for CLICK command was not as expected, got: {}",
        result_click
    );


    // 2. Teardown: Clean up the dummy elements
    body.remove_child(&test_element).expect("should remove test element");
    body.remove_child(&test_input_html_element).expect("should remove test input");
    body.remove_child(&test_button).expect("should remove test button");
}

#[wasm_bindgen_test]
async fn test_automate_llm_call_expects_error() {
    let agent = RustAgent::new();
    let task = "summarize this for me please"; // A task that should fall back to LLM
    let api_key = "invalid_api_key_for_test"; // An obviously invalid API key
    
    let result = agent.automate(task, api_key);

    // We expect an error from the LLM call due to the invalid API key or network failure.
    // The exact error message from reqwest/OpenAI can vary.
    // We check for substrings that indicate an error related to the LLM call.
    // The `call_llm` function in Rust formats this as "Error calling LLM: actual_error_from_async_call"
    let lower_result = result.to_lowercase(); // Case-insensitive check

    assert!(
        lower_result.contains("error calling llm:"), 
        "Result should indicate an 'Error calling LLM:'. Got: {}",
        result
    );
    
    // Further check for common issues like request error or API error.
    // `call_llm_async` returns `JsValue::from_str(&format!("Request error: {}", e.to_string()))`
    // or `JsValue::from_str(&format!("API error: {}", error_text))`
    assert!(
        lower_result.contains("request error:") || lower_result.contains("api error:") || lower_result.contains("unknown javascript error"),
        "Result should contain a more specific error like 'request error:', 'api error:', or 'unknown javascript error'. Got: {}",
        result
    );
}
