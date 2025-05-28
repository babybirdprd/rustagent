use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{console, Window, Document, Element, HtmlElement, HtmlInputElement, XPathResult, Node, NodeList};
use serde_json; // Added for JSON serialization

// Helper function to get window and document
fn get_window_document() -> Result<(Window, Document), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("Failed to get window object"))?;
    let document = window.document().ok_or_else(|| JsValue::from_str("Failed to get document object"))?;
    Ok((window, document))
}

// Helper function to get an element using XPath
fn get_element_by_xpath_logic(document: &Document, xpath: &str, original_selector: &str) -> Result<Element, JsValue> {
    let result = document
        .evaluate(xpath, &document, None, XPathResult::FIRST_ORDERED_NODE_TYPE, None)
        .map_err(|e| JsValue::from_str(&format!("InvalidSelector: Invalid XPath expression '{}'. Details: {:?}", original_selector, e.as_string().unwrap_or_else(|| "Unknown XPath error".to_string()))))?;

    match result.single_node_value() {
        Ok(Some(node)) => {
            node.dyn_into::<Element>()
                .map_err(|_| JsValue::from_str(&format!("ElementTypeError: XPath selector '{}' resulted in a Node that is not an Element.", original_selector)))
        }
        Ok(None) => Err(JsValue::from_str(&format!("ElementNotFound: No element found for XPath selector '{}'", original_selector))),
        Err(e) => Err(JsValue::from_str(&format!("InternalError: Error retrieving single node for XPath '{}'. Details: {:?}", original_selector, e.as_string().unwrap_or_else(|| "Unknown node retrieval error".to_string())))),
    }
}

// Unified helper function to get an element by CSS selector or XPath
fn get_element(document: &Document, original_selector: &str) -> Result<Element, JsValue> {
    if original_selector.starts_with("xpath:") {
        let xpath = original_selector.strip_prefix("xpath:").unwrap_or(original_selector);
        console::log_1(&format!("Using XPath selector: {}", xpath).into());
        get_element_by_xpath_logic(document, xpath, original_selector)
    } else {
        let css_selector_to_use;
        if original_selector.starts_with("css:") {
            css_selector_to_use = original_selector.strip_prefix("css:").unwrap_or(original_selector);
            console::log_1(&format!("Using CSS selector: {}", css_selector_to_use).into());
        } else {
            // Default to CSS selector for backward compatibility
            css_selector_to_use = original_selector;
            console::log_1(&format!("Defaulting to CSS selector: {}", css_selector_to_use).into());
        }
        document
            .query_selector(css_selector_to_use)
            .map_err(|e| JsValue::from_str(&format!("InvalidSelector: Invalid CSS selector '{}'. Details: {:?}", original_selector, e.as_string().unwrap_or_else(|| "Unknown querySelector error".to_string()))))?
            .ok_or_else(|| JsValue::from_str(&format!("ElementNotFound: No element found for CSS selector '{}'", original_selector)))
    }
}

// Helper function to get multiple elements using XPath
fn get_elements_by_xpath_logic(document: &Document, xpath: &str, original_selector: &str) -> Result<Vec<Element>, JsValue> {
    let result = document
        .evaluate(xpath, &document, None, XPathResult::ORDERED_NODE_ITERATOR_TYPE, None)
        .map_err(|e| JsValue::from_str(&format!("InvalidSelector: Invalid XPath expression '{}'. Details: {:?}", original_selector, e.as_string().unwrap_or_else(|| "Unknown XPath error".to_string()))))?;

    let mut elements = Vec::new();
    while let Ok(Some(node)) = result.iterate_next() {
        if let Some(element) = node.dyn_ref::<Element>() {
            elements.push(element.clone());
        } else {
            // Log or handle nodes that are not elements if necessary
            console::warn_1(&format!("XPath selector '{}' returned a Node that is not an Element.", original_selector).into());
        }
    }
    if elements.is_empty() {
         // Check if this should be an error or an empty vec is acceptable if no elements match
        // For get_all_elements_attributes, an empty list of attributes is valid if no elements match.
        // However, if the XPath itself was valid but found no matching elements, an empty Vec is correct.
        // The single get_element_by_xpath_logic returns ElementNotFound, this one returns Ok(empty_vec)
    }
    Ok(elements)
}

