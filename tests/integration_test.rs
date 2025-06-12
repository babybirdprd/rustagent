// tests/integration_test.rs

use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;
use web_sys::{Element, HtmlElement, HtmlInputElement, HtmlSelectElement, Document};

// Assuming your crate is named `rustagent` and dom_utils are public functions in `src/dom_utils.rs`
// and exposed via `pub mod dom_utils;` in `src/lib.rs` or directly `pub use crate::dom_utils::*;`
// This means the functions are available under `rustagent::dom_utils::*`
// For the purpose of this integration test, we assume the crate is named `rustagent`.
// If `dom_utils` is a module within `rustagent`, then `rustagent::dom_utils::function_name`.
// If functions are directly exported by `lib.rs` (e.g. `pub use crate::dom_utils::click_element;`),
// then `rustagent::click_element` would be the path.
// Let's assume they are exposed under the crate root for simplicity here,
// or that `pub mod dom_utils;` is in `lib.rs` making them `rustagent::dom_utils::*`.
use rustagent::dom_utils::{
    click_element, type_in_element, get_element_text, get_element_value,
    get_element_attribute, set_element_attribute, select_dropdown_option,
    // get_all_elements_attributes is also available if needed directly, but we test via RustAgent
};
use rustagent::RustAgent; // Import RustAgent for automate tests
use serde_json; // For parsing JSON results from automate

wasm_bindgen_test_configure!(run_in_browser);

// Helper to get document
fn document() -> Document {
    web_sys::window().unwrap().document().unwrap()
}

// Helper to get an element for direct checks (not using the utils themselves for this part)
fn get_html_element_by_id_for_test(id: &str) -> Option<HtmlElement> {
    document().get_element_by_id(id)?.dyn_into::<HtmlElement>().ok()
}

fn get_html_input_element_by_id_for_test(id: &str) -> Option<HtmlInputElement> {
    document().get_element_by_id(id)?.dyn_into::<HtmlInputElement>().ok()
}

fn get_html_select_element_by_id_for_test(id: &str) -> Option<HtmlSelectElement> {
    document().get_element_by_id(id)?.dyn_into::<HtmlSelectElement>().ok()
}


#[wasm_bindgen_test]
async fn test_click_element_success_css() {
    let result_span = get_html_element_by_id_for_test("clickResult").expect("#clickResult should exist");
    assert_eq!(result_span.text_content().unwrap_or_default(), "Not Clicked", "Initial text");
    
    let click_result = click_element("#testButton");
    assert!(click_result.is_ok(), "click_element with CSS selector failed: {:?}", click_result.err());
    
    assert_eq!(result_span.text_content().unwrap_or_default(), "Button Clicked!", "Text did not change after click");
}

#[wasm_bindgen_test]
async fn test_click_element_success_xpath() {
    let result_span = get_html_element_by_id_for_test("clickResult").expect("#clickResult should exist");
    result_span.set_text_content(Some("Not Clicked")); // Reset for this test
    assert_eq!(result_span.text_content().unwrap_or_default(), "Not Clicked", "Initial text for XPath test");

    let click_result = click_element("xpath://button[@id='testButton']");
    assert!(click_result.is_ok(), "click_element with XPath selector failed: {:?}", click_result.err());
    
    assert_eq!(result_span.text_content().unwrap_or_default(), "Button Clicked!", "Text did not change after XPath click");
}

#[wasm_bindgen_test]
async fn test_click_element_not_found() {
    let res_css = click_element("#nonExistentButton");
    assert!(res_css.is_err());
    assert_eq!(res_css.unwrap_err().as_string().unwrap(), "ElementNotFound: No element found for CSS selector '#nonExistentButton'");

    let res_xpath = click_element("xpath://button[@id='nonExistentButtonXPath']");
    assert!(res_xpath.is_err());
    assert_eq!(res_xpath.unwrap_err().as_string().unwrap(), "ElementNotFound: No element found for XPath selector 'xpath://button[@id='nonExistentButtonXPath']'");
}

