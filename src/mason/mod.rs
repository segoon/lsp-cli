pub mod platform;
pub mod registry;
pub mod resolve;
pub mod template;

mod http;
mod install;
mod link;
mod source;

pub use resolve::resolve_detect_suggestions;
