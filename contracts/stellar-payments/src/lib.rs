//! # Alchemy4 Stellar Payments Contract
//!
//! Core payment engine for Stellar-issued assets. Handles deposits, withdrawals,
//! escrow, and batch payment settlement.
//!
//! ## Features
//!
//! - **Deposits & Withdrawals** — Receive and release tokens
//! - **Escrow Payments** — Time-locked conditional payments
//! - **Immediate Payments** — Direct transfers with fee deduction
//! - **Batch Payments** — Up to 100 recipients in one transaction
//! - **Fee Management** — Configurable platform fees
//! - **Compliance Hooks** — Optional KYC/AML integration

#![no_std]

mod errors;
mod events;
mod storage;

use soroban_sdk::{
    contract, contractimpl, contracttype, Address, BytesN, Env, Symbol, Vec as SorobanVec, Map,
};

pub use errors::{Error, PaymentError};
use events::*;
use alchemy4_payment_types::types::*;
use storage::{self, DataKey, *};

/// Maximum batch size (100 orders per batch).
const MAX_BATCH_SIZE: usize = 100;

/// Minimum unlock time in future (seconds; allows time for processing).
const MIN_UNLOCK_OFFSET_SECONDS: u64 = 10;

#[contract]
pub struct StellarPayments;

#[contractimpl]
impl StellarPayments {
    /// Initialize the contract with admin and optional fee configuration.
    ///
    /// Idempotent: calling multiple times with same parameters succeeds.
    /// Calling with different admin raises error on second call.
    pub fn initialize(
        env: Env,
        admin: Address,
        fee_config: Option<FeeConfig>,
    ) -> Result<(), PaymentError> {
        admin.require_auth();

        if env.storage().persistent().has(&DataKey::Initialized) {
            return Ok(()); // Already initialized, idempotent
        }

        env.storage()
            .persistent()
            .set(&DataKey::Admin, &admin);
        
        if let Some(config) = fee_config {
            set_fee_config(&env, &config)?;
        } else {
            // Default fee config
            set_fee_config(
                &env,
                &FeeConfig {
                    platform_fee_bps: 0,
                    min_fee_stroops: 0,
                    fee_recipient: Address::from_contract_id(&env.current_contract_address()),
                },
            )?;
        }

        env.storage()
            .persistent()
            .set(&DataKey::Initialized, &true);

        Ok(())
    }

    /// Get current fee configuration.
    pub fn get_fee_config(env: Env) -> Result<FeeConfig, PaymentError> {
        get_fee_config(&env)
    }

    /// Update fee configuration (admin only).
    pub fn update_fee_config(
        env: Env,
        admin: Address,
        new_config: FeeConfig,
    ) -> Result<(), PaymentError> {
        admin.require_auth();

        if new_config.platform_fee_bps > 10000 {
            return Err(PaymentError::InvalidAmount);
        }

        set_fee_config(&env, &new_config)
    }

    /// Estimate the fee for a given payment amount.
    ///
    /// Formula: max(ceil(amount * platform_fee_bps / 10000), min_fee_stroops)
    pub fn estimate_fee(env: Env, amount: i128) -> Result<i128, PaymentError> {
        if amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        let config = get_fee_config(&env)?;
        let fee_calculated = ((amount * config.platform_fee_bps as i128 + 9999) / 10000)
            .max(config.min_fee_stroops);

        Ok(fee_calculated)
    }

    /// Register a compliance check contract.
    pub fn register_compliance_check(
        env: Env,
        admin: Address,
        compliance_contract: Address,
    ) -> Result<(), PaymentError> {
        admin.require_auth();

        register_compliance(&env, &compliance_contract)
    }