#[wasm_bindgen_test]
async fn test_type_in_element_success() {
    let input_el = get_html_input_element_by_id_for_test("textInput").expect("#textInput should exist");
    let test_text = "Hello, RustAgent!";

    // Test with CSS
    input_el.set_value(""); // Clear
    let type_res_css = type_in_element("#textInput", test_text);
    assert!(type_res_css.is_ok(), "type_in_element CSS failed: {:?}", type_res_css.err());
    assert_eq!(get_element_value("#textInput").unwrap(), test_text, "CSS type check");

    // Test with XPath
    input_el.set_value(""); // Clear
    let type_res_xpath = type_in_element("xpath://input[@id='textInput']", test_text);
    assert!(type_res_xpath.is_ok(), "type_in_element XPath failed: {:?}", type_res_xpath.err());
    assert_eq!(get_element_value("xpath://input[@id='textInput']").unwrap(), test_text, "XPath type check");
}

#[wasm_bindgen_test]
async fn test_type_in_element_not_input() {
    let res = type_in_element("#nonInputDiv", "test"); // #nonInputDiv is a div
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().as_string().unwrap(), "ElementTypeError: Element for selector '#nonInputDiv' is not an input element.");
}

#[wasm_bindgen_test]
async fn test_type_in_element_not_found() {
    let res = type_in_element("#nonExistentForType", "test");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().as_string().unwrap(), "ElementNotFound: No element found for CSS selector '#nonExistentForType'");
}

#[wasm_bindgen_test]
async fn test_get_element_text_success() {
    assert_eq!(get_element_text("#textDisplay").unwrap(), "Initial Text Content");
    assert_eq!(get_element_text("xpath://div[@id='textDisplay']").unwrap(), "Initial Text Content");
    assert_eq!(get_element_text("#emptyTextDisplay").unwrap(), "");
    assert_eq!(get_element_text("xpath://div[@data-xpath-target='true']").unwrap(), "XPath Target Text");
}

#[wasm_bindgen_test]
async fn test_get_element_text_not_found() {
    let res = get_element_text("#nonExistentForGetText");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().as_string().unwrap(), "ElementNotFound: No element found for CSS selector '#nonExistentForGetText'");
}

#[wasm_bindgen_test]
async fn test_get_element_value_success() {
    assert_eq!(get_element_value("#valueInput").unwrap(), "Initial Value");
    assert_eq!(get_element_value("xpath://input[@id='valueInput']").unwrap(), "Initial Value");
}

#[wasm_bindgen_test]
async fn test_get_element_value_not_input() {
    let res = get_element_value("#textDisplay"); // #textDisplay is a div
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().as_string().unwrap(), "ElementTypeError: Element for selector '#textDisplay' is not an input element.");
}

#[wasm_bindgen_test]
async fn test_get_element_value_not_found() {
    let res = get_element_value("#nonExistentForGetValue");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().as_string().unwrap(), "ElementNotFound: No element found for CSS selector '#nonExistentForGetValue'");
}

#[wasm_bindgen_test]
async fn test_get_element_attribute_success() {
    assert_eq!(get_element_attribute("#attributeElement", "data-test").unwrap(), "initial_value");
    assert_eq!(get_element_attribute("xpath://*[@id='attributeElement']", "data-test").unwrap(), "initial_value");
    assert_eq!(get_element_attribute("#attributeElement", "class").unwrap(), "test-class");
}

#[wasm_bindgen_test]
async fn test_get_element_attribute_attr_not_found() {
    let res = get_element_attribute("#attributeElement", "non-existent-attribute");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().as_string().unwrap(), "AttributeNotFound: Attribute 'non-existent-attribute' not found on element with selector '#attributeElement'");
}

#[wasm_bindgen_test]
async fn test_get_element_attribute_element_not_found() {
    let res = get_element_attribute("#nonExistentForGetAttr", "data-test");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().as_string().unwrap(), "ElementNotFound: No element found for CSS selector '#nonExistentForGetAttr'");
}

