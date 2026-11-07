//! Core payment types and data structures.
//!
//! Defines the canonical types used across all payment contracts:
//! - PaymentStatus: lifecycle state machine
//! - PaymentOrder: individual payment record
//! - PaymentResult: result of a payment operation
//! - FeeConfig: fee configuration parameters
//! - CorridorConfig: cross-border corridor constraints
//! - CrossBorderPayment: cross-border payment record

use soroban_sdk::{contracttype, Address, BytesN, Symbol};

/// Payment status lifecycle states.
///
/// A payment progresses through these states:
/// - Pending: Created but not yet settled
/// - Settled: Transfer complete and confirmed
/// - Refunded: Returned to payer (terminal)
/// - Disputed: Under review (terminal)
/// - Failed: Terminal failure (terminal)
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PaymentStatus {
    /// Created, awaiting settlement (initial state)
    Pending = 0,
    /// Transferred and confirmed (terminal)
    Settled = 1,
    /// Returned to payer (terminal)
    Refunded = 2,
    /// Under review (terminal)
    Disputed = 3,
    /// Terminal failure (terminal)
    Failed = 4,
}

/// A single payment order.
///
/// Represents a payment transaction with all metadata needed to track
/// its lifecycle from creation through settlement or refund.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PaymentOrder {
    /// Unique payment order ID (incremented per contract)
    pub id: u32,
    /// Payer's address (initiator of payment)
    pub payer: Address,
    /// Recipient's address (destination)
    pub recipient: Address,
    /// Amount in stroops (for XLM) or SAC base units
    pub amount: i128,
    /// Stellar asset contract address
    pub asset_contract: Address,
    /// Ledger timestamp when created (seconds)
    pub created_at: u64,
    /// Advisory settlement time (informational)
    pub expected_settlement: u64,
    /// Current lifecycle state
    pub status: PaymentStatus,
    /// If set, payment locked until this timestamp (escrow)
    pub escrow_unlock_time: Option<u64>,
    /// If set, payment requires preimage matching this hash (HTLC)
    pub condition_hash: Option<BytesN<32>>,
}

/// Result of a payment operation.
///
/// Returned by settlement functions to confirm success or failure.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PaymentResult {
    /// The order ID being settled
    pub order_id: u32,
    /// Whether the operation succeeded
    pub success: bool,
    /// Ledger timestamp when result was produced
    pub ledger_timestamp: u64,
    /// Error code if success=false (None if success=true)
    pub error_code: Option<u32>,
}

/// Fee configuration for the payment system.
///
/// Fees are deducted from the payer at settlement time.
/// Formula: fee = max(ceil(amount * platform_fee_bps / 10000), min_fee_stroops)
#[contracttype]
#[derive(Clone, Debug)]
pub struct FeeConfig {
    /// Platform fee in basis points (e.g., 50 = 0.5%)
    pub platform_fee_bps: u32,
    /// Minimum per-transaction fee in stroops/base units
    pub min_fee_stroops: i128,
    /// Address where collected fees are sent
    pub fee_recipient: Address,
}

/// Cross-border payment corridor configuration.
///
/// Defines constraints and limits for payments between country pairs.
#[contracttype]
#[derive(Clone, Debug)]
pub struct CorridorConfig {
    /// Maximum daily volume for this corridor (in base units)
    pub max_daily_volume: i128,
    /// Current daily volume (reset daily, UTC)
    pub current_daily_volume: i128,
    /// Maximum per-payment amount
    pub max_per_payment: i128,
    /// Maximum acceptable slippage in basis points (e.g., 100 = 1%)
    pub slippage_tolerance_bps: u32,
    /// Whether this corridor is enabled
    pub enabled: bool,
}

/// Cross-border payment record.
///
/// Tracks a payment that involves currency conversion across borders.
#[contracttype]
#[derive(Clone, Debug)]
pub struct CrossBorderPayment {
    /// Unique cross-border payment ID
    pub id: u32,
    /// Payer address
    pub payer: Address,
    /// Recipient address
    pub recipient: Address,
    /// Source asset (e.g., USDC on issuer-A)
    pub source_asset: Address,
    /// Destination asset (e.g., USDC on issuer-B)
    pub destination_asset: Address,
    /// Amount in source asset
    pub source_amount: i128,
    /// Minimum acceptable amount in destination asset (slippage limit)
    pub destination_amount_min: i128,
    /// Origin country code (2-char symbol)
    pub country_from: Symbol,
    /// Destination country code (2-char symbol)
    pub country_to: Symbol,
    /// Current lifecycle state
    pub status: PaymentStatus,
    /// Route taken (sequence of pool addresses)
    pub route_taken: soroban_sdk::Vec<Address>,
    /// Effective exchange rate applied (scaled to i128)
    pub effective_rate: i128,
    /// Timestamp when payment was created
    pub created_at: u64,
}
