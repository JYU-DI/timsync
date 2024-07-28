// Basic helper script
// The helper script receives only the parameters via two variables:
// `params` (for indexed arguments) and `hash` (for key=value arguments).
//
// Like in Rust, the last non-terminal expression is returned (or the first `return val` encountered).
//
// More information on the Rhai scripting language: https://rhai.rs/book/language/index.html

`Hello, ${params[0]}!`