// Unified helper function to get all elements by CSS selector or XPath
fn get_all_elements(document: &Document, original_selector: &str) -> Result<Vec<Element>, JsValue> {
    if original_selector.starts_with("xpath:") {
        let xpath = original_selector.strip_prefix("xpath:").unwrap_or(original_selector);
        console::log_1(&format!("Using XPath selector for all elements: {}", xpath).into());
        get_elements_by_xpath_logic(document, xpath, original_selector)
    } else {
        let css_selector_to_use;
        if original_selector.starts_with("css:") {
            css_selector_to_use = original_selector.strip_prefix("css:").unwrap_or(original_selector);
            console::log_1(&format!("Using CSS selector for all elements: {}", css_selector_to_use).into());
        } else {
            css_selector_to_use = original_selector;
            console::log_1(&format!("Defaulting to CSS selector for all elements: {}", css_selector_to_use).into());
        }
        let node_list: NodeList = document
            .query_selector_all(css_selector_to_use)
            .map_err(|e| JsValue::from_str(&format!("InvalidSelector: Invalid CSS selector '{}'. Details: {:?}", original_selector, e.as_string().unwrap_or_else(|| "Unknown querySelectorAll error".to_string()))))?;
        
        let mut elements = Vec::new();
        for i in 0..node_list.length() {
            if let Some(node) = node_list.item(i) {
                if let Some(element) = node.dyn_ref::<Element>() {
                    elements.push(element.clone());
                }
            }
        }
        // If no elements are found by query_selector_all, it returns an empty NodeList, 
        // resulting in an empty Vec<Element>, which is acceptable.
        Ok(elements)
    }
}


#[wasm_bindgen]
pub fn click_element(selector: &str) -> Result<(), JsValue> {
    console::log_1(&format!("Attempting to click element with selector: {}", selector).into());
    let (_window, document) = get_window_document()?;
    
    let element = get_element(&document, selector)?;

    let html_element = element
        .dyn_into::<HtmlElement>()
        .map_err(|_| JsValue::from_str(&format!("ElementTypeError: Element for selector '{}' is not an HTMLElement, cannot click.", selector)))?;
    
    html_element
        .click();
        
    console::log_1(&format!("Successfully clicked element with selector: {}", selector).into());
    Ok(())
}

#[wasm_bindgen]
pub fn type_in_element(selector: &str, text: &str) -> Result<(), JsValue> {
    console::log_1(&format!("Attempting to type '{}' in element with selector: {}", text, selector).into());
    let (_window, document) = get_window_document()?;

    let element = get_element(&document, selector)?;

    let input_element = element
        .dyn_into::<HtmlInputElement>()
        .map_err(|_| JsValue::from_str(&format!("ElementTypeError: Element for selector '{}' is not an input element.", selector)))?;

    input_element.set_value(text);
    
    console::log_1(&format!("Successfully typed '{}' in element with selector: {}", text, selector).into());
    Ok(())
}

#[wasm_bindgen]
pub fn get_element_text(selector: &str) -> Result<String, JsValue> {
    console::log_1(&format!("Attempting to get text from element with selector: {}", selector).into());
    let (_window, document) = get_window_document()?;

    let element = get_element(&document, selector)?;

    let html_element = element
        .dyn_into::<HtmlElement>()
        .map_err(|_| JsValue::from_str(&format!("ElementTypeError: Element for selector '{}' is not an HtmlElement, cannot get text.", selector)))?;
    
    console::log_1(&format!("Successfully retrieved text from element with selector: {}", selector).into());
    Ok(html_element.inner_text())
}

#[wasm_bindgen]
pub fn get_element_value(selector: &str) -> Result<String, JsValue> {
    console::log_1(&format!("Attempting to get value from input element with selector: {}", selector).into());
    let (_window, document) = get_window_document()?;
    
    let element = get_element(&document, selector)?;

    let input_element = element
        .dyn_into::<HtmlInputElement>()
        .map_err(|_| JsValue::from_str(&format!("ElementTypeError: Element for selector '{}' is not an input element.", selector)))?;
    
    console::log_1(&format!("Successfully retrieved value from element with selector: {}", selector).into());
    Ok(input_element.value())
}

#[wasm_bindgen]
pub fn get_element_attribute(selector: &str, attribute_name: &str) -> Result<String, JsValue> {
    console::log_1(&format!("Attempting to get attribute '{}' from element with selector: {}", attribute_name, selector).into());
    let (_window, document) = get_window_document()?;
    let element = get_element(&document, selector)?;

    match element.get_attribute(attribute_name) {
        Some(value) => {
            console::log_1(&format!("Successfully retrieved attribute '{}' with value '{}' from element with selector: {}", attribute_name, value, selector).into());
            Ok(value)
        }
        None => {
            Err(JsValue::from_str(&format!("AttributeNotFound: Attribute '{}' not found on element with selector '{}'", attribute_name, selector)))
        }
    }
}

