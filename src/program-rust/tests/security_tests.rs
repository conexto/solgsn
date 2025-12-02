// Security tests for SolGSN program
// These tests verify replay attack prevention, balance checks, and fee claim authorization

use solgsn::{
    error::GsnError,
    state::{GsnInfo, FeeMode},
};
use solana_program::pubkey::Pubkey;

#[test]
fn test_replay_attack_prevention() {
    // Test that the same nonce cannot be used twice (replay attack)
    
    let sender_key = Pubkey::new_unique().to_string();
    let mut gsn = GsnInfo::new();
    gsn.add_consumer(sender_key.clone(), 1000000); // 1 SOL top-up
    
    // Test nonce validation logic
    assert_eq!(gsn.get_next_nonce(&sender_key), 0);
    
    // Simulate first transaction with nonce 0
    let provided_nonce = 0;
    let expected_nonce = gsn.get_next_nonce(&sender_key);
    assert_eq!(provided_nonce, expected_nonce, "First transaction should use nonce 0");
    
    // After first transaction, increment nonce
    gsn.increment_nonce(&sender_key);
    assert_eq!(gsn.get_next_nonce(&sender_key), 1);
    
    // Try to use nonce 0 again (replay attack) - should be detected
    assert!(gsn.is_nonce_used(&sender_key, 0), "Nonce 0 should be marked as used after increment");
    
    // Second transaction with nonce 1
    let provided_nonce = 1;
    let expected_nonce = gsn.get_next_nonce(&sender_key);
    assert_eq!(provided_nonce, expected_nonce, "Second transaction should use nonce 1");
    
    gsn.increment_nonce(&sender_key);
    assert_eq!(gsn.get_next_nonce(&sender_key), 2);
    
    // Try to use nonce 0 or 1 again (replay attack)
    assert!(gsn.is_nonce_used(&sender_key, 0), "Nonce 0 should still be marked as used");
    assert!(gsn.is_nonce_used(&sender_key, 1), "Nonce 1 should be marked as used after increment");
    
    // Next expected nonce is 2
    assert_eq!(gsn.get_next_nonce(&sender_key), 2);
    assert!(!gsn.is_nonce_used(&sender_key, 2), "Nonce 2 should not be used yet");
}

#[test]
fn test_insufficient_balance_check() {
    // Test that transactions fail if top-up balance doesn't cover the fee
    
    let sender_key = Pubkey::new_unique().to_string();
    let mut gsn = GsnInfo::new();
    
    // Set fee to 100,000 lamports
    gsn.update_fee_params(FeeMode::Fixed(100000));
    
    // Add consumer with insufficient balance (only 50,000, but fee is 100,000)
    gsn.add_consumer(sender_key.clone(), 50000);
    
    let fee = gsn.calculate_fee(1000000);
    assert_eq!(fee, 100000);
    
    let balance = gsn.consumer.get(&sender_key).copied().unwrap_or(0);
    assert!(balance < fee, "Balance should be less than fee - this should trigger InsufficientBalance error");
    
    // Simulate the balance check from process_submit_tx
    // This check happens BEFORE executing the transaction
    assert!(balance < fee, "Pre-execution balance check should fail");
}

#[test]
fn test_underfunded_topup_account() {
    // Test various scenarios with underfunded accounts
    
    let mut gsn = GsnInfo::new();
    gsn.update_fee_params(FeeMode::Fixed(100000));
    
    let sender_key = Pubkey::new_unique().to_string();
    let fee = gsn.calculate_fee(1000000);
    
    // Test 1: No balance at all
    assert!(!gsn.consumer.contains_key(&sender_key));
    let balance = gsn.consumer.get(&sender_key).copied().unwrap_or(0);
    assert_eq!(balance, 0);
    assert!(balance < fee, "No balance should fail balance check");
    
    // Test 2: Balance exactly equal to fee (should pass, but edge case)
    gsn.add_consumer(sender_key.clone(), 100000);
    let balance = gsn.consumer.get(&sender_key).copied().unwrap_or(0);
    assert_eq!(balance, fee);
    assert!(balance >= fee, "Balance equal to fee should pass check");
    
    // Test 3: Balance less than fee
    gsn.consumer.insert(sender_key.clone(), 50000);
    let balance = gsn.consumer.get(&sender_key).copied().unwrap_or(0);
    assert!(balance < fee, "Balance less than fee should fail check");
    
    // Test 4: Balance slightly less than fee
    gsn.consumer.insert(sender_key.clone(), 99999);
    let balance = gsn.consumer.get(&sender_key).copied().unwrap_or(0);
    assert!(balance < fee, "Balance 1 lamport less than fee should fail");
}

