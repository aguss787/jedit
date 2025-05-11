#[derive(Debug, thiserror::Error)]
pub enum SerializationError {
    #[error("Invalid number: {0}")]
    InvalidNumber(f64),
}

#[derive(Debug, thiserror::Error)]
pub enum DumpError {
    #[error("Serialization error: {0}")]
    SerdeJson(#[from] sonic_rs::Error),
    #[error(transparent)]
    SerializationError(#[from] SerializationError),
}

#[derive(Debug, thiserror::Error)]
pub enum DeserializationError {
    #[error("Invalid number: {0}")]
    InvalidNumber(serde_json::Number),
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("Deserialization error: {0}")]
    SerdeJson(#[from] sonic_rs::Error),
    #[error(transparent)]
    DeserializationError(#[from] DeserializationError),
    #[error(transparent)]
    IO(#[from] std::io::Error),
}

// TODO: add error path
#[derive(Debug, thiserror::Error)]
#[cfg_attr(test, derive(PartialEq))]
pub enum IndexingError {
    #[error("Not indexable")]
    NotIndexable,
    #[error("Missing key: {0}")]
    MissingKey(String),
}

#[derive(Debug, thiserror::Error)]
#[cfg_attr(test, derive(PartialEq))]
pub enum MutationError {
    #[error("Duplicate key")]
    DuplicateKey,
    #[error("Not renameable")]
    NotRenameable,
    #[error(transparent)]
    Indexing(#[from] IndexingError),
}