#[wasm_bindgen_test]
async fn test_set_element_attribute_success() {
    let attr_name = "data-test";
    let new_value_css = "new_value_css";
    let new_value_xpath = "new_value_xpath";

    // Test with CSS
    let set_res_css = set_element_attribute("#attributeElement", attr_name, new_value_css);
    assert!(set_res_css.is_ok(), "set_element_attribute CSS failed: {:?}", set_res_css.err());
    assert_eq!(get_element_attribute("#attributeElement", attr_name).unwrap(), new_value_css);

    // Test with XPath
    let set_res_xpath = set_element_attribute("xpath://*[@id='attributeElement']", attr_name, new_value_xpath);
    assert!(set_res_xpath.is_ok(), "set_element_attribute XPath failed: {:?}", set_res_xpath.err());
    assert_eq!(get_element_attribute("xpath://*[@id='attributeElement']", attr_name).unwrap(), new_value_xpath);
}

#[wasm_bindgen_test]
async fn test_set_element_attribute_element_not_found() {
    let res = set_element_attribute("#nonExistentForSetAttr", "data-test", "value");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().as_string().unwrap(), "ElementNotFound: No element found for CSS selector '#nonExistentForSetAttr'");
}

#[wasm_bindgen_test]
async fn test_select_dropdown_option_success() {
    let select_el = get_html_select_element_by_id_for_test("selectElement").expect("#selectElement should exist");
    
    // Initial value check
    assert_eq!(select_el.value(), "val2", "Initial selected value");

    // Test with CSS
    let sel_res_css = select_dropdown_option("#selectElement", "val3");
    assert!(sel_res_css.is_ok(), "select_dropdown_option CSS failed: {:?}", sel_res_css.err());
    assert_eq!(select_el.value(), "val3", "CSS select check");

    // Test with XPath
    let sel_res_xpath = select_dropdown_option("xpath://select[@id='selectElement']", "val1");
    assert!(sel_res_xpath.is_ok(), "select_dropdown_option XPath failed: {:?}", sel_res_xpath.err());
    assert_eq!(select_el.value(), "val1", "XPath select check");
}

#[wasm_bindgen_test]
async fn test_select_dropdown_option_non_existent_value() {
    let select_el = get_html_select_element_by_id_for_test("selectElement").expect("#selectElement should exist");
    let initial_val = select_el.value(); // Store current value
    
    let res = select_dropdown_option("#selectElement", "nonExistentValue");
    // Setting a non-existent value on a select element doesn't throw an error in browsers,
    // it usually results in no change or the first option being selected.
    // Our function will return Ok if the element is a select, regardless of value existence.
    assert!(res.is_ok(), "select_dropdown_option with non-existent value should be Ok");
    
    // Verify the value hasn't changed (or changed to specific browser default if applicable)
    // For robustness, we check it's not the value we tried to set, or it's still the initial.
    let current_val = select_el.value();
    assert_ne!(current_val, "nonExistentValue");
    if !current_val.is_empty() { // If an option is selected (e.g. first one by default)
      assert_eq!(current_val, initial_val, "Value should remain unchanged or be a default if non-existent value was set.");
    } else {
      // Some browsers might make value empty if non-existent option is set
      assert!(current_val.is_empty() || current_val == initial_val, "Value should be empty or initial after trying to set non-existent option.");
    }
}

#[wasm_bindgen_test]
async fn test_select_dropdown_option_not_select() {
    let res = select_dropdown_option("#nonSelectDiv", "val1"); // #nonSelectDiv is a div
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().as_string().unwrap(), "ElementTypeError: Element for selector '#nonSelectDiv' is not a select element.");
}

#[wasm_bindgen_test]
async fn test_select_dropdown_option_element_not_found() {
    let res = select_dropdown_option("#nonExistentForSelect", "val1");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().as_string().unwrap(), "ElementNotFound: No element found for CSS selector '#nonExistentForSelect'");
}

// Specific XPath test
#[wasm_bindgen_test]
async fn test_get_text_xpath_target() {
    assert_eq!(get_element_text("xpath://div[@data-xpath-target='true']").unwrap(), "XPath Target Text");
}

// Invalid selector tests
#[wasm_bindgen_test]
async fn test_invalid_css_selector_error() {
    let res = get_element_text("css:[[[invalid");
    assert!(res.is_err());
    assert!(res.unwrap_err().as_string().unwrap().starts_with("InvalidSelector: Invalid CSS selector 'css:[[[invalid'. Details:"));
}

