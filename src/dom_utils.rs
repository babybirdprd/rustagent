use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{console, Window, Document, Element, HtmlElement, HtmlInputElement};

// Helper function to get window and document
fn get_window_document() -> Result<(Window, Document), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("Failed to get window object"))?;
    let document = window.document().ok_or_else(|| JsValue::from_str("Failed to get document object"))?;
    Ok((window, document))
}

// Helper function to query select an element
fn query_selector_element(document: &Document, selector: &str) -> Result<Element, JsValue> {
    document
        .query_selector(selector)
        .map_err(|e| JsValue::from_str(&format!("query_selector failed: {:?}", e)))?
        .ok_or_else(|| JsValue::from_str(&format!("Element with selector '{}' not found", selector)))
}

#[wasm_bindgen]
pub fn click_element(selector: &str) -> Result<(), JsValue> {
    console::log_1(&format!("Attempting to click element with selector: {}", selector).into());
    let (_window, document) = get_window_document()?;
    
    let element = query_selector_element(&document, selector)?;

    let html_element = element
        .dyn_into::<HtmlElement>()
        .map_err(|_| JsValue::from_str("Element is not an HTMLElement"))?;
    
    html_element
        .click();
        
    console::log_1(&format!("Successfully clicked element with selector: {}", selector).into());
    Ok(())
}

#[wasm_bindgen]
pub fn type_in_element(selector: &str, text: &str) -> Result<(), JsValue> {
    console::log_1(&format!("Attempting to type '{}' in element with selector: {}", text, selector).into());
    let (_window, document) = get_window_document()?;

    let element = query_selector_element(&document, selector)?;

    let input_element = element
        .dyn_into::<HtmlInputElement>()
        .map_err(|_| JsValue::from_str(&format!("Element with selector '{}' is not an HtmlInputElement", selector)))?;

    input_element.set_value(text);
    
    console::log_1(&format!("Successfully typed '{}' in element with selector: {}", text, selector).into());
    Ok(())
}

#[wasm_bindgen]
pub fn get_element_text(selector: &str) -> Result<String, JsValue> {
    console::log_1(&format!("Attempting to get text from element with selector: {}", selector).into());
    let (_window, document) = get_window_document()?;

    let element = query_selector_element(&document, selector)?;

    let html_element = element
        .dyn_into::<HtmlElement>()
        .map_err(|_| JsValue::from_str(&format!("Element with selector '{}' is not an HtmlElement", selector)))?;
    
    console::log_1(&format!("Successfully retrieved text from element with selector: {}", selector).into());
    Ok(html_element.inner_text())
}

#[wasm_bindgen]
pub fn get_element_value(selector: &str) -> Result<String, JsValue> {
    console::log_1(&format!("Attempting to get value from input element with selector: {}", selector).into());
    let (_window, document) = get_window_document()?;
    
    let element = query_selector_element(&document, selector)?;

    let input_element = element
        .dyn_into::<HtmlInputElement>()
        .map_err(|_| JsValue::from_str(&format!("Element with selector '{}' is not an HtmlInputElement", selector)))?;
    
    console::log_1(&format!("Successfully retrieved value from element with selector: {}", selector).into());
    Ok(input_element.value())
}