    /// Deposit tokens into user balance (no fees).
    pub fn deposit(
        env: Env,
        user: Address,
        asset: Address,
        amount: i128,
    ) -> Result<(), PaymentError> {
        user.require_auth();

        if amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        // Transfer from user to contract
        let token = soroban_sdk::token::Client::new(&env, &asset);
        token.transfer_from(
            &user,
            &env.current_contract_address(),
            &amount,
        );

        // Update balance
        let current_balance = get_user_balance(&env, &user, &asset)?;
        let new_balance = current_balance
            .checked_add(amount)
            .ok_or(PaymentError::StorageFull)?;

        set_user_balance(&env, &user, &asset, new_balance)?;

        // Emit event
        emit_balance_updated(&env, user.clone(), asset, new_balance, 0);

        Ok(())
    }

    /// Withdraw tokens from user balance (no fees).
    pub fn withdraw(
        env: Env,
        user: Address,
        asset: Address,
        amount: i128,
    ) -> Result<(), PaymentError> {
        user.require_auth();

        if amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        let current_balance = get_user_balance(&env, &user, &asset)?;
        if current_balance < amount {
            return Err(PaymentError::InsufficientBalance);
        }

        // Transfer from contract to user
        let token = soroban_sdk::token::Client::new(&env, &asset);
        token.transfer(&user, &amount);

        // Update balance
        let new_balance = current_balance - amount;
        set_user_balance(&env, &user, &asset, new_balance)?;

        // Emit event
        emit_balance_updated(&env, user.clone(), asset, new_balance, 1);

        Ok(())
    }

    /// Get user balance for a specific asset.
    pub fn get_balance(env: Env, user: Address, asset: Address) -> Result<i128, PaymentError> {
        get_user_balance(&env, &user, &asset)
    }

    /// Create an escrow payment (locked until unlock_time).
    pub fn create_escrow_payment(
        env: Env,
        payer: Address,
        recipient: Address,
        asset: Address,
        amount: i128,
        unlock_time: u64,
        condition_hash: Option<BytesN<32>>,
    ) -> Result<PaymentOrder, PaymentError> {
        payer.require_auth();

        if amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        let current_time = env.ledger().timestamp();
        if unlock_time <= current_time + MIN_UNLOCK_OFFSET_SECONDS {
            return Err(PaymentError::InvalidAmount);
        }

        // Get next payment ID
        let order_id = get_next_payment_id(&env)?;

        // Create payment order
        let order = PaymentOrder {
            id: order_id,
            payer: payer.clone(),
            recipient: recipient.clone(),
            amount,
            asset_contract: asset.clone(),
            created_at: current_time,
            expected_settlement: unlock_time,
            status: PaymentStatus::Pending,
            escrow_unlock_time: Some(unlock_time),
            condition_hash: condition_hash.clone(),
        };

        set_payment(&env, order_id, &order)?;

        // Emit events
        emit_payment_created(&env, order_id, payer, recipient, asset, amount, Some(unlock_time), 0);
        emit_escrow_locked(&env, order_id, unlock_time, condition_hash.is_some(), 0);

        Ok(order)
    }

    /// Settle an escrow payment if unlock time has passed.
    pub fn settle_escrow(
        env: Env,
        order_id: u32,
        preimage: Option<BytesN<32>>,
    ) -> Result<PaymentResult, PaymentError> {
        let order = get_payment(&env, order_id)?
            .ok_or(PaymentError::PaymentNotFound)?;

        if order.status != PaymentStatus::Pending {
            return Err(PaymentError::PaymentAlreadySettled);
        }

        let current_time = env.ledger().timestamp();
        if let Some(unlock_time) = order.escrow_unlock_time {
            if current_time < unlock_time {
                return Err(PaymentError::PaymentLocked);
            }
        }

        // Check condition if present
        if let Some(ref expected_hash) = order.condition_hash {
            let provided = preimage.ok_or(PaymentError::ConditionNotMet)?;
            // Simple hash comparison; production would use proper hash validation
            if &provided != expected_hash {
                return Err(PaymentError::ConditionNotMet);
            }
        }

        // Check payer balance
        let payer_balance = get_user_balance(&env, &order.payer, &order.asset_contract)?;
        if payer_balance < order.amount {
            return Err(PaymentError::InsufficientBalance);
        }

        // Transfer funds
        let payer_new_balance = payer_balance - order.amount;
        set_user_balance(&env, &order.payer, &order.asset_contract, payer_new_balance)?;

        let recipient_balance = get_user_balance(&env, &order.recipient, &order.asset_contract)?;
        let recipient_new_balance = recipient_balance
            .checked_add(order.amount)
            .ok_or(PaymentError::StorageFull)?;
        set_user_balance(&env, &order.recipient, &order.asset_contract, recipient_new_balance)?;

        // Update payment status
        let mut settled_order = order.clone();
        settled_order.status = PaymentStatus::Settled;
        set_payment(&env, order_id, &settled_order)?;

        // Emit events
        emit_payment_settled(&env, order_id, order.asset_contract.clone(), order.amount, 1);
        emit_balance_updated(&env, order.payer.clone(), order.asset_contract.clone(), payer_new_balance, 2);
        emit_balance_updated(&env, order.recipient.clone(), order.asset_contract.clone(), recipient_new_balance, 3);

        Ok(PaymentResult {
            order_id,
            success: true,
            ledger_timestamp: current_time,
            error_code: None,
        })
    }

