use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{console, Window, Document, Element, HtmlElement, HtmlInputElement, XPathResult, NodeList}; // Removed Node
use serde_json; // Added for JSON serialization
use std::fmt;
use gloo_timers::future::{TimeoutFuture, IntervalStream};
use futures_util::stream::StreamExt; // For IntervalStream.next()
use futures::future::{select, Either}; // For select pattern

/// Represents errors that can occur during DOM operations.
#[derive(Debug, PartialEq)]
pub enum DomError {
    /// Indicates that an element could not be found using the provided selector.
    ElementNotFound {
        selector: String,
        /// Optional detailed message, e.g., from `wait_for_element` timeout.
        message: Option<String>
    },
    /// Indicates that the provided selector (CSS or XPath) is invalid.
    InvalidSelector { selector: String, error: String },
    /// Indicates that an element was found but is not of the expected HTML element type
    /// (e.g., trying to use `type_in_element` on a `<div>`).
    ElementTypeError { selector: String, expected_type: String },
    /// Indicates that a specified attribute was not found on the element.
    AttributeNotFound { selector: String, attribute_name: String },
    /// Indicates an error during JSON serialization of results.
    SerializationError { message: String },
    /// A generic JavaScript error occurred that doesn't fit other categories.
    JsError { message: String },
    /// A JavaScript `TypeError` occurred (e.g., calling a method on an undefined object).
    JsTypeError { message: String },
    /// A JavaScript `SyntaxError` occurred (e.g., an invalid selector string passed to `document.querySelector`).
    JsSyntaxError { message: String },
    /// A JavaScript `ReferenceError` occurred (e.g., accessing an undefined variable).
    JsReferenceError { message: String },
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
            DomError::JsTypeError { message } => write!(f, "JsTypeError: {}", message),
            DomError::JsSyntaxError { message } => write!(f, "JsSyntaxError: {}", message),
            DomError::JsReferenceError { message } => write!(f, "JsReferenceError: {}", message),
        }
    }
}