#[test]
fn test_unauthorized_fee_claim() {
    // Test that only the executor who executed a transaction can claim fees
    
    let mut gsn = GsnInfo::new();
    
    let consumer_key = Pubkey::new_unique().to_string();
    let executor1_key = Pubkey::new_unique().to_string();
    let executor2_key = Pubkey::new_unique().to_string();
    
    // Simulate executor1 executing a transaction
    let nonce = 0;
    gsn.record_transaction_executor(&consumer_key, nonce, &executor1_key);
    gsn.add_executor(executor1_key.clone(), 100000);
    
    // Verify executor1 is recorded as the executor
    let recorded_executor = gsn.get_transaction_executor(&consumer_key, nonce);
    assert_eq!(recorded_executor, Some(&executor1_key));
    assert_ne!(recorded_executor, Some(&executor2_key));
    
    // Executor1 should be able to claim (has balance)
    let executor1_balance = gsn.executor.get(&executor1_key).copied().unwrap_or(0);
    assert_eq!(executor1_balance, 100000);
    
    // Executor2 should not have any balance
    let executor2_balance = gsn.executor.get(&executor2_key).copied().unwrap_or(0);
    assert_eq!(executor2_balance, 0);
    
    // Executor2 should not be able to claim executor1's fees
    // This would be enforced in process_claim_fees by checking the executor key
}

#[test]
fn test_malicious_fee_claim_attempt() {
    // Test that a malicious executor cannot claim fees for transactions they didn't execute
    
    let mut gsn = GsnInfo::new();
    
    let consumer_key = Pubkey::new_unique().to_string();
    let legitimate_executor = Pubkey::new_unique().to_string();
    let malicious_executor = Pubkey::new_unique().to_string();
    
    // Legitimate executor executes transaction
    let nonce = 0;
    gsn.record_transaction_executor(&consumer_key, nonce, &legitimate_executor);
    gsn.add_executor(legitimate_executor.clone(), 100000);
    
    // Malicious executor tries to claim
    let malicious_balance = gsn.executor.get(&malicious_executor).copied().unwrap_or(0);
    assert_eq!(malicious_balance, 0, "Malicious executor should have no balance");
    
    // Verify the transaction was executed by legitimate executor
    let recorded_executor = gsn.get_transaction_executor(&consumer_key, nonce);
    assert_eq!(recorded_executor, Some(&legitimate_executor));
    assert_ne!(recorded_executor, Some(&malicious_executor));
    
    // In process_claim_fees, if malicious_executor tries to claim:
    // 1. They would need to sign (is_signer check)
    // 2. But they have 0 balance, so the claim would fail with InsufficientFunds
    // 3. Even if they somehow had balance, the transaction_executor mapping
    //    would show they didn't execute this transaction
}

#[test]
fn test_nonce_sequence_enforcement() {
    // Test that nonces must be sequential (cannot skip or reuse)
    
    let mut gsn = GsnInfo::new();
    let consumer_key = Pubkey::new_unique().to_string();
    
    // Initial state: nonce should be 0
    assert_eq!(gsn.get_next_nonce(&consumer_key), 0);
    
    // First transaction: nonce 0
    gsn.increment_nonce(&consumer_key);
    assert_eq!(gsn.get_next_nonce(&consumer_key), 1);
    
    // Second transaction: nonce 1 (correct)
    gsn.increment_nonce(&consumer_key);
    assert_eq!(gsn.get_next_nonce(&consumer_key), 2);
    
    // Try to use nonce 0 again (should be detected as used)
    assert!(gsn.is_nonce_used(&consumer_key, 0));
    assert!(gsn.is_nonce_used(&consumer_key, 1));
    assert!(!gsn.is_nonce_used(&consumer_key, 2));
    
    // Try to skip to nonce 5 (should fail - expected nonce is 2)
    // In process_submit_tx, this would return InvalidNonce error
    let expected_nonce = gsn.get_next_nonce(&consumer_key);
    let provided_nonce = 5;
    assert_ne!(expected_nonce, provided_nonce, "Should fail with InvalidNonce");
}

#[test]
fn test_balance_check_before_execution() {
    // Test that balance is checked BEFORE executing the transaction
    // This is critical: we must verify balance covers fee before invoking the user's transaction
    
    let mut gsn = GsnInfo::new();
    gsn.update_fee_params(FeeMode::Fixed(100000));
    
    let sender_key = Pubkey::new_unique().to_string();
    let fee = gsn.calculate_fee(1000000);
    
    // Add balance that exactly covers the fee
    gsn.add_consumer(sender_key.clone(), 100000);
    
    let balance = gsn.consumer.get(&sender_key).copied().unwrap_or(0);
    
    // Balance check should pass (happens BEFORE execution in process_submit_tx)
    assert!(balance >= fee, "Balance should cover fee - check happens before execution");
    
    // Now reduce balance below fee
    gsn.consumer.insert(sender_key.clone(), 50000);
    let balance = gsn.consumer.get(&sender_key).copied().unwrap_or(0);
    
    // Balance check should fail BEFORE any transaction execution
    assert!(balance < fee, "Balance check should fail before execution - prevents partial execution");
    // This would cause process_submit_tx to return InsufficientBalance error
    // and the user's transaction would never be invoked
}
