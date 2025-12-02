use crate::{
    error::GsnError,
    instruction::{GsnInstruction, UpdateFeeParamsArgs, TokenMintArgs},
    state::{FeeMode, GsnInfo},
};

use num_traits::FromPrimitive;
use solana_program::{
    account_info::next_account_info,
    account_info::AccountInfo,
    decode_error::DecodeError,
    entrypoint_deprecated::ProgramResult,
    info,
    program::invoke,
    program_error::{PrintProgramError, ProgramError},
    pubkey::Pubkey,
    system_instruction,
    // message::Message,
    // fee_calculator::FeeCalculator,
};

pub struct Processor {}

impl Processor {
    pub fn process(accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        let instruction = GsnInstruction::deserialize(input)?;
        match instruction {
            GsnInstruction::Initialize => Self::process_initialize(accounts),
            GsnInstruction::Topup(args) => {
                info!("Instruction: TopUp");
                Self::process_topup(args.amount, accounts)
            }
            GsnInstruction::SubmitTransaction(args) => {
                info!("Instruction: Submit Transaction");
                Self::process_submit_tx(args.amount, accounts)
            }
            GsnInstruction::UpdateFeeParams(args) => {
                info!("Instruction: Update Fee Params");
                Self::process_update_fee_params(args, accounts)
            }
            GsnInstruction::AddAllowedToken(args) => {
                info!("Instruction: Add Allowed Token");
                Self::process_add_allowed_token(args, accounts)
            }
            GsnInstruction::RemoveAllowedToken(args) => {
                info!("Instruction: Remove Allowed Token");
                Self::process_remove_allowed_token(args, accounts)
            }
        }
    }

    pub fn process_initialize(accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let gsn_program_info = next_account_info(account_info_iter)?;
        // Optional: authority account for governance (if provided)
        let authority_info = next_account_info(account_info_iter).ok();

        let mut gsn = GsnInfo::new();
        
        // If authority is provided, initialize governance
        if let Some(auth) = authority_info {
            if !auth.is_signer {
                return Err(GsnError::Unauthorized.into());
            }
            gsn.initialize_governance(*auth.key);
        }

        gsn.serialize(&mut gsn_program_info.data.borrow_mut())
    }

    pub fn process_topup(amount: u64, accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let gsn_program_info = next_account_info(account_info_iter)?;
        let consumer_info = next_account_info(account_info_iter)?;

        let mut gsn = GsnInfo::deserialize(gsn_program_info.data.borrow().as_ref())?;

        // TODO: deduct amount

        if gsn.consumer.contains_key(&consumer_info.key.to_string()) {
            match gsn.consumer.get(&consumer_info.key.to_string()) {
                Some(current_topup) => {
                    let val = current_topup + amount;
                    gsn.consumer
                        .entry(consumer_info.key.to_string())
                        .or_insert(val);
                }
                None => println!("has no value"),
            }
        } else {
            gsn.add_consumer(consumer_info.key.to_string(), amount);
        }

        gsn.serialize(&mut gsn_program_info.data.borrow_mut())
    }

    pub fn process_submit_tx(amount: u64, accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let target_program_info = next_account_info(account_info_iter)?;
        let sender_info = next_account_info(account_info_iter)?;
        let reciever_info = next_account_info(account_info_iter)?;
        let fee_payer_info = next_account_info(account_info_iter)?;
        let gsn_program_info = next_account_info(account_info_iter)?;

        let mut gsn = GsnInfo::deserialize(&gsn_program_info.data.borrow())?;

        if gsn.consumer.contains_key(&sender_info.key.to_string()) {
            let inst = system_instruction::transfer(&sender_info.key, &reciever_info.key, amount);

            // Calculate fee using governance configuration
            let fee = gsn.calculate_fee(amount);

            match invoke(
                &inst,
                &[
                    sender_info.clone(),
                    reciever_info.clone(),
                    target_program_info.clone(),
                ],
            ) {
                Ok(_) => {
                    if gsn.executor.contains_key(&fee_payer_info.key.to_string()) {
                        match gsn.executor.get(&fee_payer_info.key.to_string()) {
                            Some(earned_amount) => {
                                let val = earned_amount + fee;
                                gsn.executor
                                    .entry(fee_payer_info.key.to_string())
                                    .or_insert(val);
                            }
                            None => println!("has no value"),
                        }
                    } else {
                        gsn.add_executor(fee_payer_info.key.to_string(), fee);
                    }
                }
                Err(error) => return Err(error),
            }

            match gsn.consumer.get(&sender_info.key.to_string()) {
                Some(current_topup) => {
                    let val = current_topup - fee;
                    gsn.consumer
                        .entry(sender_info.key.to_string())
                        .or_insert(val);
                }
                None => println!("has no value"),
            }

            gsn.serialize(&mut gsn_program_info.data.borrow_mut())
        } else {
            return Err(ProgramError::InvalidInstructionData);
        }
    }

    pub fn process_update_fee_params(
        args: UpdateFeeParamsArgs,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let gsn_program_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;

        if !authority_info.is_signer {
            return Err(GsnError::Unauthorized.into());
        }

        let mut gsn = GsnInfo::deserialize(&gsn_program_info.data.borrow())?;

        if !gsn.is_authority(authority_info.key) {
            return Err(GsnError::Unauthorized.into());
        }

        let fee_mode = match args.fee_mode_type {
            0 => FeeMode::Fixed(args.fee_value),
            1 => {
                if args.fee_value > 10000 {
                    return Err(GsnError::InvalidFeeMode.into());
                }
                FeeMode::Percent(args.fee_value as u16)
            }
            _ => return Err(GsnError::InvalidFeeMode.into()),
        };

        gsn.update_fee_params(fee_mode);
        gsn.serialize(&mut gsn_program_info.data.borrow_mut())
    }

    pub fn process_add_allowed_token(
        args: TokenMintArgs,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let gsn_program_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;

        if !authority_info.is_signer {
            return Err(GsnError::Unauthorized.into());
        }

        let mut gsn = GsnInfo::deserialize(&gsn_program_info.data.borrow())?;

        if !gsn.is_authority(authority_info.key) {
            return Err(GsnError::Unauthorized.into());
        }

        let mint_pubkey = Pubkey::new_from_array(args.mint);
        gsn.add_allowed_token(mint_pubkey.to_string());
        gsn.serialize(&mut gsn_program_info.data.borrow_mut())
    }

    pub fn process_remove_allowed_token(
        args: TokenMintArgs,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let gsn_program_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;

        if !authority_info.is_signer {
            return Err(GsnError::Unauthorized.into());
        }

        let mut gsn = GsnInfo::deserialize(&gsn_program_info.data.borrow())?;

        if !gsn.is_authority(authority_info.key) {
            return Err(GsnError::Unauthorized.into());
        }

        let mint_pubkey = Pubkey::new_from_array(args.mint);
        gsn.remove_allowed_token(&mint_pubkey.to_string());
        gsn.serialize(&mut gsn_program_info.data.borrow_mut())
    }
}

impl PrintProgramError for GsnError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        match self {
            GsnError::AlreadyInUse => info!("Error: GSN account already in use"),
            GsnError::InvalidState => info!("Error: GSN state is not valid"),
            GsnError::Unauthorized => info!("Error: Unauthorized - not the governance authority"),
            GsnError::GovernanceNotInitialized => info!("Error: Governance not initialized"),
            GsnError::InvalidFeeMode => info!("Error: Invalid fee mode"),
        }
    }
}