#[wasm_bindgen_test]
async fn test_invalid_xpath_selector_error() {
    let res = get_element_text("xpath://[invalid-xpath");
    assert!(res.is_err());
    assert!(res.unwrap_err().as_string().unwrap().starts_with("InvalidSelector: Invalid XPath expression 'xpath://[invalid-xpath'. Details:"));
}


// --- Tests for GET_ALL_ATTRIBUTES via RustAgent.automate ---

#[wasm_bindgen_test]
async fn test_get_all_attributes_css_success() {
    let agent = RustAgent::new();
    let task_string = "GET_ALL_ATTRIBUTES css:.attr-item data-value";
    let tasks_json = format!(r#"[["{}"]]"#, task_string);

    let js_value_from_automate = agent.automate(&tasks_json).await.unwrap_or_else(|err| panic!("Automate call failed: {:?}", err));
    let results_list_json = js_value_from_automate.as_string().expect("Automate result should be a string");
    let parsed_results: Vec<Result<String, String>> = serde_json::from_str(&results_list_json).unwrap_or_else(|err| panic!("Failed to parse automate results: {}", err));

    assert_eq!(parsed_results.len(), 1, "Expected one result from the task list");
    let task_result = parsed_results[0].as_ref().unwrap_or_else(|err| panic!("Task failed: {}", err));
    
    // Order of elements with class "attr-item": item-1, item-2, item-3, div-item-5
    // data-value attributes: "apple", null, "cherry", "elderberry"
    let expected_json_payload = r#"["apple",null,"cherry","elderberry"]"#;
    assert!(task_result.contains(expected_json_payload), 
            "Task result '{}' did not contain expected JSON payload '{}'", task_result, expected_json_payload);
    assert!(task_result.contains("Successfully retrieved attributes 'data-value' for elements matching selector 'css:.attr-item'"));
}

#[wasm_bindgen_test]
async fn test_get_all_attributes_xpath_success() {
    let agent = RustAgent::new();
    let task_string = "GET_ALL_ATTRIBUTES xpath://span[@class='xpath-attr'] data-fruit";
    let tasks_json = format!(r#"[["{}"]]"#, task_string);

    let js_value_from_automate = agent.automate(&tasks_json).await.unwrap_or_else(|err| panic!("Automate call failed: {:?}", err));
    let results_list_json = js_value_from_automate.as_string().expect("Automate result should be a string");
    let parsed_results: Vec<Result<String, String>> = serde_json::from_str(&results_list_json).unwrap_or_else(|err| panic!("Failed to parse automate results: {}", err));

    assert_eq!(parsed_results.len(), 1, "Expected one result from the task list");
    let task_result = parsed_results[0].as_ref().unwrap_or_else(|err| panic!("Task failed: {}", err));
    
    // Elements: grape, fig, null
    let expected_json_payload = r#"["grape","fig",null]"#;
    assert!(task_result.contains(expected_json_payload), 
            "Task result '{}' did not contain expected JSON payload '{}'", task_result, expected_json_payload);
    assert!(task_result.contains("Successfully retrieved attributes 'data-fruit' for elements matching selector 'xpath://span[@class='xpath-attr']'"));
}

#[wasm_bindgen_test]
async fn test_get_all_attributes_no_elements_found() {
    let agent = RustAgent::new();
    let task_string = "GET_ALL_ATTRIBUTES css:.non-existent-class data-value";
    let tasks_json = format!(r#"[["{}"]]"#, task_string);

    let js_value_from_automate = agent.automate(&tasks_json).await.unwrap_or_else(|err| panic!("Automate call failed: {:?}", err));
    let results_list_json = js_value_from_automate.as_string().expect("Automate result should be a string");
    let parsed_results: Vec<Result<String, String>> = serde_json::from_str(&results_list_json).unwrap_or_else(|err| panic!("Failed to parse automate results: {}", err));

    assert_eq!(parsed_results.len(), 1, "Expected one result from the task list");
    let task_result = parsed_results[0].as_ref().unwrap_or_else(|err| panic!("Task failed: {}", err));
    
    let expected_json_payload = r#"[]"#; // Empty array
    assert!(task_result.contains(expected_json_payload), 
            "Task result '{}' did not contain expected JSON payload '{}'", task_result, expected_json_payload);
    assert!(task_result.contains("Successfully retrieved attributes 'data-value' for elements matching selector 'css:.non-existent-class'"));
}

#[wasm_bindgen_test]
async fn test_get_all_attributes_attribute_non_existent_on_any() {
    let agent = RustAgent::new();
    // Use .attr-item which has 4 matching elements
    let task_string = "GET_ALL_ATTRIBUTES css:.attr-item data-nonexistent";
    let tasks_json = format!(r#"[["{}"]]"#, task_string);

    let js_value_from_automate = agent.automate(&tasks_json).await.unwrap_or_else(|err| panic!("Automate call failed: {:?}", err));
    let results_list_json = js_value_from_automate.as_string().expect("Automate result should be a string");
    let parsed_results: Vec<Result<String, String>> = serde_json::from_str(&results_list_json).unwrap_or_else(|err| panic!("Failed to parse automate results: {}", err));

    assert_eq!(parsed_results.len(), 1, "Expected one result from the task list");
    let task_result = parsed_results[0].as_ref().unwrap_or_else(|err| panic!("Task failed: {}", err));
    
    // Four elements match .attr-item, none will have 'data-nonexistent'
    let expected_json_payload = r#"[null,null,null,null]"#;
    assert!(task_result.contains(expected_json_payload), 
            "Task result '{}' did not contain expected JSON payload '{}'", task_result, expected_json_payload);
    assert!(task_result.contains("Successfully retrieved attributes 'data-nonexistent' for elements matching selector 'css:.attr-item'"));
}

#[wasm_bindgen_test]
async fn test_get_all_attributes_invalid_selector() {
    let agent = RustAgent::new();
    let task_string = "GET_ALL_ATTRIBUTES css:[[[ data-value";
    let tasks_json = format!(r#"[["{}"]]"#, task_string);

    let js_value_from_automate = agent.automate(&tasks_json).await.unwrap_or_else(|err| panic!("Automate call failed: {:?}", err));
    let results_list_json = js_value_from_automate.as_string().expect("Automate result should be a string");
    let parsed_results: Vec<Result<String, String>> = serde_json::from_str(&results_list_json).unwrap_or_else(|err| panic!("Failed to parse automate results: {}", err));

    assert_eq!(parsed_results.len(), 1, "Expected one result from the task list");
    let task_error = parsed_results[0].as_ref().err().expect("Task should have failed with an error");

    assert!(task_error.contains("InvalidSelector"), "Error message '{}' did not contain 'InvalidSelector'", task_error);
    assert!(task_error.contains("Error getting all attributes:"), "Error message should specify 'Error getting all attributes'");
}


// --- New Integration Tests for LLM-suggested commands and {{PREVIOUS_RESULT}} placeholder ---

fn setup_agent_for_integration_tests() -> RustAgent {
    let mut agent = RustAgent::new();
    // It's crucial that mock-llm feature is enabled for these tests
    // The dummy values are fine as mock_llm will intercept the call.
    agent.set_llm_config(
        "http://localhost:1234/mock".to_string(),
        "mock-model".to_string(),
        "mock-api-key".to_string(),
    );
    agent
}

#[wasm_bindgen_test]
async fn test_llm_suggested_multi_step_dom_commands_end_to_end() {
    let agent = setup_agent_for_integration_tests();
    
    // This task string is configured in llm.rs (mock) to return a JSON array of commands
    let task_for_llm = "fill username and password and click login";
    let tasks_json = serde_json::to_string(&vec![task_for_llm]).unwrap();

    // Clear initial state if any (e.g., from previous tests if page is not reloaded)
    if let Some(user_input) = get_html_input_element_by_id_for_test("testuser") { user_input.set_value(""); }
    if let Some(pass_input) = get_html_input_element_by_id_for_test("testpass") { pass_input.set_value(""); }
    if let Some(status_div) = get_html_element_by_id_for_test("loginStatus") { status_div.set_text_content(Some("Not logged in")); }


    let result_js = agent.automate(tasks_json).await.expect("Automate call failed");
    let result_str = result_js.as_string().expect("Result should be a string");
    let results_outer: Vec<Result<String, String>> = serde_json::from_str(&result_str)
        .expect("Failed to parse outer JSON array of results");

    assert_eq!(results_outer.len(), 1, "Expected one overall task result");
    let llm_commands_execution_result_str = results_outer[0].as_ref().expect("Task execution failed");

    // The result from agent.rs run_task for LLM-generated JSON commands is a JSON string of Vec<Result<String, String>>
    let inner_results: Vec<Result<String, String>> = serde_json::from_str(llm_commands_execution_result_str)
        .expect("Failed to parse inner JSON array of command results");
    
    assert_eq!(inner_results.len(), 3, "Expected three DOM command results");

    assert!(inner_results[0].is_ok(), "First command (TYPE username) should be Ok");
    assert_eq!(inner_results[0].as_ref().unwrap(), "Successfully typed 'testuser' in element with selector: 'css:#testuser'");
    
    assert!(inner_results[1].is_ok(), "Second command (TYPE password) should be Ok");
    assert_eq!(inner_results[1].as_ref().unwrap(), "Successfully typed 'testpass' in element with selector: 'css:#testpass'");

    assert!(inner_results[2].is_ok(), "Third command (CLICK login) should be Ok");
    assert_eq!(inner_results[2].as_ref().unwrap(), "Successfully clicked element with selector: 'css:#testloginbtn'");

    // Verify DOM state after commands
    assert_eq!(get_element_value("css:#testuser").unwrap(), "testuser", "Username input value check");
    assert_eq!(get_element_value("css:#testpass").unwrap(), "testpass", "Password input value check");
    assert_eq!(get_element_text("css:#loginStatus").unwrap(), "Login Successful!", "Login status check");
}


#[wasm_bindgen_test]
async fn test_sequential_tasks_with_previous_result_placeholder_end_to_end() {
    let agent = setup_agent_for_integration_tests();

    // Clear initial state
    if let Some(input_field) = get_html_input_element_by_id_for_test("inputfield") { input_field.set_value(""); }

    let tasks = vec![
        "READ css:#dataelement", // This will be handled by parse_dom_command, not LLM
        "TYPE css:#inputfield {{PREVIOUS_RESULT}}" // Also handled by parse_dom_command
    ];
    let tasks_json = serde_json::to_string(&tasks).unwrap();

    let result_js = agent.automate(tasks_json).await.expect("Automate call failed");
    let result_str = result_js.as_string().expect("Result should be a string");
    let results: Vec<Result<String, String>> = serde_json::from_str(&result_str)
        .expect("Failed to parse JSON array of results");

    assert_eq!(results.len(), 2, "Expected two task results");

    // Task 1: READ css:#dataelement
    // Agent 3 (Generic) should be selected by agent.rs agent selection logic
    // if parse_dom_command is used. The result string format includes the agent info.
    assert!(results[0].is_ok(), "First task (READ) should be Ok. Got: {:?}", results[0].as_ref().err());
    let expected_read_output = "Agent 3 (Generic): Text from element 'css:#dataelement': Expected Text for Placeholder Test";
    assert_eq!(results[0].as_ref().unwrap(), expected_read_output, "READ command output mismatch");
    
    // Task 2: TYPE css:#inputfield {{PREVIOUS_RESULT}}
    // The {{PREVIOUS_RESULT}} should be the string from results[0].as_ref().unwrap().
    // However, the placeholder logic in lib.rs uses the direct output of run_task.
    // For a direct DOM command like READ, run_task returns "Agent X: Text from element..."
    // So, the TYPE command will try to type this entire string.
    let expected_typed_text = expected_read_output; // This is what will be typed due to current placeholder logic

    assert!(results[1].is_ok(), "Second task (TYPE) should be Ok. Got: {:?}", results[1].as_ref().err());
    let expected_type_output = format!("Agent 3 (Generic): Successfully typed '{}' in element with selector: 'css:#inputfield'", expected_typed_text);
    assert_eq!(results[1].as_ref().unwrap(), expected_type_output, "TYPE command output mismatch");
    
    // Verify DOM state after commands
    assert_eq!(get_element_value("css:#inputfield").unwrap(), expected_typed_text, "Input field value check after TYPE with placeholder");
}
