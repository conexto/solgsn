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
    msg,
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
                Self::process_submit_tx(args.amount, args.nonce, accounts)
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
            GsnInstruction::ClaimFees => {
                info!("Instruction: Claim Fees");
                Self::process_claim_fees(accounts)
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

        let previous_balance = gsn.consumer.get(&consumer_info.key.to_string()).copied().unwrap_or(0);
        let new_balance;

        if gsn.consumer.contains_key(&consumer_info.key.to_string()) {
            match gsn.consumer.get(&consumer_info.key.to_string()) {
                Some(current_topup) => {
                    let val = current_topup + amount;
                    gsn.consumer
                        .entry(consumer_info.key.to_string())
                        .or_insert(val);
                    new_balance = val;
                }
                None => {
                    println!("has no value");
                    gsn.add_consumer(consumer_info.key.to_string(), amount);
                    new_balance = amount;
                }
            }
        } else {
            gsn.add_consumer(consumer_info.key.to_string(), amount);
            new_balance = amount;
        }

        msg!(
            "[TOPUP] consumer={} amount={} previous_balance={} new_balance={}",
            consumer_info.key.to_string(),
            amount,
            previous_balance,
            new_balance
        );

        gsn.serialize(&mut gsn_program_info.data.borrow_mut())
    }

    pub fn process_submit_tx(amount: u64, nonce: u64, accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let target_program_info = next_account_info(account_info_iter)?;
        let sender_info = next_account_info(account_info_iter)?;
        let reciever_info = next_account_info(account_info_iter)?;
        let fee_payer_info = next_account_info(account_info_iter)?;
        let gsn_program_info = next_account_info(account_info_iter)?;

        let mut gsn = GsnInfo::deserialize(&gsn_program_info.data.borrow())?;

        let sender_key = sender_info.key.to_string();

        // Check if consumer exists
        if !gsn.consumer.contains_key(&sender_key) {
            return Err(ProgramError::InvalidInstructionData);
        }

        // SECURITY CHECK 1: Verify nonce to prevent replay attacks
        let expected_nonce = gsn.get_next_nonce(&sender_key);
        if nonce != expected_nonce {
            return Err(GsnError::InvalidNonce.into());
        }

        // Additional replay protection: check if nonce was already used
        if gsn.is_nonce_used(&sender_key, nonce) {
            return Err(GsnError::ReplayAttack.into());
        }

        // Calculate fee using governance configuration
        let fee = gsn.calculate_fee(amount);

        // SECURITY CHECK 2: Verify top-up balance covers expected fee BEFORE execution
        let current_balance = gsn.consumer.get(&sender_key)
            .copied()
            .ok_or(GsnError::InsufficientBalance)?;
        
        if current_balance < fee {
            msg!(
                "[EXECUTION_FAILED] reason=insufficient_balance consumer={} required_fee={} available_balance={}",
                sender_key,
                fee,
                current_balance
            );
            return Err(GsnError::InsufficientBalance.into());
        }

        msg!(
            "[EXECUTION_START] consumer={} executor={} amount={} fee={} nonce={}",
            sender_key,
            fee_payer_info.key.to_string(),
            amount,
            fee,
            nonce
        );

        // Execute the transaction
        let inst = system_instruction::transfer(&sender_info.key, &reciever_info.key, amount);

        match invoke(
            &inst,
            &[
                sender_info.clone(),
                reciever_info.clone(),
                target_program_info.clone(),
            ],
        ) {
            Ok(_) => {
                msg!(
                    "[EXECUTION_SUCCESS] consumer={} executor={} amount={}",
                    sender_key,
                    fee_payer_info.key.to_string(),
                    amount
                );

                // SECURITY CHECK 3: Record transaction-executor mapping before updating balances
                gsn.record_transaction_executor(&sender_key, nonce, &fee_payer_info.key.to_string());
                
                // Increment nonce to prevent replay
                gsn.increment_nonce(&sender_key);

                // Update executor balance
                let executor_previous_balance = gsn.executor.get(&fee_payer_info.key.to_string()).copied().unwrap_or(0);
                let executor_new_balance;
                if gsn.executor.contains_key(&fee_payer_info.key.to_string()) {
                    match gsn.executor.get(&fee_payer_info.key.to_string()) {
                        Some(earned_amount) => {
                            let val = earned_amount + fee;
                            gsn.executor
                                .entry(fee_payer_info.key.to_string())
                                .or_insert(val);
                            executor_new_balance = val;
                        }
                        None => {
                            println!("has no value");
                            gsn.add_executor(fee_payer_info.key.to_string(), fee);
                            executor_new_balance = fee;
                        }
                    }
                } else {
                    gsn.add_executor(fee_payer_info.key.to_string(), fee);
                    executor_new_balance = fee;
                }

                // Deduct fee from consumer balance
                let val = current_balance - fee;
                gsn.consumer
                    .entry(sender_key.clone())
                    .or_insert(val);

                msg!(
                    "[FEE_DEDUCTION] consumer={} fee={} previous_balance={} new_balance={}",
                    sender_key,
                    fee,
                    current_balance,
                    val
                );

                msg!(
                    "[EXECUTOR_CREDIT] executor={} fee={} previous_balance={} new_balance={}",
                    fee_payer_info.key.to_string(),
                    fee,
                    executor_previous_balance,
                    executor_new_balance
                );
            }
            Err(error) => {
                msg!(
                    "[EXECUTION_FAILED] consumer={} executor={} error={:?}",
                    sender_key,
                    fee_payer_info.key.to_string(),
                    error
                );
                return Err(error);
            }
        }

        gsn.serialize(&mut gsn_program_info.data.borrow_mut())
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

    pub fn process_claim_fees(accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let gsn_program_info = next_account_info(account_info_iter)?;
        let executor_info = next_account_info(account_info_iter)?;
        let destination_info = next_account_info(account_info_iter)?;
        let system_program_info = next_account_info(account_info_iter)?;

        // SECURITY CHECK: Only the executor can claim their own fees
        if !executor_info.is_signer {
            msg!(
                "[EXECUTOR_CLAIM_FAILED] executor={} reason=not_signer",
                executor_info.key.to_string()
            );
            return Err(GsnError::UnauthorizedFeeClaim.into());
        }

        let executor_key = executor_info.key.to_string();

        // Verify the executor is claiming fees to their own account
        if executor_info.key != destination_info.key {
            msg!(
                "[EXECUTOR_CLAIM_FAILED] executor={} destination={} reason=destination_mismatch",
                executor_key,
                destination_info.key.to_string()
            );
            return Err(GsnError::UnauthorizedFeeClaim.into());
        }

        let mut gsn = GsnInfo::deserialize(&gsn_program_info.data.borrow())?;

        // Get the executor's earned fees
        let earned_fees = gsn.executor
            .get(&executor_key)
            .copied()
            .unwrap_or(0);

        if earned_fees == 0 {
            msg!(
                "[EXECUTOR_CLAIM_FAILED] executor={} reason=insufficient_funds earned_fees=0",
                executor_key
            );
            return Err(ProgramError::InsufficientFunds);
        }

        msg!(
            "[EXECUTOR_CLAIM_START] executor={} amount={}",
            executor_key,
            earned_fees
        );

        // Transfer fees to executor
        // Note: This assumes gsn_program_info is a program-owned account that holds SOL
        // The account must be writable and owned by the system program or this program
        let transfer_instruction = system_instruction::transfer(
            gsn_program_info.key,
            destination_info.key,
            earned_fees,
        );

        match invoke(
            &transfer_instruction,
            &[
                gsn_program_info.clone(),
                destination_info.clone(),
                system_program_info.clone(),
            ],
        ) {
            Ok(_) => {
                msg!(
                    "[EXECUTOR_CLAIM_SUCCESS] executor={} amount={}",
                    executor_key,
                    earned_fees
                );
            }
            Err(error) => {
                msg!(
                    "[EXECUTOR_CLAIM_FAILED] executor={} amount={} error={:?}",
                    executor_key,
                    earned_fees,
                    error
                );
                return Err(error);
            }
        }

        // Reset executor's earned balance
        gsn.executor.insert(executor_key.clone(), 0);

        msg!(
            "[EXECUTOR_CLAIM_COMPLETE] executor={} claimed_amount={} remaining_balance=0",
            executor_key,
            earned_fees
        );

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
            GsnError::InsufficientBalance => info!("Error: Insufficient balance in top-up account"),
            GsnError::ReplayAttack => info!("Error: Replay attack detected"),
            GsnError::InvalidNonce => info!("Error: Invalid nonce"),
            GsnError::UnauthorizedFeeClaim => info!("Error: Unauthorized fee claim"),
        }
    }
}
