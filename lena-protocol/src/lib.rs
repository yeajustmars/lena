#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]
// TODO: set clippy::pedantic in just lib.rs/mod.rs, as long as it carries through module

pub mod emitter;
pub mod schema;
pub mod validate;
