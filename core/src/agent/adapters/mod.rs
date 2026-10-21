//! Concrete CLI adapters, one file per engine. Shared helpers live in `common`.
mod claude;
mod codex;
mod common;
mod cursor;
mod custom;
mod gemini;
mod opencode;
mod qwen;

pub use claude::Claude;
pub use codex::Codex;
pub use cursor::Cursor;
pub use custom::Custom;
pub use gemini::Gemini;
pub use opencode::OpenCode;
pub use qwen::Qwen;
