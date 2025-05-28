use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{console, Window, Document, Element, HtmlElement, HtmlInputElement, XPathResult, NodeList}; // Removed Node
use serde_json; // Added for JSON serialization
use std::fmt;
use gloo_timers::future::{TimeoutFuture, IntervalStream};
use futures_util::stream::StreamExt; // For IntervalStream.next()
use futures::future::{select, Either}; // For select pattern

#[derive(Debug, PartialEq)]
pub enum DomError {
    ElementNotFound { selector: String, message: Option<String> },
    InvalidSelector { selector: String, error: String },
    ElementTypeError { selector: String, expected_type: String },
    AttributeNotFound { selector: String, attribute_name: String },
    SerializationError { message: String },
    JsError { message: String },
}

impl fmt::Display for DomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomError::ElementNotFound { selector, message } => {
                if let Some(msg) = message {
                    write!(f, "{}", msg) // The message from wait_for_element will be complete
                } else {
                    write!(f, "ElementNotFound: No element found for selector '{}'", selector)
                }
            }
            DomError::InvalidSelector { selector, error } => write!(f, "InvalidSelector: Invalid selector '{}'. Details: {}", selector, error),
            DomError::ElementTypeError { selector, expected_type } => write!(f, "ElementTypeError: Element for selector '{}' is not of expected type '{}'", selector, expected_type),
            DomError::AttributeNotFound { selector, attribute_name } => write!(f, "AttributeNotFound: Attribute '{}' not found on element with selector '{}'", attribute_name, selector),
            DomError::SerializationError { message } => write!(f, "SerializationError: {}", message),
            DomError::JsError { message } => write!(f, "JsError: {}", message),
        }
    }
}

#[wasm_bindgen]
pub fn element_exists(selector: &str) -> Result<bool, DomError> {
    let (_window, document) = get_window_document()?;
    match get_element(&document, selector) {
        Ok(_) => Ok(true),
        Err(DomError::ElementNotFound { .. }) => Ok(false), // Specifically consume the error if it's ElementNotFound
        Err(e) => Err(e), // Propagate other errors (e.g., InvalidSelector)
    }
}

impl From<JsValue> for DomError {
    fn from(value: JsValue) -> Self {
        DomError::JsError {
            message: value.as_string().unwrap_or_else(|| "Unknown JsValue error".to_string()),
        }
    }
}

impl Into<JsValue> for DomError {
    fn into(self) -> JsValue {
        JsValue::from_str(&self.to_string())
    }
}

// Helper function to get window and document
fn get_window_document() -> Result<(Window, Document), DomError> {
    let window = web_sys::window().ok_or_else(|| DomError::JsError { message: "Failed to get window object".to_string() })?;
    let document = window.document().ok_or_else(|| DomError::JsError { message: "Failed to get document object".to_string() })?;
    Ok((window, document))
}

// Helper function to get an element using XPath
fn get_element_by_xpath_logic(document: &Document, xpath: &str, original_selector: &str) -> Result<Element, DomError> {
    let result = document
        .evaluate(xpath, &document) // Corrected as per compiler suggestion
        .map_err(|e| DomError::InvalidSelector {
            selector: original_selector.to_string(),
            error: e.as_string().unwrap_or_else(|| "Unknown XPath error".to_string()),
        })?;

    match result.single_node_value() {
        Ok(Some(node)) => {
            node.dyn_into::<Element>()
                .map_err(|_| DomError::ElementTypeError {
                    selector: original_selector.to_string(),
                    expected_type: "Element".to_string(),
                })
        }
    Ok(None) => Err(DomError::ElementNotFound { selector: original_selector.to_string(), message: None }),
        Err(e) => Err(DomError::JsError {
            message: format!("Error retrieving single node for XPath '{}'. Details: {:?}", original_selector, e.as_string().unwrap_or_else(|| "Unknown node retrieval error".to_string())),
        }),
    }
}

