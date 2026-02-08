// rmcp's #[tool] macros generate code that calls these functions,
// but rustc/clippy can't trace through the macro-generated dispatching.
#![allow(dead_code)]

pub mod browser;
pub mod interaction;
pub mod selectors;
pub mod server;
pub mod tools;
