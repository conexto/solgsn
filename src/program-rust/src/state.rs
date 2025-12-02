use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    program_error::ProgramError,
    pubkey::Pubkey,
};
use std::collections::BTreeMap;

/// Fee calculation mode
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum FeeMode {
    /// Fixed fee amount in lamports
    Fixed(u64),
    /// Percentage fee (basis points, e.g., 100 = 1%)
    Percent(u16),
}

/// Governance configuration
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct GovernanceConfig {
    /// Authority address that can update governance parameters
    pub authority: Pubkey,
    /// Fee calculation mode
    pub fee_mode: FeeMode,
    /// Set of allowed token mint addresses (empty means all tokens allowed)
    pub allowed_tokens: BTreeMap<String, bool>,
}

#[derive(Default, BorshSerialize, BorshDeserialize)]
pub struct GsnInfo {
    pub is_initialized: bool,
    pub consumer: BTreeMap<String, u64>,
    pub executor: BTreeMap<String, u64>,
    pub governance: Option<GovernanceConfig>,
    /// Nonce tracking per consumer to prevent replay attacks
    pub consumer_nonces: BTreeMap<String, u64>,
    /// Track which executor executed which transaction (by nonce)
    /// Key: format!("{}:{}", consumer_address, nonce), Value: executor_address
    pub transaction_executor: BTreeMap<String, String>,
}

impl GsnInfo {
    pub fn serialize(&self, mut data: &mut [u8]) -> Result<(), ProgramError> {
        BorshSerialize::serialize(self, &mut data).map_err(|_| ProgramError::AccountDataTooSmall)
    }

    pub fn deserialize(mut data: &[u8]) -> Result<Self, ProgramError> {
        BorshDeserialize::deserialize(&mut data).map_err(|_| ProgramError::InvalidAccountData)
    }

    pub fn add_consumer(&mut self, address: String, amount: u64) -> bool {
        self.consumer.insert(address, amount);
        true
    }

    pub fn add_executor(&mut self, address: String, amount: u64) -> bool {
        self.executor.insert(address, amount);
        true
    }

    pub fn new() -> Self {
        Self {
            is_initialized: true,
            consumer: BTreeMap::new(),
            executor: BTreeMap::new(),
            governance: None,
            consumer_nonces: BTreeMap::new(),
            transaction_executor: BTreeMap::new(),
        }
    }

    /// Initialize governance with default authority
    pub fn initialize_governance(&mut self, authority: Pubkey) {
        self.governance = Some(GovernanceConfig {
            authority,
            fee_mode: FeeMode::Fixed(50000), // Default 50,000 lamports
            allowed_tokens: BTreeMap::new(), // Empty means all tokens allowed
        });
    }

    /// Calculate fee based on governance configuration
    pub fn calculate_fee(&self, transaction_amount: u64) -> u64 {
        match &self.governance {
            Some(gov) => match &gov.fee_mode {
                FeeMode::Fixed(amount) => *amount,
                FeeMode::Percent(basis_points) => {
                    // Calculate percentage: (amount * basis_points) / 10000
                    (transaction_amount as u128 * *basis_points as u128 / 10000) as u64
                }
            },
            None => 50000, // Default fallback
        }
    }

    /// Check if a token is allowed for fee payment
    pub fn is_token_allowed(&self, token_mint: &str) -> bool {
        match &self.governance {
            Some(gov) => {
                // If allowed_tokens is empty, all tokens are allowed
                if gov.allowed_tokens.is_empty() {
                    true
                } else {
                    gov.allowed_tokens.get(token_mint).copied().unwrap_or(false)
                }
            }
            None => true, // If no governance, all tokens allowed
        }
    }

    /// Add an allowed token
    pub fn add_allowed_token(&mut self, token_mint: String) {
        if let Some(gov) = &mut self.governance {
            gov.allowed_tokens.insert(token_mint, true);
        }
    }

    /// Remove an allowed token
    pub fn remove_allowed_token(&mut self, token_mint: &str) {
        if let Some(gov) = &mut self.governance {
            gov.allowed_tokens.remove(token_mint);
        }
    }

    /// Update fee parameters
    pub fn update_fee_params(&mut self, fee_mode: FeeMode) {
        if let Some(gov) = &mut self.governance {
            gov.fee_mode = fee_mode;
        }
    }

    /// Check if an address is the governance authority
    pub fn is_authority(&self, address: &Pubkey) -> bool {
        match &self.governance {
            Some(gov) => gov.authority == *address,
            None => false,
        }
    }

    /// Get the next nonce for a consumer
    pub fn get_next_nonce(&self, consumer: &str) -> u64 {
        self.consumer_nonces.get(consumer).copied().unwrap_or(0)
    }

    /// Increment and return the nonce for a consumer
    pub fn increment_nonce(&mut self, consumer: &str) -> u64 {
        let current_nonce = self.get_next_nonce(consumer);
        let next_nonce = current_nonce + 1;
        self.consumer_nonces.insert(consumer.to_string(), next_nonce);
        next_nonce
    }

    /// Check if a nonce has been used (replay protection)
    /// Returns true if the nonce is less than the next expected nonce
    pub fn is_nonce_used(&self, consumer: &str, nonce: u64) -> bool {
        let next_nonce = self.get_next_nonce(consumer);
        nonce < next_nonce
    }

    /// Record which executor executed a transaction
    pub fn record_transaction_executor(&mut self, consumer: &str, nonce: u64, executor: &str) {
        let key = format!("{}:{}", consumer, nonce);
        self.transaction_executor.insert(key, executor.to_string());
    }

    /// Get the executor that executed a specific transaction
    pub fn get_transaction_executor(&self, consumer: &str, nonce: u64) -> Option<&String> {
        let key = format!("{}:{}", consumer, nonce);
        self.transaction_executor.get(&key)
    }
}