// Unified helper function to get an element by CSS selector or XPath
fn get_element(document: &Document, original_selector: &str) -> Result<Element, DomError> {
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
            .map_err(|e| DomError::InvalidSelector {
                selector: original_selector.to_string(),
                error: e.as_string().unwrap_or_else(|| "Unknown querySelector error".to_string()),
            })?
            .ok_or_else(|| DomError::ElementNotFound { selector: original_selector.to_string(), message: None })
    }
}

// Helper function to get multiple elements using XPath
fn get_elements_by_xpath_logic(document: &Document, xpath: &str, original_selector: &str) -> Result<Vec<Element>, DomError> {
    let result = document
        .evaluate(xpath, &document) // Corrected as per compiler suggestion
        .map_err(|e| DomError::InvalidSelector {
            selector: original_selector.to_string(),
            error: e.as_string().unwrap_or_else(|| "Unknown XPath error".to_string()),
        })?;

    let mut elements = Vec::new();
    while let Ok(Some(node)) = result.iterate_next() {
        if let Some(element) = node.dyn_ref::<Element>() {
            elements.push(element.clone());
        } else {
            console::warn_1(&format!("XPath selector '{}' returned a Node that is not an Element.", original_selector).into());
        }
    }
    Ok(elements)
}

// Unified helper function to get all elements by CSS selector or XPath
fn get_all_elements(document: &Document, original_selector: &str) -> Result<Vec<Element>, DomError> {
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
            .map_err(|e| DomError::InvalidSelector {
                selector: original_selector.to_string(),
                error: e.as_string().unwrap_or_else(|| "Unknown querySelectorAll error".to_string()),
            })?;
        
        let mut elements = Vec::new();
        for i in 0..node_list.length() {
            if let Some(node) = node_list.item(i) {
                if let Some(element) = node.dyn_ref::<Element>() {
                    elements.push(element.clone());
                }
            }
        }
        Ok(elements)
    }
}


#[wasm_bindgen]
pub fn click_element(selector: &str) -> Result<(), DomError> {
    console::log_1(&format!("Attempting to click element with selector: {}", selector).into());
    let (_window, document) = get_window_document()?;
    
    let element = get_element(&document, selector)?;

    let html_element = element
        .dyn_into::<HtmlElement>()
        .map_err(|_| DomError::ElementTypeError {
            selector: selector.to_string(),
            expected_type: "HtmlElement".to_string(),
        })?;
    
    html_element.click();
        
    console::log_1(&format!("Successfully clicked element with selector: {}", selector).into());
    Ok(())
}

#[wasm_bindgen]
pub fn type_in_element(selector: &str, text: &str) -> Result<(), DomError> {
    console::log_1(&format!("Attempting to type '{}' in element with selector: {}", text, selector).into());
    let (_window, document) = get_window_document()?;

    let element = get_element(&document, selector)?;

    let input_element = element
        .dyn_into::<HtmlInputElement>()
        .map_err(|_| DomError::ElementTypeError {
            selector: selector.to_string(),
            expected_type: "HtmlInputElement".to_string(),
        })?;

    input_element.set_value(text);
    
    console::log_1(&format!("Successfully typed '{}' in element with selector: {}", text, selector).into());
    Ok(())
}

#[wasm_bindgen]
pub fn get_element_text(selector: &str) -> Result<String, DomError> {
    console::log_1(&format!("Attempting to get text from element with selector: {}", selector).into());
    let (_window, document) = get_window_document()?;

    let element = get_element(&document, selector)?;

    let html_element = element
        .dyn_into::<HtmlElement>()
        .map_err(|_| DomError::ElementTypeError {
            selector: selector.to_string(),
            expected_type: "HtmlElement".to_string(),
        })?;
    
    console::log_1(&format!("Successfully retrieved text from element with selector: {}", selector).into());
    Ok(html_element.inner_text())
}