#[wasm_bindgen]
pub fn set_element_attribute(selector: &str, attribute_name: &str, attribute_value: &str) -> Result<(), JsValue> {
    console::log_1(&format!("Attempting to set attribute '{}' to '{}' for element with selector: {}", attribute_name, attribute_value, selector).into());
    let (_window, document) = get_window_document()?;
    let element = get_element(&document, selector)?;

    element.set_attribute(attribute_name, attribute_value)
        .map_err(|e| {
            JsValue::from_str(&format!("SetAttributeError: Failed to set attribute '{}' on element with selector '{}'. Details: {:?}", attribute_name, selector, e.as_string().unwrap_or_else(|| "Unknown set_attribute error".to_string())))
        })?;
    
    console::log_1(&format!("Successfully set attribute '{}' to '{}' for element with selector: {}", attribute_name, attribute_value, selector).into());
    Ok(())
}

#[wasm_bindgen]
pub fn select_dropdown_option(selector: &str, value: &str) -> Result<(), JsValue> {
    console::log_1(&format!("Attempting to select option with value '{}' for dropdown with selector: {}", value, selector).into());
    let (_window, document) = get_window_document()?;
    let element = get_element(&document, selector)?;

    let select_element = element
        .dyn_into::<web_sys::HtmlSelectElement>()
        .map_err(|_| JsValue::from_str(&format!("ElementTypeError: Element for selector '{}' is not a select element.", selector)))?;
    
    select_element.set_value(value);
    
    console::log_1(&format!("Successfully selected option with value '{}' for dropdown with selector: {}", value, selector).into());
    Ok(())
}

#[wasm_bindgen]
pub fn get_all_elements_attributes(selector: &str, attribute_name: &str) -> Result<String, JsValue> {
    console::log_1(&format!("Attempting to get attribute '{}' from all elements matching selector: {}", attribute_name, selector).into());
    let (_window, document) = get_window_document()?;
    
    let elements = get_all_elements(&document, selector)?;
    
    if elements.is_empty() {
        // If no elements are found, it's not an error; return an empty list JSON.
        console::log_1(&format!("No elements found for selector '{}'. Returning empty list.", selector).into());
        return Ok("[]".to_string());
    }

    let mut attributes_vec: Vec<Option<String>> = Vec::new();
    for element in elements {
        attributes_vec.push(element.get_attribute(attribute_name));
    }

    let json_string = serde_json::to_string(&attributes_vec)
        .map_err(|e| JsValue::from_str(&format!("SerializationError: Failed to serialize attributes to JSON. Details: {}", e)))?;
    
    console::log_1(&format!("Successfully retrieved attributes for selector '{}', attribute '{}'. Count: {}", selector, attribute_name, attributes_vec.len()).into());
    Ok(json_string)
}


