//! Utilities for interacting with the console
pub use ansi_parsing::*;
pub use console_color::*;
pub use console_elem::*;

mod console_elem;
mod ansi_parsing;
mod console_color;

pub const RESET_CODE: & str = "\u{001B}[0m";