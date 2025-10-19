pub mod element;
pub mod wait;
pub mod cookie;

use serde_json::Value;

pub fn build_js_call(func: &str, args: &[Value]) -> String {
    let args_str = args.iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("({})({})", func, args_str)
}