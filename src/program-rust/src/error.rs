use num_derive::FromPrimitive;
use solana_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

/// Errors that may be returned by the SolGSN program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum GsnError {
    /// The account cannot be initialized because it is already being used.
    #[error("SolGSN account already in use")]
    AlreadyInUse,
    #[error("InvalidState")]
    InvalidState,
    /// Unauthorized: caller is not the governance authority
    #[error("Unauthorized: not the governance authority")]
    Unauthorized,
    /// Governance not initialized
    #[error("Governance not initialized")]
    GovernanceNotInitialized,
    /// Invalid fee mode
    #[error("Invalid fee mode")]
    InvalidFeeMode,
}

impl From<GsnError> for ProgramError {
    fn from(e: GsnError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl<T> DecodeError<T> for GsnError {
    fn type_of() -> &'static str {
        "SolGSN Error"
    }
}
