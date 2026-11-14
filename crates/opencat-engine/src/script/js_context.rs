//! Engine JS backend selection.

#[cfg(all(feature = "js-v8", feature = "js-quickjs"))]
compile_error!("select exactly one of js-v8 and js-quickjs");

#[cfg(feature = "js-v8")]
pub type EngineJsContext = crate::script::v8::V8JsContext;

#[cfg(feature = "js-quickjs")]
pub type EngineJsContext = crate::script::quickjs::RqJsContext;

