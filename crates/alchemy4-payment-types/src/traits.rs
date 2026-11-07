//! Payment gateway traits and interfaces.
//!
//! Defines trait abstractions for payment operations and compliance oracles,
//! allowing different payment implementations to be swapped.

use crate::errors::PaymentError;
use crate::types::{PaymentOrder, PaymentResult, PaymentStatus};
use soroban_sdk::{Address, Env, Symbol};

/// Payment gateway trait: core interface for payment operations.
///
/// Implementers (e.g., stellar-payments) provide the entry points that
/// other contracts call to perform deposits, withdrawals, and settlements.
pub trait PaymentGateway {
    /// Submit a payment order for settlement.
    ///
    /// Validates the order, deducts from payer, and increments recipient balance.
    fn submit_payment(env: &Env, order: PaymentOrder) -> Result<PaymentResult, PaymentError>;

    /// Claim a pending payment (for conditional/time-locked payments).
    ///
    /// Moves a payment from pending to settled if conditions are met.
    fn claim_payment(
        env: &Env,
        order_id: u32,
        recipient: Address,
    ) -> Result<PaymentResult, PaymentError>;

    /// Refund a pending payment back to the payer.
    ///
    /// Can only be called by payer or admin; sets status to Refunded.
    fn refund_payment(env: &Env, order_id: u32, reason: u32) -> Result<(), PaymentError>;

    /// Query payment status by order ID.
    fn get_payment_status(env: &Env, order_id: u32) -> Result<PaymentStatus, PaymentError>;
}

/// Compliance oracle trait: optional KYC/AML check interface.
///
/// Implementers provide compliance checks that can be plugged into
/// payment processing. The check runs before settlement.
pub trait ComplianceOracle {
    /// Check whether a payment is compliant.
    ///
    /// Returns Ok(()) if compliant, Err(ComplianceCheckFailed) if rejected.
    ///
    /// # Arguments
    ///
    /// * `env` - Soroban environment
    /// * `payer` - Payer address
    /// * `recipient` - Recipient address
    /// * `country_from` - Origin country code (2-char symbol)
    /// * `country_to` - Destination country code (2-char symbol)
    /// * `amount` - Payment amount
    /// * `asset` - Asset contract address
    fn check_payment(
        env: Env,
        payer: Address,
        recipient: Address,
        country_from: Symbol,
        country_to: Symbol,
        amount: i128,
        asset: Address,
    ) -> Result<(), PaymentError>;
}
