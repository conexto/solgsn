/// Instructions supported by the SolGSN.
use crate::state::FeeMode;
use solana_program::program_error::ProgramError;
use std::mem::size_of;

/// Topup argument structure
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct TopupAgrs {
    pub amount: u64,
}

/// Submit argument structure
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct SubmitArgs {
    pub amount: u64,
    /// Nonce to prevent replay attacks
    pub nonce: u64,
}

/// Update fee parameters argument structure
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct UpdateFeeParamsArgs {
    /// Fee mode: 0 = Fixed, 1 = Percent
    pub fee_mode_type: u8,
    /// For Fixed: amount in lamports, For Percent: basis points (e.g., 100 = 1%)
    pub fee_value: u64,
}

/// Add/Remove allowed token argument structure
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct TokenMintArgs {
    /// Token mint address as bytes (32 bytes for Pubkey)
    pub mint: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub enum GsnInstruction {
    Initialize,
    Topup(TopupAgrs),
    SubmitTransaction(SubmitArgs),
    UpdateFeeParams(UpdateFeeParamsArgs),
    AddAllowedToken(TokenMintArgs),
    RemoveAllowedToken(TokenMintArgs),
    ClaimFees,
}

impl GsnInstruction {
    pub fn deserialize(input: &[u8]) -> Result<Self, ProgramError> {
        if input.len() < size_of::<u8>() {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(match input[0] {
            0 => Self::Initialize,
            1 => {
                let val: &TopupAgrs = unpack(input)?;
                Self::Topup(val.clone())
            }
            2 => {
                let val: &SubmitArgs = unpack(input)?;
                Self::SubmitTransaction(val.clone())
            }
            3 => {
                let val: &UpdateFeeParamsArgs = unpack(input)?;
                Self::UpdateFeeParams(val.clone())
            }
            4 => {
                let val: &TokenMintArgs = unpack(input)?;
                Self::AddAllowedToken(val.clone())
            }
            5 => {
                let val: &TokenMintArgs = unpack(input)?;
                Self::RemoveAllowedToken(val.clone())
            }
            6 => Self::ClaimFees,
            _ => return Err(ProgramError::InvalidAccountData),
        })
    }
}

/// Unpacks a reference from a bytes buffer.
pub fn unpack<T>(input: &[u8]) -> Result<&T, ProgramError> {
    if input.len() < size_of::<u8>() + size_of::<T>() {
        return Err(ProgramError::InvalidAccountData);
    }
    #[allow(clippy::cast_ptr_alignment)]
    let val: &T = unsafe { &*(&input[1] as *const u8 as *const T) };
    Ok(val)
}