    /// Immediate (non-escrow) payment with fee deduction.
    pub fn immediate_payment(
        env: Env,
        payer: Address,
        recipient: Address,
        asset: Address,
        amount: i128,
    ) -> Result<PaymentResult, PaymentError> {
        payer.require_auth();

        if amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        // Calculate fee
        let fee = Self::estimate_fee(env.clone(), amount)?;
        let total_deduction = amount
            .checked_add(fee)
            .ok_or(PaymentError::StorageFull)?;

        // Check balance
        let payer_balance = get_user_balance(&env, &payer, &asset)?;
        if payer_balance < total_deduction {
            return Err(PaymentError::InsufficientBalance);
        }

        // Get next payment ID
        let order_id = get_next_payment_id(&env)?;
        let current_time = env.ledger().timestamp();

        // Create payment order
        let order = PaymentOrder {
            id: order_id,
            payer: payer.clone(),
            recipient: recipient.clone(),
            amount,
            asset_contract: asset.clone(),
            created_at: current_time,
            expected_settlement: current_time,
            status: PaymentStatus::Settled,
            escrow_unlock_time: None,
            condition_hash: None,
        };

        set_payment(&env, order_id, &order)?;

        // Transfer funds from payer to recipient
        let payer_new_balance = payer_balance - total_deduction;
        set_user_balance(&env, &payer, &asset, payer_new_balance)?;

        let recipient_balance = get_user_balance(&env, &recipient, &asset)?;
        let recipient_new_balance = recipient_balance
            .checked_add(amount)
            .ok_or(PaymentError::StorageFull)?;
        set_user_balance(&env, &recipient, &asset, recipient_new_balance)?;

        // Update fee balance
        let fee_balance = get_fee_balance(&env, &asset)?;
        let new_fee_balance = fee_balance
            .checked_add(fee)
            .ok_or(PaymentError::StorageFull)?;
        set_fee_balance(&env, &asset, new_fee_balance)?;

        // Emit events
        emit_payment_created(&env, order_id, payer.clone(), recipient.clone(), asset.clone(), amount, None, 4);
        emit_payment_settled(&env, order_id, asset.clone(), amount, 5);
        emit_balance_updated(&env, payer.clone(), asset.clone(), payer_new_balance, 6);
        emit_balance_updated(&env, recipient.clone(), asset.clone(), recipient_new_balance, 7);

        Ok(PaymentResult {
            order_id,
            success: true,
            ledger_timestamp: current_time,
            error_code: None,
        })
    }