#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    // Helper to create and append element for testing
    fn setup_element(document: &Document, id: &str, tag: &str, attributes: Option<Vec<(&str, &str)>>) -> Element {
        let el = document.create_element(tag).unwrap();
        el.set_id(id);
        if let Some(attrs) = attributes {
            for (key, value) in attrs {
                el.set_attribute(key, value).unwrap();
            }
        }
        document.body().unwrap().append_child(&el).unwrap();
        el
    }

    // Helper to clean up element
    fn cleanup_element(element: Element) {
        element.remove();
    }

    #[wasm_bindgen_test]
    fn test_get_element_css_selector_no_element() {
        let result = get_element_attribute("css:#nonexistent", "value"); // Uses get_element internally
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().as_string().unwrap_or_default(), "ElementNotFound: No element found for CSS selector 'css:#nonexistent'");
    }

    #[wasm_bindgen_test]
    fn test_get_element_default_css_selector_no_element() {
        let result = get_element_attribute("#nonexistent_default", "value"); // Uses get_element internally
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().as_string().unwrap_or_default(), "ElementNotFound: No element found for CSS selector '#nonexistent_default'");
    }
    
    #[wasm_bindgen_test]
    fn test_get_element_xpath_selector_no_element() {
        let result = get_element_attribute("xpath://div[@id='nonexistent_xpath']", "value"); // Uses get_element internally
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().as_string().unwrap_or_default(), "ElementNotFound: No element found for XPath selector 'xpath://div[@id='nonexistent_xpath']'");
    }

    #[wasm_bindgen_test]
    fn test_get_element_xpath_invalid_xpath() {
        let result = get_element_attribute("xpath://[invalid", "value");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().as_string().unwrap_or_default();
        assert!(err_msg.starts_with("InvalidSelector: Invalid XPath expression 'xpath://[invalid'. Details:"));
    }

    #[wasm_bindgen_test]
    fn test_type_in_element_wrong_type() {
        // Setup: Create a div, not an input. This requires DOM manipulation.
        // This test is conceptual if we can't easily create DOM elements in wasm-bindgen-test.
        // Assuming such a setup, the error would be:
        // let result = type_in_element("css:#not_an_input_div", "test");
        // assert!(result.is_err());
        // assert_eq!(result.unwrap_err().as_string().unwrap_or_default(), "ElementTypeError: Element for selector 'css:#not_an_input_div' is not an input element.");
        
        // For now, test the path that would lead to this error if an element was found but was wrong type.
        // We can simulate this by directly testing get_element and then a dyn_into failure.
        // Since get_element will fail first if element not found, this specific test is hard to isolate perfectly
        // without a live DOM and a non-input element.
        // The error message check on `type_in_element` for non-existent element will cover `get_element`'s error.
        // A more direct test would be to create a div and try to type in it.
    }

    #[wasm_bindgen_test]
    fn test_get_element_attribute_not_found() {
        // This test requires an element to exist first. For now, we test the error message from get_element.
        let result = get_element_attribute("css:#nonexistent_for_attr_test", "data-nonexistent");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().as_string().unwrap_or_default(), "ElementNotFound: No element found for CSS selector 'css:#nonexistent_for_attr_test'");
        // If the element *was* found, and then the attribute was not, the message would be:
        // "AttributeNotFound: Attribute 'data-nonexistent' not found on element with selector 'css:#actual_id_here'"
    }
    
    // The existing tests for no_element implicitly test the ElementNotFound messages from get_element.
    // We can refine them to check the exact string.

    #[wasm_bindgen_test]
    fn test_get_element_attribute_no_element_refined() {
        let result_css = get_element_attribute("css:#nonexistent_attr", "value");
        assert!(result_css.is_err());
        assert_eq!(result_css.unwrap_err().as_string().unwrap_or_default(), "ElementNotFound: No element found for CSS selector 'css:#nonexistent_attr'");

        let result_xpath = get_element_attribute("xpath://*[@id='nonexistent_attr_xpath']", "value");
        assert!(result_xpath.is_err());
        assert_eq!(result_xpath.unwrap_err().as_string().unwrap_or_default(), "ElementNotFound: No element found for XPath selector 'xpath://*[@id='nonexistent_attr_xpath']'");
    }

    #[wasm_bindgen_test]
    fn test_set_element_attribute_no_element_refined() {
        let result_css = set_element_attribute("css:#nonexistent_set_attr", "value", "test");
        assert!(result_css.is_err());
        assert_eq!(result_css.unwrap_err().as_string().unwrap_or_default(), "ElementNotFound: No element found for CSS selector 'css:#nonexistent_set_attr'");

        let result_xpath = set_element_attribute("xpath://*[@id='nonexistent_set_attr_xpath']", "value", "test");
        assert!(result_xpath.is_err());
        assert_eq!(result_xpath.unwrap_err().as_string().unwrap_or_default(), "ElementNotFound: No element found for XPath selector 'xpath://*[@id='nonexistent_set_attr_xpath']'");
    }

    #[wasm_bindgen_test]
    fn test_select_dropdown_option_no_element_refined() {
        let result_css = select_dropdown_option("css:#nonexistent_select", "option_value");
        assert!(result_css.is_err());
        assert_eq!(result_css.unwrap_err().as_string().unwrap_or_default(), "ElementNotFound: No element found for CSS selector 'css:#nonexistent_select'");

        let result_xpath = select_dropdown_option("xpath://select[@id='nonexistent_select_xpath']", "option_value");
        assert!(result_xpath.is_err());
        assert_eq!(result_xpath.unwrap_err().as_string().unwrap_or_default(), "ElementNotFound: No element found for XPath selector 'xpath://select[@id='nonexistent_select_xpath']'");
    }

    // To test success cases, you'd typically need to set up a DOM environment.
    // For example, by appending elements to the test body:
    // ```
    // #[wasm_bindgen_test]
    // fn test_get_element_attribute_success() {
    //     let (_window, document) = get_window_document().unwrap();
    //     let body = document.body().unwrap();
    //     let el = document.create_element("input").unwrap();
    //     el.set_id("test_input_attr");
    //     el.set_attribute("data-test", "hello").unwrap();
    //     body.append_child(&el).unwrap();
    //
    //     let result = get_element_attribute("#test_input_attr", "data-test");
    //     assert!(result.is_ok());
    //     assert_eq!(result.unwrap(), "hello");
    //
    //     el.remove(); // Clean up
    // }
    // ```
    // However, these tests might be flaky or require more setup depending on the test runner's capabilities.
    // The current subtask focuses on implementing the functions and basic error path checks.

    #[wasm_bindgen_test]
    fn test_get_all_elements_attributes_css_no_elements_found() {
        let result = get_all_elements_attributes("css:.nonexistent-class", "data-test");
        assert!(result.is_ok(), "Expected Ok for no elements found, got {:?}", result.err());
        assert_eq!(result.unwrap(), "[]");
    }

    #[wasm_bindgen_test]
    fn test_get_all_elements_attributes_xpath_no_elements_found() {
        let result = get_all_elements_attributes("xpath://div[@class='nonexistent-class-xpath']", "data-test");
        assert!(result.is_ok(), "Expected Ok for no elements found, got {:?}", result.err());
        assert_eq!(result.unwrap(), "[]");
    }
    
    #[wasm_bindgen_test]
    fn test_get_all_elements_attributes_css_single_element_with_attribute() {
        let (_window, document) = get_window_document().unwrap();
        let el = setup_element(&document, "single-css", "div", Some(vec![("data-test", "value1")]));

        let result = get_all_elements_attributes("css:#single-css", "data-test");
        assert!(result.is_ok(), "Error: {:?}", result.err());
        assert_eq!(result.unwrap(), "[\"value1\"]");
        
        cleanup_element(el);
    }

    #[wasm_bindgen_test]
    fn test_get_all_elements_attributes_xpath_single_element_with_attribute() {
        let (_window, document) = get_window_document().unwrap();
        let el = setup_element(&document, "single-xpath", "div", Some(vec![("data-test", "value-xpath")]));

        let result = get_all_elements_attributes("xpath://div[@id='single-xpath']", "data-test");
        assert!(result.is_ok(), "Error: {:?}", result.err());
        assert_eq!(result.unwrap(), "[\"value-xpath\"]");
        
        cleanup_element(el);
    }

    #[wasm_bindgen_test]
    fn test_get_all_elements_attributes_multiple_elements_some_with_attr() {
        let (_window, document) = get_window_document().unwrap();
        let el1 = setup_element(&document, "multi1", "span", Some(vec![("class", "target-multi"), ("data-id", "1")]));
        let el2 = setup_element(&document, "multi2", "span", Some(vec![("class", "target-multi")])); // No data-id
        let el3 = setup_element(&document, "multi3", "span", Some(vec![("class", "target-multi"), ("data-id", "3")]));
        
        let result = get_all_elements_attributes("css:.target-multi", "data-id");
        assert!(result.is_ok(), "Error: {:?}", result.err());
        assert_eq!(result.unwrap(), "[\"1\",null,\"3\"]"); // serde_json serializes Option<String>::None as null

        cleanup_element(el1);
        cleanup_element(el2);
        cleanup_element(el3);
    }

    #[wasm_bindgen_test]
    fn test_get_all_elements_attributes_xpath_multiple_elements() {
        let (_window, document) = get_window_document().unwrap();
        let el1 = setup_element(&document, "xpath-multi1", "a", Some(vec![("href", "/page1"), ("data-common", "val") ]));
        let el2 = setup_element(&document, "xpath-multi2", "a", Some(vec![("data-common", "val")])); // No href
        let el3 = setup_element(&document, "xpath-multi3", "a", Some(vec![("href", "/page3"), ("data-common", "val")]));
        
        // Using XPath to select all 'a' elements with 'data-common' attribute
        let result = get_all_elements_attributes("xpath://a[@data-common='val']", "href");
        assert!(result.is_ok(), "Error: {:?}", result.err());
        // Order depends on document order, assuming they are appended in order el1, el2, el3
        assert_eq!(result.unwrap(), "[\"/page1\",null,\"/page3\"]");

        cleanup_element(el1);
        cleanup_element(el2);
        cleanup_element(el3);
    }

    #[wasm_bindgen_test]
    fn test_get_all_elements_attributes_invalid_css_selector() {
        let result = get_all_elements_attributes("css:[invalid-selector", "data-test");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().as_string().unwrap_or_default();
        assert!(err_msg.starts_with("InvalidSelector: Invalid CSS selector 'css:[invalid-selector'. Details:"));
    }

    #[wasm_bindgen_test]
    fn test_get_all_elements_attributes_invalid_xpath_selector() {
        let result = get_all_elements_attributes("xpath://[invalid-xpath", "data-test");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().as_string().unwrap_or_default();
        assert!(err_msg.starts_with("InvalidSelector: Invalid XPath expression 'xpath://[invalid-xpath'. Details:"));
    }
}