#[wasm_bindgen]
pub fn get_element_value(selector: &str) -> Result<String, DomError> {
    console::log_1(&format!("Attempting to get value from input element with selector: {}", selector).into());
    let (_window, document) = get_window_document()?;
    
    let element = get_element(&document, selector)?;

    let input_element = element
        .dyn_into::<HtmlInputElement>()
        .map_err(|_| DomError::ElementTypeError {
            selector: selector.to_string(),
            expected_type: "HtmlInputElement".to_string(),
        })?;
    
    console::log_1(&format!("Successfully retrieved value from element with selector: {}", selector).into());
    Ok(input_element.value())
}

#[wasm_bindgen]
pub fn get_element_attribute(selector: &str, attribute_name: &str) -> Result<String, DomError> {
    console::log_1(&format!("Attempting to get attribute '{}' from element with selector: {}", attribute_name, selector).into());
    let (_window, document) = get_window_document()?;
    let element = get_element(&document, selector)?;

    match element.get_attribute(attribute_name) {
        Some(value) => {
            console::log_1(&format!("Successfully retrieved attribute '{}' with value '{}' from element with selector: {}", attribute_name, value, selector).into());
            Ok(value)
        }
        None => Err(DomError::AttributeNotFound {
            selector: selector.to_string(),
            attribute_name: attribute_name.to_string(),
        }),
    }
}

#[wasm_bindgen]
pub async fn wait_for_element(selector: &str, timeout_ms: Option<u32>) -> Result<(), DomError> {
    const DEFAULT_TIMEOUT_MS: u32 = 5000;
    const INTERVAL_MS: u32 = 100; // Polling interval
    let timeout_duration = timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);

    let main_future = async {
        let mut interval = IntervalStream::new(INTERVAL_MS);
        loop {
            match element_exists(selector) {
                Ok(true) => return Ok(()),
                Ok(false) => { /* continue polling */ }
                // ElementNotFound is handled by element_exists returning Ok(false)
                // Other errors from element_exists (like InvalidSelector) should propagate
                Err(e) => return Err(e), 
            }
            StreamExt::next(&mut interval).await; // Corrected: Use StreamExt::next for IntervalStream
        }
    };

    let timeout_event = TimeoutFuture::new(timeout_duration);

    match select(Box::pin(main_future), timeout_event).await {
        Either::Left((Ok(()), _)) => Ok(()), // main_future completed successfully
        Either::Left((Err(e), _)) => Err(e),  // main_future returned an error
        Either::Right((_, _)) => Err(DomError::ElementNotFound { // timeout_event completed first
            selector: selector.to_string(),
            message: Some(format!("Element '{}' not found after {}ms timeout", selector, timeout_duration)),
        }),
    }
}


#[wasm_bindgen]
pub fn set_element_attribute(selector: &str, attribute_name: &str, attribute_value: &str) -> Result<(), DomError> {
    console::log_1(&format!("Attempting to set attribute '{}' to '{}' for element with selector: {}", attribute_name, attribute_value, selector).into());
    let (_window, document) = get_window_document()?;
    let element = get_element(&document, selector)?;

    element.set_attribute(attribute_name, attribute_value)
        .map_err(|e| DomError::JsError { // set_attribute can fail if attribute name is invalid
            message: format!("Failed to set attribute '{}' on element with selector '{}'. Details: {:?}", attribute_name, selector, e.as_string().unwrap_or_else(|| "Unknown set_attribute error".to_string())),
        })?;
    
    console::log_1(&format!("Successfully set attribute '{}' to '{}' for element with selector: {}", attribute_name, attribute_value, selector).into());
    Ok(())
}