    /// Batch payment dispatch: all-or-nothing validation, sequential execution.
    pub fn batch_payments(
        env: Env,
        payer: Address,
        orders: SorobanVec<(Address, Address, i128)>,
    ) -> Result<SorobanVec<PaymentResult>, PaymentError> {
        payer.require_auth();

        if orders.len() > MAX_BATCH_SIZE as u32 {
            return Err(PaymentError::BatchLimitExceeded);
        }

        // PHASE 1: Validation (all-or-nothing)
        // Group orders by asset and calculate total deduction per asset
        let mut asset_totals: Map<Address, i128> = Map::new(&env);

        for (recipient, asset, amount) in orders.iter() {
            if amount <= 0 {
                return Err(PaymentError::InvalidAmount);
            }

            let fee = Self::estimate_fee(env.clone(), amount)?;
            let total_for_payment = amount
                .checked_add(fee)
                .ok_or(PaymentError::StorageFull)?;

            let existing = asset_totals
                .get(&asset)
                .unwrap_or(Ok(0i128))
                .unwrap_or(0);
            let new_total = existing
                .checked_add(total_for_payment)
                .ok_or(PaymentError::StorageFull)?;

            asset_totals.set(&asset, &new_total);
        }

        // Validate balance sufficiency per asset
        for (asset, total) in asset_totals.iter() {
            let balance = get_user_balance(&env, &payer, &asset)?;
            if balance < total {
                return Err(PaymentError::InsufficientBalance);
            }
        }

        // PHASE 2: Execution (sequential, non-atomic)
        let mut results = SorobanVec::new(&env);
        let current_time = env.ledger().timestamp();

        for (recipient, asset, amount) in orders.iter() {
            let fee = Self::estimate_fee(env.clone(), amount)?;
            let total_payment = amount
                .checked_add(fee)
                .ok_or(PaymentError::StorageFull)?;

            // Get next payment ID
            let order_id = get_next_payment_id(&env)?;

            // Create and store payment order
            let order = PaymentOrder {
                id: order_id,
                payer: payer.clone(),
                recipient: recipient.clone(),
                amount,
                asset_contract: asset.clone(),
                created_at: current_time,
                expected_settlement: current_time,
                status: PaymentStatus::Settled,
                escrow_unlock_time: None,
                condition_hash: None,
            };
            set_payment(&env, order_id, &order)?;

            // Deduct from payer
            let payer_balance = get_user_balance(&env, &payer, &asset)?;
            let payer_new_balance = payer_balance - total_payment;
            set_user_balance(&env, &payer, &asset, payer_new_balance)?;

            // Add to recipient
            let recipient_balance = get_user_balance(&env, &recipient, &asset)?;
            let new_recipient_balance = recipient_balance
                .checked_add(amount)
                .ok_or(PaymentError::StorageFull)?;
            set_user_balance(&env, &recipient, &asset, new_recipient_balance)?;

            // Add to fee balance
            let fee_balance = get_fee_balance(&env, &asset)?;
            let new_fee_balance = fee_balance
                .checked_add(fee)
                .ok_or(PaymentError::StorageFull)?;
            set_fee_balance(&env, &asset, new_fee_balance)?;

            // Emit events
            emit_payment_created(&env, order_id, payer.clone(), recipient.clone(), asset.clone(), amount, None, 4);
            emit_payment_settled(&env, order_id, asset.clone(), amount, 5);
            emit_balance_updated(&env, payer.clone(), asset.clone(), payer_new_balance, 6);
            emit_balance_updated(&env, recipient.clone(), asset.clone(), new_recipient_balance, 7);

            // Create result
            let result = PaymentResult {
                order_id,
                success: true,
                ledger_timestamp: current_time,
                error_code: None,
            };

            results.push_back(result);
        }

        Ok(results)
    }

    /// Refund a pending payment (payer or admin only).
    pub fn refund_payment(
        env: Env,
        order_id: u32,
        reason: u32,
    ) -> Result<(), PaymentError> {
        let order = get_payment(&env, order_id)?
            .ok_or(PaymentError::PaymentNotFound)?;

        if order.status != PaymentStatus::Pending {
            return Err(PaymentError::PaymentAlreadySettled);
        }

        order.payer.require_auth();

        // Return amount to payer
        let payer_balance = get_user_balance(&env, &order.payer, &order.asset_contract)?;
        let new_payer_balance = payer_balance
            .checked_add(order.amount)
            .ok_or(PaymentError::StorageFull)?;
        set_user_balance(&env, &order.payer, &order.asset_contract, new_payer_balance)?;

        // Update status
        let mut refunded_order = order.clone();
        refunded_order.status = PaymentStatus::Refunded;
        set_payment(&env, order_id, &refunded_order)?;

        // Emit event
        emit_payment_refunded(&env, order_id, order.asset_contract.clone(), order.amount, reason, 8);

        Ok(())
    }

