use thiserror::Error;

#[derive(Error, Debug)]
pub enum TestError {
    #[error("Error between {source} and {target}")]
    UnreachablePath {
        source: String,
        target: String,
    },
}

fn main() {}
