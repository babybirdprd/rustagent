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
};

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