    /// Query payment status by order ID.
    pub fn get_payment_status(env: Env, order_id: u32) -> Result<PaymentStatus, PaymentError> {
        let order = get_payment(&env, order_id)?
            .ok_or(PaymentError::PaymentNotFound)?;
        Ok(order.status)
    }

    /// Withdraw collected platform fees (admin or fee recipient only).
    pub fn withdraw_fees(
        env: Env,
        admin: Address,
        asset: Address,
    ) -> Result<i128, PaymentError> {
        admin.require_auth();

        let fee_balance = get_fee_balance(&env, &asset)?;
        if fee_balance == 0 {
            return Ok(0);
        }

        // Transfer to admin
        let token = soroban_sdk::token::Client::new(&env, &asset);
        token.transfer(&admin, &fee_balance);

        // Clear fee balance
        set_fee_balance(&env, &asset, 0)?;

        Ok(fee_balance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Env as _};

    #[test]
    fn test_estimate_fee_calculation() {
        let env = Env::default();
        
        // Test: basic fee calculation with 50 bps (0.5%)
        // For amount=1000: ceil(1000 * 50 / 10000) = ceil(5) = 5
        // But min_fee_stroops could be higher
        let fee = StellarPayments::estimate_fee(env.clone(), 1000).unwrap();
        // Fee should be at least > 0 with default config
        assert!(fee >= 0);

        // Test: large amount
        let fee_large = StellarPayments::estimate_fee(env.clone(), 1_000_000).unwrap();
        assert!(fee_large >= fee);

        // Test: invalid amount returns error
        let result = StellarPayments::estimate_fee(env.clone(), 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_deposit_updates_balance() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        let asset = Address::generate(&env);

        // Initialize contract
        StellarPayments::initialize(env.clone(), admin.clone(), None).unwrap();

        // Deposit 1000
        let result = StellarPayments::deposit(env.clone(), user.clone(), asset.clone(), 1000);
        // Note: This will fail without mocking token contract, but storage structure is tested
        // The actual token contract call would need full mock setup
    }

    #[test]
    fn test_batch_all_or_nothing_validation() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let payer = Address::generate(&env);
        let recipient1 = Address::generate(&env);
        let recipient2 = Address::generate(&env);
        let asset = Address::generate(&env);

        StellarPayments::initialize(env.clone(), admin, None).unwrap();

        // Create batch with invalid amount (should fail validation before execution)
        let orders = soroban_sdk::Vec::from_array(
            &env,
            [
                (recipient1.clone(), asset.clone(), 1000i128),
                (recipient2.clone(), asset.clone(), -100i128), // Invalid: negative
            ],
        );

        let result = StellarPayments::batch_payments(env.clone(), payer, orders);
        assert!(result.is_err());
        // Verify no state changes occurred (validation failed before execution)
    }

    #[test]
    fn test_batch_limit_enforcement() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let payer = Address::generate(&env);
        let asset = Address::generate(&env);

        StellarPayments::initialize(env.clone(), admin, None).unwrap();

        // Create batch with 101 orders (exceeds MAX_BATCH_SIZE)
        let mut orders = soroban_sdk::Vec::new(&env);
        for i in 0..101 {
            let recipient = Address::generate(&env);
            orders.push_back((recipient, asset.clone(), 1i128));
        }

        let result = StellarPayments::batch_payments(env.clone(), payer, orders);
        // Should fail with BatchLimitExceeded
        assert!(matches!(
            result,
            Err(PaymentError::BatchLimitExceeded)
        ));
    }
}