#[wasm_bindgen]
pub fn select_dropdown_option(selector: &str, value: &str) -> Result<(), DomError> {
    console::log_1(&format!("Attempting to select option with value '{}' for dropdown with selector: {}", value, selector).into());
    let (_window, document) = get_window_document()?;
    let element = get_element(&document, selector)?;

    let select_element = element
        .dyn_into::<web_sys::HtmlSelectElement>()
        .map_err(|_| DomError::ElementTypeError {
            selector: selector.to_string(),
            expected_type: "HtmlSelectElement".to_string(),
        })?;
    
    select_element.set_value(value);
    
    console::log_1(&format!("Successfully selected option with value '{}' for dropdown with selector: {}", value, selector).into());
    Ok(())
}

#[wasm_bindgen]
pub fn get_all_elements_attributes(selector: &str, attribute_name: &str) -> Result<String, DomError> {
    console::log_1(&format!("Attempting to get attribute '{}' from all elements matching selector: {}", attribute_name, selector).into());
    let (_window, document) = get_window_document()?;
    
    let elements = get_all_elements(&document, selector)?;
    
    if elements.is_empty() {
        console::log_1(&format!("No elements found for selector '{}'. Returning empty list.", selector).into());
        return Ok("[]".to_string());
    }

    let mut attributes_vec: Vec<Option<String>> = Vec::new();
    for element in elements {
        attributes_vec.push(element.get_attribute(attribute_name));
    }

    let json_string = serde_json::to_string(&attributes_vec)
        .map_err(|e| DomError::SerializationError { message: format!("Failed to serialize attributes to JSON. Details: {}", e) })?;
    
    console::log_1(&format!("Successfully retrieved attributes for selector '{}', attribute '{}'. Count: {}", selector, attribute_name, attributes_vec.len()).into());
    Ok(json_string)
}

