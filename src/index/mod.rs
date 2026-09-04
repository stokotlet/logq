mod build;
mod dict;
mod files;
mod query;
mod record;

pub use build::build;
pub use query::{Select, counts, each_line};

#[cfg(test)]
mod tests;