/// Checks if an element matching the given selector exists in the DOM.
///
/// # Arguments
/// * `selector`: A string representing a CSS selector (e.g., "#myId", ".myClass")
///   or an XPath expression (prefixed with "xpath:", e.g., "xpath://div[@id='example']").
///   If no prefix is provided, it defaults to a CSS selector.
///
/// # Returns
/// * `Ok(true)` if the element exists.
/// * `Ok(false)` if the element does not exist (specifically due to `DomError::ElementNotFound`).
/// * `Err(DomError)` for other errors, such as an invalid selector syntax.
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
        if value.is_instance_of::<js_sys::TypeError>() {
            DomError::JsTypeError {
                message: js_sys::Error::from(value).message().into(),
            }
        } else if value.is_instance_of::<js_sys::SyntaxError>() {
            DomError::JsSyntaxError {
                message: js_sys::Error::from(value).message().into(),
            }
        } else if value.is_instance_of::<js_sys::ReferenceError>() {
            DomError::JsReferenceError {
                message: js_sys::Error::from(value).message().into(),
            }
        } else {
            DomError::JsError {
                message: value.as_string().unwrap_or_else(|| "Unknown JsValue error".to_string()),
            }
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


/// Clicks an element identified by the given selector.
///
/// # Arguments
/// * `selector`: A string representing a CSS selector or an XPath expression.
///   If no prefix is provided, it defaults to a CSS selector.
///
/// # Returns
/// * `Ok(())` if the click was successful.
/// * `Err(DomError)` if the element is not found, not a clickable `HtmlElement`, or another error occurs.
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

/// Types the given text into an input element identified by the selector.
/// The element must be an `HTMLInputElement`.
///
/// # Arguments
/// * `selector`: A string representing a CSS selector or an XPath expression for the input element.
///   If no prefix is provided, it defaults to a CSS selector.
/// * `text`: The text to type into the element.
///
/// # Returns
/// * `Ok(())` if typing was successful.
/// * `Err(DomError)` if the element is not found, not an `HTMLInputElement`, or another error occurs.
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

/// Retrieves the inner text content of an element identified by the selector.
/// The element should be an `HtmlElement` or subclass.
///
/// # Arguments
/// * `selector`: A string representing a CSS selector or an XPath expression.
///   If no prefix is provided, it defaults to a CSS selector.
///
/// # Returns
/// * `Ok(String)` containing the text content if successful.
/// * `Err(DomError)` if the element is not found, not an `HtmlElement`, or another error occurs.
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

/// Retrieves the value of an input, textarea, or select element identified by the selector.
/// The element must be an `HTMLInputElement`.
///
/// # Arguments
/// * `selector`: A string representing a CSS selector or an XPath expression for the form element.
///   If no prefix is provided, it defaults to a CSS selector.
///
/// # Returns
/// * `Ok(String)` containing the element's value if successful.
/// * `Err(DomError)` if the element is not found, not an `HTMLInputElement`, or another error occurs.
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

/// Retrieves the value of a specified attribute from an element identified by the selector.
///
/// # Arguments
/// * `selector`: A string representing a CSS selector or an XPath expression.
///   If no prefix is provided, it defaults to a CSS selector.
/// * `attribute_name`: The name of the attribute to retrieve.
///
/// # Returns
/// * `Ok(String)` containing the attribute's value if successful.
/// * `Err(DomError::AttributeNotFound)` if the attribute does not exist on the element.
/// * `Err(DomError)` for other errors, such as element not found or invalid selector.
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

/// Waits for an element matching the selector to exist in the DOM within a specified timeout.
///
/// Polls the DOM at regular intervals (currently 100ms) until the element is found
/// or the timeout is reached.
///
/// # Arguments
/// * `selector`: A string representing a CSS selector or an XPath expression.
///   If no prefix is provided, it defaults to a CSS selector.
/// * `timeout_ms`: An optional timeout in milliseconds. If `None`, a default timeout (5000ms) is used.
///
/// # Returns
/// * `Ok(())` if the element appears within the timeout.
/// * `Err(DomError::ElementNotFound)` if the element does not appear within the timeout,
///   with a message indicating the timeout duration.
/// * `Err(DomError)` for other errors, such as an invalid selector.
#[wasm_bindgen]
pub async fn wait_for_element(selector: &str, timeout_ms: Option<u32>) -> Result<(), DomError> {
    const DEFAULT_TIMEOUT_MS: u32 = 5000; // Default timeout: 5 seconds
    const INTERVAL_MS: u32 = 100; // Polling interval: 100 milliseconds
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

/// Sets an attribute on an element identified by the selector.
///
/// # Arguments
/// * `selector`: A string representing a CSS selector or an XPath expression.
///   If no prefix is provided, it defaults to a CSS selector.
/// * `attribute_name`: The name of the attribute to set.
/// * `attribute_value`: The value to set for the attribute.
///
/// # Returns
/// * `Ok(())` if the attribute was set successfully.
/// * `Err(DomError)` if the element is not found or the attribute cannot be set (e.g., invalid attribute name, read-only attribute).
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

/// Selects an option in a dropdown (`<select>`) element identified by the selector by setting its value.
/// The element must be an `HtmlSelectElement`.
///
/// # Arguments
/// * `selector`: A string representing a CSS selector or an XPath expression for the `<select>` element.
///   If no prefix is provided, it defaults to a CSS selector.
/// * `value`: The value of the `<option>` to select.
///
/// # Returns
/// * `Ok(())` if the option was selected successfully.
/// * `Err(DomError)` if the element is not found, not an `HtmlSelectElement`, or the value cannot be set.
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

/// Retrieves a specific attribute from all elements matching the selector and returns them as a JSON string.
///
/// # Arguments
/// * `selector`: A string representing a CSS selector or an XPath expression for the elements.
///   If no prefix is provided, it defaults to a CSS selector.
/// * `attribute_name`: The name of the attribute to retrieve from each element.
///
/// # Returns
/// * `Ok(String)` containing a JSON array of attribute values. Each value in the array
///   is either the attribute string or `null` if the attribute is missing for a specific element.
///   Returns an empty JSON array `[]` if no elements are found.
/// * `Err(DomError)` if an error occurs during element retrieval or JSON serialization.
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

/// Retrieves the current URL of the page.
///
/// # Returns
/// * `Ok(String)` containing the current URL if successful.
/// * `Err(DomError::JsError)` if the URL cannot be retrieved from `window.location.href`.
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

/// Checks if an element identified by the selector is currently visible on the page.
///
/// An element is considered visible if it meets all the following conditions:
/// * It is present in the DOM.
/// * Its computed `display` style is not `none`.
/// * Its computed `visibility` style is not `hidden`.
/// * Its bounding box has a width and height greater than 0.
///   * If width or height is 0, it additionally checks if its computed `opacity` is "0". If so, it's considered not visible.
///
/// Note: This function checks the computed style of the element itself.
/// Parent-induced invisibility (e.g., a parent with `display: none` or `visibility: hidden`)
/// is typically reflected in the child's computed styles or bounding box dimensions.
///
/// # Arguments
/// * `selector`: A string representing a CSS selector or an XPath expression.
///   If no prefix is provided, it defaults to a CSS selector.
///
/// # Returns
/// * `Ok(true)` if the element is determined to be visible.
/// * `Ok(false)` if the element is determined to be not visible.
/// * `Err(DomError)` if the element is not found or another error occurs during style/dimension retrieval.
#[wasm_bindgen]
pub fn is_visible(selector: &str) -> Result<bool, DomError> {
    console::log_1(&format!("Checking visibility for selector: {}", selector).into());
    let (window, document) = get_window_document()?;
    let element = get_element(&document, selector)?;

    let style = window.get_computed_style(&element)
        .map_err(|e| DomError::JsError { message: format!("Failed to get computed style for {}: {:?}", selector, e.as_string()) })?
        .ok_or_else(|| DomError::JsError { message: format!("Computed style is null for {}", selector) })?;

    let display = style.get_property_value("display")
        .map_err(|e| DomError::JsError { message: format!("Failed to get display property for {}: {:?}", selector, e.as_string()) })?;
    if display == "none" {
        console::log_1(&format!("Element {} is not visible (display: none)", selector).into());
        return Ok(false);
    }

    let visibility = style.get_property_value("visibility")
        .map_err(|e| DomError::JsError { message: format!("Failed to get visibility property for {}: {:?}", selector, e.as_string()) })?;
    if visibility == "hidden" {
        console::log_1(&format!("Element {} is not visible (visibility: hidden)", selector).into());
        return Ok(false);
    }

    let rect = element.get_bounding_client_rect();
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        // Check for opacity: 0 as well, as zero-size elements might still be considered "visible" by some definitions if opacity is not 0
        let opacity_str = style.get_property_value("opacity")
            .map_err(|e| DomError::JsError { message: format!("Failed to get opacity property for {}: {:?}", selector, e.as_string()) })?;
        if let Ok(opacity_val) = opacity_str.parse::<f64>() {
            if opacity_val <= 0.0 {
                console::log_1(&format!("Element {} is not visible (opacity: 0)", selector).into());
                return Ok(false);
            }
        }
        // If opacity is not 0, but width/height is 0, it might still be considered not visible for interaction.
        // However, some interpretations might vary. For now, zero width/height is sufficient.
        console::log_1(&format!("Element {} is not visible (width: {}, height: {})", selector, rect.width(), rect.height()).into());
        return Ok(false);
    }

    // Additionally, check parent visibility. If any parent is display:none, this isn't truly visible.
    // This is a simplified check; a full check would traverse up the DOM tree.
    // For now, we rely on the browser's computed style for the element itself.
    // A more robust check might involve `offsetParent` being null, but that also has caveats.

    console::log_1(&format!("Element {} is visible", selector).into());
    Ok(true)
}

/// Scrolls the page to make the element identified by the selector visible in the viewport.
///
/// Uses the standard `element.scroll_into_view()` method.
///
/// # Arguments
/// * `selector`: A string representing a CSS selector or an XPath expression.
///   If no prefix is provided, it defaults to a CSS selector.
///
/// # Returns
/// * `Ok(())` if scrolling was successful (or if the element was already in view and no scrolling was needed).
/// * `Err(DomError)` if the element is not found or another error occurs.
#[wasm_bindgen]
pub fn scroll_to(selector: &str) -> Result<(), DomError> {
    console::log_1(&format!("Attempting to scroll to element with selector: {}", selector).into());
    let (_window, document) = get_window_document()?;
    let element = get_element(&document, selector)?;

    element.scroll_into_view(); // Basic scroll
    // For more options:
    // let mut options = web_sys::ScrollIntoViewOptions::new();
    // options.behavior(web_sys::ScrollBehavior::Smooth);
    // options.block(web_sys::ScrollLogicalPosition::Center);
    // element.scroll_into_view_with_scroll_into_view_options(&options);

    console::log_1(&format!("Successfully scrolled to element with selector: {}", selector).into());
    Ok(())
}

/// Simulates hovering over an element by dispatching `mouseover` and `mouseenter` events.
///
/// This function attempts to find the element specified by the selector. If found,
/// it casts the element to an `HtmlElement` and then programmatically creates and dispatches
/// both `mouseover` and `mouseenter` events on it. These events are configured to bubble
/// and be cancelable. This is useful for triggering hover-dependent UI changes or
/// JavaScript logic.
///
/// # Arguments
/// * `selector`: A `&str` representing a CSS selector (e.g., "#myId", ".myClass")
///   or an XPath expression (prefixed with "xpath:", e.g., "xpath://div[@id='example']")
///   used to identify the target element.
///
/// # Returns
/// * `Ok(())` if the element is found and both `mouseover` and `mouseenter` events are successfully dispatched.
/// * `Err(DomError)` if:
///     - The element is not found (`DomError::ElementNotFound`).
///     - The found element cannot be cast to `HtmlElement` (`DomError::ElementTypeError`).
///     - There's an issue creating or dispatching the mouse events (`DomError::JsError`).
#[wasm_bindgen]
pub fn hover_element(selector: &str) -> Result<(), DomError> {
    console::log_1(&format!("Attempting to hover over element with selector: {}", selector).into());
    let (window, document) = get_window_document()?;
    let element = get_element(&document, selector)?;

    let html_element = element
        .dyn_into::<HtmlElement>()
        .map_err(|_| DomError::ElementTypeError {
            selector: selector.to_string(),
            expected_type: "HtmlElement".to_string(),
        })?;

    // Create a mouse event that bubbles and is cancelable
    let mut event_init = web_sys::MouseEventInit::new();
    event_init.bubbles(true);
    event_init.cancelable(true);
    event_init.view(Some(&window));

    // Dispatch mouseover event
    let mouseover_event = web_sys::MouseEvent::new_with_event_init_dict("mouseover", &event_init)
        .map_err(|e| DomError::JsError { message: format!("Failed to create mouseover event: {:?}", e.as_string()) })?;
    html_element.dispatch_event(&mouseover_event)
        .map_err(|e| DomError::JsError { message: format!("Failed to dispatch mouseover event: {:?}", e.as_string()) })?;

    // Dispatch mouseenter event (often used together with mouseover for hover effects)
    let mouseenter_event = web_sys::MouseEvent::new_with_event_init_dict("mouseenter", &event_init)
        .map_err(|e| DomError::JsError { message: format!("Failed to create mouseenter event: {:?}", e.as_string()) })?;
    html_element.dispatch_event(&mouseenter_event)
        .map_err(|e| DomError::JsError { message: format!("Failed to dispatch mouseenter event: {:?}", e.as_string()) })?;

    console::log_1(&format!("Successfully hovered over element with selector: {}", selector).into());
    Ok(())
}

/// Retrieves and concatenates the inner text content from all elements matching the given selector.
///
/// This function finds all DOM elements that match the provided `selector`. For each
/// matching element that is an `HtmlElement`, it extracts its `inner_text()`.
/// Only non-empty text strings are collected. These collected text strings are then
/// joined together into a single `String`, with the specified `separator` inserted
/// between each piece of text.
///
/// # Arguments
/// * `selector`: A `&str` representing a CSS selector (e.g., ".myClass", "div > p")
///   or an XPath expression (prefixed with "xpath:", e.g., "xpath://ul/li") used to
///   identify the target elements.
/// * `separator`: A `&str` that will be used to join the `inner_text` from each
///   matching element. For example, a newline character `"\n"`, a comma and space `", "`,
///   or any other custom string.
///
/// # Returns
/// * `Ok(String)`:
///     - If elements are found and contain text, this is the concatenated string of their
///       `inner_text` values, joined by the `separator`.
///     - If no elements are found matching the `selector`, an empty string is returned.
///     - If elements are found but none of them contain any non-empty text content (e.g.,
///       they are empty elements or contain only other elements without text), an empty
///       string is returned.
/// * `Err(DomError)`: If an error occurs during element retrieval, such as an
///   `InvalidSelector` if the provided selector string is malformed.
#[wasm_bindgen]
pub fn get_all_text_from_elements(selector: &str, separator: &str) -> Result<String, DomError> {
    console::log_1(&format!("Attempting to get all text from elements matching selector: {} with separator: '{}'", selector, separator).into());
    let (_window, document) = get_window_document()?;
    let elements = get_all_elements(&document, selector)?;

    if elements.is_empty() {
        console::log_1(&format!("No elements found for selector '{}'. Returning empty string.", selector).into());
        return Ok("".to_string());
    }

    let texts: Vec<String> = elements
        .into_iter()
        .filter_map(|el| {
            el.dyn_into::<HtmlElement>().ok().map(|html_el| html_el.inner_text())
        })
        .filter(|text| !text.is_empty()) // Optionally filter out empty strings
        .collect();

    if texts.is_empty() {
        console::log_1(&format!("Elements found for selector '{}', but they contained no text. Returning empty string.", selector).into());
        return Ok("".to_string());
    }

    console::log_1(&format!("Successfully retrieved {} text segments for selector '{}'.", texts.len(), selector).into());
    Ok(texts.join(separator))
}


#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    use wasm_bindgen::JsValue;
    use web_sys::{EventTarget, MouseEventInit, MouseEvent}; // Added for hover tests
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
        assert_eq!(
            DomError::JsTypeError { message: "type error".to_string() }.to_string(),
            "JsTypeError: type error"
        );
        assert_eq!(
            DomError::JsSyntaxError { message: "syntax error".to_string() }.to_string(),
            "JsSyntaxError: syntax error"
        );
        assert_eq!(
            DomError::JsReferenceError { message: "reference error".to_string() }.to_string(),
            "JsReferenceError: reference error"
        );
    }

    #[test]
    fn test_dom_error_into_js_value() {
        let error = DomError::ElementNotFound { selector: "test".to_string(), message: None };
        let js_value: JsValue = error.into();
        assert_eq!(js_value.as_string().unwrap(), "ElementNotFound: No element found for selector 'test'");
    }

    #[test]
    fn test_dom_error_from_js_value_generic() {
        let js_value_error = JsValue::from_str("generic js error");
        let dom_error: DomError = js_value_error.into();
        match dom_error {
            DomError::JsError { message } => assert_eq!(message, "generic js error"),
            _ => panic!("Incorrect DomError variant for generic JsValue"),
        }
    }

    #[wasm_bindgen_test]
    fn test_dom_error_from_js_value_type_error() {
        let js_error = js_sys::TypeError::new("test type error");
        let js_value_error: JsValue = js_error.into();
        let dom_error: DomError = js_value_error.into();
        match dom_error {
            DomError::JsTypeError { message } => assert_eq!(message, "test type error"),
            _ => panic!("Incorrect DomError variant for TypeError JsValue"),
        }
    }

    #[wasm_bindgen_test]
    fn test_dom_error_from_js_value_syntax_error() {
        let js_error = js_sys::SyntaxError::new("test syntax error");
        let js_value_error: JsValue = js_error.into();
        let dom_error: DomError = js_value_error.into();
        match dom_error {
            DomError::JsSyntaxError { message } => assert_eq!(message, "test syntax error"),
            _ => panic!("Incorrect DomError variant for SyntaxError JsValue"),
        }
    }

    #[wasm_bindgen_test]
    fn test_dom_error_from_js_value_reference_error() {
        let js_error = js_sys::ReferenceError::new("test reference error");
        let js_value_error: JsValue = js_error.into();
        let dom_error: DomError = js_value_error.into();
        match dom_error {
            DomError::JsReferenceError { message } => assert_eq!(message, "test reference error"),
            _ => panic!("Incorrect DomError variant for ReferenceError JsValue"),
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

    // Tests for is_visible
    #[wasm_bindgen_test]
    fn test_is_visible_standard_element() {
        let (_window, document) = get_window_document().unwrap();
        let el = setup_element(&document, "visible-el", "div", Some(vec![("style", "width: 10px; height: 10px; background: blue;")]));
        assert_eq!(is_visible("css:#visible-el").unwrap(), true, "Standard visible element reported as not visible");
        cleanup_element(el);
    }

    #[wasm_bindgen_test]
    fn test_is_visible_display_none() {
        let (_window, document) = get_window_document().unwrap();
        let el = setup_element(&document, "display-none-el", "div", Some(vec![("style", "display: none;")]));
        assert_eq!(is_visible("css:#display-none-el").unwrap(), false, "Element with display:none reported as visible");
        cleanup_element(el);
    }

    #[wasm_bindgen_test]
    fn test_is_visible_visibility_hidden() {
        let (_window, document) = get_window_document().unwrap();
        let el = setup_element(&document, "visibility-hidden-el", "div", Some(vec![("style", "visibility: hidden; width: 10px; height: 10px;")]));
        assert_eq!(is_visible("css:#visibility-hidden-el").unwrap(), false, "Element with visibility:hidden reported as visible");
        cleanup_element(el);
    }

    #[wasm_bindgen_test]
    fn test_is_visible_zero_dimensions() {
        let (_window, document) = get_window_document().unwrap();
        let el = setup_element(&document, "zero-dim-el", "div", Some(vec![("style", "width: 0; height: 0;")]));
        assert_eq!(is_visible("css:#zero-dim-el").unwrap(), false, "Element with zero dimensions reported as visible");
        cleanup_element(el);

        let el2 = setup_element(&document, "zero-width-el", "div", Some(vec![("style", "width: 0; height: 10px;")]));
        assert_eq!(is_visible("css:#zero-width-el").unwrap(), false, "Element with zero width reported as visible");
        cleanup_element(el2);

        let el3 = setup_element(&document, "zero-height-el", "div", Some(vec![("style", "width: 10px; height: 0;")]));
        assert_eq!(is_visible("css:#zero-height-el").unwrap(), false, "Element with zero height reported as visible");
        cleanup_element(el3);
    }

    #[wasm_bindgen_test]
    fn test_is_visible_opacity_zero_positive_dimensions() {
        let (_window, document) = get_window_document().unwrap();
        let el = setup_element(&document, "opacity-zero-pos-dim-el", "div", Some(vec![("style", "width: 10px; height: 10px; opacity: 0;")]));
        // Element is in layout, occupies space, but is not visible to human eye.
        // Current `is_visible` logic considers this visible because rect.width/height > 0 and display/visibility are normal.
        // Opacity check is only triggered if width/height is also zero.
        assert_eq!(is_visible("css:#opacity-zero-pos-dim-el").unwrap(), true, "Element with opacity:0 but positive dimensions should be true by current logic");
        cleanup_element(el);
    }

    #[wasm_bindgen_test]
    fn test_is_visible_zero_dimensions_and_opacity_zero() {
        let (_window, document) = get_window_document().unwrap();
        let el_zero_dim_opacity_zero = setup_element(&document, "opacity-zero-dim-zero-el", "div", Some(vec![("style", "width: 0px; height: 0px; opacity: 0;")]));
        assert_eq!(is_visible("css:#opacity-zero-dim-zero-el").unwrap(), false, "Element with opacity:0 and zero dimensions reported as visible");
        cleanup_element(el_zero_dim_opacity_zero);
    }

    #[wasm_bindgen_test]
    fn test_is_visible_child_of_display_none_parent() {
        let (_window, document) = get_window_document().unwrap();
        let parent = setup_element(&document, "parent-display-none", "div", Some(vec![("style", "display: none;")]));
        let child = document.create_element("div").unwrap();
        child.set_id("child-of-display-none");
        child.set_attribute("style", "width: 10px; height: 10px;").unwrap();
        parent.append_child(&child).unwrap();

        // The child's own computed style for "display" might not be "none" (it's "block" by default for a div),
        // but get_bounding_client_rect() should return all zeros because the parent is not rendered.
        // Our current `is_visible` logic relies on `get_computed_style` of the element itself.
        // If parent is display:none, child's get_bounding_client_rect() will have 0 width/height.
        assert_eq!(is_visible("css:#child-of-display-none").unwrap(), false, "Child of display:none parent reported as visible");
        cleanup_element(parent); // Child is removed with parent
    }

    #[wasm_bindgen_test]
    fn test_is_visible_child_of_visibility_hidden_parent() {
        let (_window, document) = get_window_document().unwrap();
        let parent = setup_element(&document, "parent-visibility-hidden", "div", Some(vec![("style", "visibility: hidden; width: 20px; height: 20px;")]));
        let child = document.create_element("div").unwrap();
        child.set_id("child-of-visibility-hidden");
        child.set_attribute("style", "width: 10px; height: 10px; background: green;").unwrap(); // Child itself is visibility: visible by default
        parent.append_child(&child).unwrap();

        // If parent is visibility:hidden, child (even if visibility:visible) is not visible.
        // The computed style for the child's 'visibility' should be 'hidden' due to inheritance.
        assert_eq!(is_visible("css:#child-of-visibility-hidden").unwrap(), false, "Child of visibility:hidden parent reported as visible");
        cleanup_element(parent);
    }


    #[wasm_bindgen_test]
    fn test_is_visible_no_element() {
        let result = is_visible("css:#nonexistent-visible-check");
        assert!(result.is_err());
        match result.unwrap_err() {
            DomError::ElementNotFound { selector, .. } => assert_eq!(selector, "css:#nonexistent-visible-check"),
            other => panic!("Expected ElementNotFound, got {:?}", other),
        }
    }

    // Tests for scroll_to
    #[wasm_bindgen_test]
    fn test_scroll_to_existing_element() {
        let (_window, document) = get_window_document().unwrap();
        // Make the body scrollable and add an element at the bottom
        document.body().unwrap().set_attribute("style", "height: 2000px;").unwrap();
        let el = document.create_element("div").unwrap();
        el.set_id("scroll-target");
        el.set_inner_html("Scroll To Me");
        el.set_attribute("style", "margin-top: 1800px; height: 50px; background: lightblue;").unwrap();
        document.body().unwrap().append_child(&el).unwrap();

        let initial_scroll_y = web_sys::window().unwrap().scroll_y().unwrap_or(0.0);
        assert_eq!(initial_scroll_y, 0.0, "Initial scroll Y should be 0");

        let result = scroll_to("css:#scroll-target");
        assert!(result.is_ok(), "scroll_to failed: {:?}", result.err());

        let final_scroll_y = web_sys::window().unwrap().scroll_y().unwrap_or(0.0);
        // Exact scroll position can be tricky due to browser differences/layout,
        // but it should definitely be greater than 0 and likely close to the element's offset.
        assert!(final_scroll_y > 1500.0, "Final scroll Y ({}) should be significantly greater after scroll_to", final_scroll_y);

        // Cleanup
        document.body().unwrap().remove_attribute("style").unwrap();
        cleanup_element(el);
        web_sys::window().unwrap().scroll_to_with_x_and_y(0.0, 0.0); // Reset scroll
    }

    #[wasm_bindgen_test]
    fn test_scroll_to_no_element() {
        let result = scroll_to("css:#nonexistent-scroll-target");
        assert!(result.is_err());
        match result.unwrap_err() {
            DomError::ElementNotFound { selector, .. } => assert_eq!(selector, "css:#nonexistent-scroll-target"),
            other => panic!("Expected ElementNotFound, got {:?}", other),
        }
    }

    // Tests for hover_element
    #[wasm_bindgen_test]
    async fn test_hover_element_success() {
        let (_window, document) = get_window_document().unwrap();
        let el_id = "hover-test-el";
        let el = setup_element(&document, el_id, "div", Some(vec![("style", "width:50px;height:50px;background:blue;")]));

        // Add event listeners to check if events are dispatched
        let mouseover_received = std::rc::Rc::new(std::cell::Cell::new(false));
        let mouseenter_received = std::rc::Rc::new(std::cell::Cell::new(false));

        let mouseover_received_clone = mouseover_received.clone();
        let on_mouseover = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
            mouseover_received_clone.set(true);
        }) as Box<dyn FnMut(_)>);

        let mouseenter_received_clone = mouseenter_received.clone();
        let on_mouseenter = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
            mouseenter_received_clone.set(true);
        }) as Box<dyn FnMut(_)>);

        let event_target: &EventTarget = el.as_ref();
        event_target.add_event_listener_with_callback("mouseover", on_mouseover.as_ref().unchecked_ref()).unwrap();
        event_target.add_event_listener_with_callback("mouseenter", on_mouseenter.as_ref().unchecked_ref()).unwrap();
        on_mouseover.forget(); // To keep the closure alive
        on_mouseenter.forget();


        let result = hover_element(&format!("css:#{}", el_id));
        assert!(result.is_ok(), "hover_element failed: {:?}", result.err());

        // Give a brief moment for events to be processed, though dispatch should be synchronous for basic cases.
        // For more complex scenarios or if issues arise, a small delay might be needed here.
        // TimeoutFuture::new(10).await;

        assert!(mouseover_received.get(), "mouseover event was not received");
        assert!(mouseenter_received.get(), "mouseenter event was not received");

        cleanup_element(el);
    }

    #[wasm_bindgen_test]
    fn test_hover_element_no_element() {
        let result = hover_element("css:#nonexistent-hover-target");
        assert!(result.is_err());
        match result.unwrap_err() {
            DomError::ElementNotFound { selector, .. } => assert_eq!(selector, "css:#nonexistent-hover-target"),
            other => panic!("Expected ElementNotFound, got {:?}", other),
        }
    }

    // Tests for get_all_text_from_elements
    #[wasm_bindgen_test]
    fn test_get_all_text_from_elements_success() {
        let (_window, document) = get_window_document().unwrap();
        let parent = setup_element(&document, "text-parent", "div", None);

        let child1 = document.create_element("p").unwrap();
        child1.set_id("text-child1");
        child1.set_text_content(Some("Hello"));
        parent.append_child(&child1).unwrap();

        let child2 = document.create_element("p").unwrap();
        child2.set_id("text-child2");
        child2.set_text_content(Some("World"));
        parent.append_child(&child2).unwrap();

        // Element with no text
        let child3 = document.create_element("p").unwrap();
        child3.set_id("text-child3");
        parent.append_child(&child3).unwrap();

        // Element that is not HtmlElement (e.g. SVG), should be skipped by dyn_into
        // let svg_el = document.create_element_ns(Some("http://www.w3.org/2000/svg"), "svg").unwrap();
        // parent.append_child(&svg_el).unwrap();


        let result = get_all_text_from_elements("css:#text-parent p", ", ");
        assert!(result.is_ok(), "get_all_text_from_elements failed: {:?}", result.err());
        assert_eq!(result.unwrap(), "Hello, World");

        let result_newline = get_all_text_from_elements("css:#text-parent p", "\n");
        assert!(result_newline.is_ok(), "get_all_text_from_elements failed: {:?}", result_newline.err());
        assert_eq!(result_newline.unwrap(), "Hello\nWorld");

        cleanup_element(parent); // Cleans children too
    }

    #[wasm_bindgen_test]
    fn test_get_all_text_from_elements_no_elements_found() {
        let result = get_all_text_from_elements("css:.nonexistent-text-class", ", ");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[wasm_bindgen_test]
    fn test_get_all_text_from_elements_elements_found_no_text() {
        let (_window, document) = get_window_document().unwrap();
        let el1 = setup_element(&document, "no-text1", "div", None);
        let el2 = setup_element(&document, "no-text2", "div", None);
        el1.set_attribute("class", "no-text-class").unwrap();
        el2.set_attribute("class", "no-text-class").unwrap();

        let result = get_all_text_from_elements("css:.no-text-class", ", ");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        cleanup_element(el1);
        cleanup_element(el2);
    }

    #[wasm_bindgen_test]
    fn test_get_all_text_from_elements_invalid_selector() {
        let result = get_all_text_from_elements("css:[[[invalid-text-selector", ", ");
        assert!(result.is_err());
         match result.unwrap_err() {
            DomError::InvalidSelector { selector, .. } => assert_eq!(selector, "css:[[[invalid-text-selector"),
            other => panic!("Expected InvalidSelector, got {:?}", other),
        }
    }
}
