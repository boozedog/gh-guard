use thiserror::Error;

#[derive(Error, Debug)]
pub enum GuardError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("policy denied")]
    Denied,

    #[error("gate denied: sudo authorization failed")]
    GateDenied,

    #[error("refusing to run as root")]
    RefuseRoot,

    #[error("recursion detected: managed real gh appears to be the wrapper itself")]
    Recursion,

    #[error("real gh not available and cannot be installed: {0}")]
    RealGhMissing(String),

    #[error("{0}")]
    Other(String),
}