#[wasm_bindgen]
pub fn get_current_url() -> Result<String, DomError> {
    let (window, _) = get_window_document()?;
    match window.location().href() {
        Ok(href) => Ok(href),
        Err(js_val) => Err(DomError::JsError {
            message: format!("Failed to get URL: {:?}", js_val.as_string().unwrap_or_else(|| "Unknown JS error".to_string())),
        }),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    use wasm_bindgen::JsValue;
    use futures::future::ready; // For simulating delays

    wasm_bindgen_test_configure!(run_in_browser);

    #[test]
    fn test_dom_error_display() {
        assert_eq!(
            DomError::ElementNotFound { selector: "test".to_string(), message: None }.to_string(),
            "ElementNotFound: No element found for selector 'test'"
        );
        assert_eq!(
            DomError::ElementNotFound { selector: "test".to_string(), message: Some("Custom message".to_string()) }.to_string(),
            "Custom message"
        );
        assert_eq!(
            DomError::InvalidSelector { selector: "test".to_string(), error: "details".to_string() }.to_string(),
            "InvalidSelector: Invalid selector 'test'. Details: details"
        );
        assert_eq!(
            DomError::ElementTypeError { selector: "test".to_string(), expected_type: "div".to_string() }.to_string(),
            "ElementTypeError: Element for selector 'test' is not of expected type 'div'"
        );
        assert_eq!(
            DomError::AttributeNotFound { selector: "test".to_string(), attribute_name: "href".to_string() }.to_string(),
            "AttributeNotFound: Attribute 'href' not found on element with selector 'test'"
        );
        assert_eq!(
            DomError::SerializationError { message: "json error".to_string() }.to_string(),
            "SerializationError: json error"
        );
        assert_eq!(
            DomError::JsError { message: "js error".to_string() }.to_string(),
            "JsError: js error"
        );
    }

    #[test]
    fn test_dom_error_into_js_value() {
        let error = DomError::ElementNotFound { selector: "test".to_string(), message: None };
        let js_value: JsValue = error.into();
        assert_eq!(js_value.as_string().unwrap(), "ElementNotFound: No element found for selector 'test'");
    }

    #[test]
    fn test_dom_error_from_js_value() {
        let js_value_error = JsValue::from_str("generic js error");
        let dom_error: DomError = js_value_error.into();
        match dom_error {
            DomError::JsError { message } => assert_eq!(message, "generic js error"),
            _ => panic!("Incorrect DomError variant from JsValue"),
        }
    }

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

    // Helper to assert DomError equality, converting JsValue back to DomError string for comparison
    fn assert_dom_error_eq(result: Result<String, DomError>, expected_error: DomError) {
        match result {
            Ok(_) => panic!("Expected error {:?}, but got Ok", expected_error),
            Err(e) => assert_eq!(e, expected_error, "Error mismatch. Expected: {}, Got: {}", expected_error.to_string(), e.to_string()),
        }
    }
    
    fn assert_dom_error_eq_unit(result: Result<(), DomError>, expected_error: DomError) {
        match result {
            Ok(_) => panic!("Expected error {:?}, but got Ok", expected_error),
            Err(e) => assert_eq!(e, expected_error, "Error mismatch. Expected: {}, Got: {}", expected_error.to_string(), e.to_string()),
        }
    }


    #[wasm_bindgen_test]
    fn test_get_element_css_selector_no_element() {
        let result = get_element_attribute("css:#nonexistent", "value");
        assert_dom_error_eq(result, DomError::ElementNotFound { selector: "css:#nonexistent".to_string(), message: None });
    }

    #[wasm_bindgen_test]
    fn test_get_element_default_css_selector_no_element() {
        let result = get_element_attribute("#nonexistent_default", "value");
        assert_dom_error_eq(result, DomError::ElementNotFound { selector: "#nonexistent_default".to_string(), message: None });
    }
    
    #[wasm_bindgen_test]
    fn test_get_element_xpath_selector_no_element() {
        let result = get_element_attribute("xpath://div[@id='nonexistent_xpath']", "value");
        assert_dom_error_eq(result, DomError::ElementNotFound { selector: "xpath://div[@id='nonexistent_xpath']".to_string(), message: None });
    }

    #[wasm_bindgen_test]
    fn test_get_element_xpath_invalid_xpath() {
        let result = get_element_attribute("xpath://[invalid", "value");
        // The exact error message from browser's XPath engine can vary or be complex.
        // We check that it's an InvalidSelector and contains the problematic selector.
        match result {
            Err(DomError::InvalidSelector { selector, .. }) => {
                assert_eq!(selector, "xpath://[invalid");
            }
            other => panic!("Expected InvalidSelector, got {:?}", other),
        }
    }
    
    #[wasm_bindgen_test]
    fn test_get_element_attribute_not_found_on_existing_element() {
        let (_window, document) = get_window_document().unwrap();
        let el = setup_element(&document, "attr-test-exists", "div", None);

        let result = get_element_attribute("css:#attr-test-exists", "data-nonexistent");
        assert_dom_error_eq(result, DomError::AttributeNotFound {
            selector: "css:#attr-test-exists".to_string(),
            attribute_name: "data-nonexistent".to_string(),
        });
        
        cleanup_element(el);
    }


    #[wasm_bindgen_test]
    fn test_type_in_element_wrong_type() {
        let (_window, document) = get_window_document().unwrap();
        let el = setup_element(&document, "not_an_input_div", "div", None);

        let result = type_in_element("css:#not_an_input_div", "test");
        assert_dom_error_eq_unit(result, DomError::ElementTypeError {
            selector: "css:#not_an_input_div".to_string(),
            expected_type: "HtmlInputElement".to_string(),
        });

        cleanup_element(el);
    }


    #[wasm_bindgen_test]
    fn test_get_element_attribute_no_element_refined() {
        let result_css = get_element_attribute("css:#nonexistent_attr", "value");
        assert_dom_error_eq(result_css, DomError::ElementNotFound { selector: "css:#nonexistent_attr".to_string(), message: None });

        let result_xpath = get_element_attribute("xpath://*[@id='nonexistent_attr_xpath']", "value");
        assert_dom_error_eq(result_xpath, DomError::ElementNotFound { selector: "xpath://*[@id='nonexistent_attr_xpath']".to_string(), message: None });
    }

    #[wasm_bindgen_test]
    fn test_set_element_attribute_no_element_refined() {
        let result_css = set_element_attribute("css:#nonexistent_set_attr", "value", "test");
        assert_dom_error_eq_unit(result_css, DomError::ElementNotFound { selector: "css:#nonexistent_set_attr".to_string(), message: None });

        let result_xpath = set_element_attribute("xpath://*[@id='nonexistent_set_attr_xpath']", "value", "test");
        assert_dom_error_eq_unit(result_xpath, DomError::ElementNotFound { selector: "xpath://*[@id='nonexistent_set_attr_xpath']".to_string(), message: None });
    }

    #[wasm_bindgen_test]
    fn test_select_dropdown_option_no_element_refined() {
        let result_css = select_dropdown_option("css:#nonexistent_select", "option_value");
        assert_dom_error_eq_unit(result_css, DomError::ElementNotFound { selector: "css:#nonexistent_select".to_string(), message: None });

        let result_xpath = select_dropdown_option("xpath://select[@id='nonexistent_select_xpath']", "option_value");
        assert_dom_error_eq_unit(result_xpath, DomError::ElementNotFound { selector: "xpath://select[@id='nonexistent_select_xpath']".to_string(), message: None });
    }
    
    #[wasm_bindgen_test]
    fn test_select_dropdown_option_wrong_type() {
        let (_window, document) = get_window_document().unwrap();
        let el = setup_element(&document, "not_a_select", "div", None);

        let result = select_dropdown_option("css:#not_a_select", "value");
        assert_dom_error_eq_unit(result, DomError::ElementTypeError {
            selector: "css:#not_a_select".to_string(),
            expected_type: "HtmlSelectElement".to_string(),
        });
        cleanup_element(el);
    }


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
        
        let result = get_all_elements_attributes("xpath://a[@data-common='val']", "href");
        assert!(result.is_ok(), "Error: {:?}", result.err());
        assert_eq!(result.unwrap(), "[\"/page1\",null,\"/page3\"]");

        cleanup_element(el1);
        cleanup_element(el2);
        cleanup_element(el3);
    }

    #[wasm_bindgen_test]
    fn test_get_all_elements_attributes_invalid_css_selector() {
        let result = get_all_elements_attributes("css:[invalid-selector", "data-test");
        match result {
            Err(DomError::InvalidSelector { selector, .. }) => {
                assert_eq!(selector, "css:[invalid-selector");
            }
            other => panic!("Expected InvalidSelector, got {:?}", other),
        }
    }

    #[wasm_bindgen_test]
    fn test_get_all_elements_attributes_invalid_xpath_selector() {
        let result = get_all_elements_attributes("xpath://[invalid-xpath", "data-test");
         match result {
            Err(DomError::InvalidSelector { selector, .. }) => {
                assert_eq!(selector, "xpath://[invalid-xpath");
            }
            other => panic!("Expected InvalidSelector, got {:?}", other),
        }
    }

    // Tests for get_current_url
    #[wasm_bindgen_test]
    fn test_get_current_url_success() {
        // This test runs in a browser context, so window.location.href should be available.
        // The exact URL will depend on the test runner's environment, so we just check it's not empty.
        let result = get_current_url();
        assert!(result.is_ok(), "get_current_url should return Ok");
        let url = result.unwrap();
        assert!(!url.is_empty(), "URL should not be empty");
        // Example: "http://127.0.0.1:8000/wasm-test-adapter/test_page.html?..." or similar for wasm-pack test
        assert!(url.contains("http") || url.contains("file:"), "URL should be a valid http or file URL, got: {}", url);
    }

    // Tests for element_exists
    #[wasm_bindgen_test]
    fn test_element_exists_css_true() {
        let (_window, document) = get_window_document().unwrap();
        let el = setup_element(&document, "exists-css", "div", None);
        assert_eq!(element_exists("css:#exists-css").unwrap(), true);
        cleanup_element(el);
    }

    #[wasm_bindgen_test]
    fn test_element_exists_xpath_true() {
        let (_window, document) = get_window_document().unwrap();
        let el = setup_element(&document, "exists-xpath", "div", None);
        assert_eq!(element_exists("xpath://div[@id='exists-xpath']").unwrap(), true);
        cleanup_element(el);
    }

    #[wasm_bindgen_test]
    fn test_element_exists_false() {
        assert_eq!(element_exists("css:#nonexistent-for-exists").unwrap(), false);
    }

    #[wasm_bindgen_test]
    fn test_element_exists_invalid_selector() {
        let result = element_exists("css:[[[invalid");
        assert!(result.is_err());
        match result.unwrap_err() {
            DomError::InvalidSelector { selector, .. } => assert_eq!(selector, "css:[[[invalid"),
            other => panic!("Expected InvalidSelector, got {:?}", other),
        }
    }

    // Tests for wait_for_element
    #[wasm_bindgen_test]
    async fn test_wait_for_element_appears_immediately() {
        let (_window, document) = get_window_document().unwrap();
        let el = setup_element(&document, "wait-immediate", "div", None);
        let result = wait_for_element("css:#wait-immediate", Some(100)).await;
        assert!(result.is_ok(), "Element should be found immediately: {:?}", result.err());
        cleanup_element(el);
    }

    #[wasm_bindgen_test]
    async fn test_wait_for_element_appears_after_delay() {
        let (_window, document) = get_window_document().unwrap();
        let selector = "css:#wait-delayed";

        // Don't add element yet
        let wait_task = wait_for_element(selector, Some(500)); // Wait for 500ms

        // Create a future that adds the element after a short delay
        let add_element_task = async {
            TimeoutFuture::new(100).await; // Delay for 100ms
            ready(setup_element(&document, "wait-delayed", "div", None)).await
        };
        
        // Run both futures concurrently. select will complete when the first one does.
        // We expect wait_task to complete after add_element_task makes the element available.
        let (wait_result, el_handle_option) = futures::future::join(wait_task, async { Some(add_element_task.await) }).await;

        assert!(wait_result.is_ok(), "Element should be found after delay: {:?}", wait_result.err());
        if let Some(el) = el_handle_option {
            cleanup_element(el);
        }
    }
    
    #[wasm_bindgen_test]
    async fn test_wait_for_element_times_out() {
        let result = wait_for_element("css:#wait-timeout-nonexistent", Some(100)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            DomError::ElementNotFound { selector, message } => {
                assert_eq!(selector, "css:#wait-timeout-nonexistent");
                assert!(message.unwrap().contains("not found after 100ms timeout"));
            }
            other => panic!("Expected ElementNotFound due to timeout, got {:?}", other),
        }
    }

    #[wasm_bindgen_test]
    async fn test_wait_for_element_invalid_selector() {
        let result = wait_for_element("css:[[[invalid-wait", Some(100)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            DomError::InvalidSelector { selector, .. } => assert_eq!(selector, "css:[[[invalid-wait"),
            other => panic!("Expected InvalidSelector, got {:?}", other),
        }
    }

    #[wasm_bindgen_test]
    async fn test_wait_for_element_default_timeout() {
        // This test will take around 5 seconds if the element doesn't exist
        // To make it practical, we can test that it *would* succeed if element was there
        // or test the timeout with a very short, specific timeout for "non-existent"
        // The timeout_ms: None should use DEFAULT_TIMEOUT_MS (5000ms)
        let (_window, document) = get_window_document().unwrap();
        let el = setup_element(&document, "wait-default-timeout", "div", None);
        let result = wait_for_element("css:#wait-default-timeout", None).await; // Uses default timeout
        assert!(result.is_ok(), "Element should be found with default timeout: {:?}", result.err());
        cleanup_element(el);
    }
}
