//! Payment system error definitions and code mappings.
//!
//! Implements the PaymentError enum with error codes 505–599.
//! All variants map to specific error codes via the #[contracterror] macro.

use soroban_sdk::contracterror;

/// Payment system error codes (500–599).
/// 
/// All variants have corresponding canonical definitions in hermes-errors.
#[contracterror]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PaymentError {
    /// Unauthorized: Caller not authorized (code 505)
    #[error(code = 505, name = "Unauthorized")]
    Unauthorized = 505,

    /// PaymentNotFound: Order ID doesn't exist (code 506)
    #[error(code = 506, name = "PaymentNotFound")]
    PaymentNotFound = 506,

    /// InvalidAmount: Amount <= 0 or exceeds limits (code 507)
    #[error(code = 507, name = "InvalidAmount")]
    InvalidAmount = 507,

    /// InvalidAsset: Asset contract address invalid or not a token (code 508)
    #[error(code = 508, name = "InvalidAsset")]
    InvalidAsset = 508,

    /// InsufficientBalance: Payer has no balance (code 509)
    #[error(code = 509, name = "InsufficientBalance")]
    InsufficientBalance = 509,

    /// ConditionNotMet: HTLC preimage invalid or not provided (code 510)
    #[error(code = 510, name = "ConditionNotMet")]
    ConditionNotMet = 510,

    /// PaymentLocked: Escrow time not yet reached (code 511)
    #[error(code = 511, name = "PaymentLocked")]
    PaymentLocked = 511,

    /// PaymentAlreadySettled: Can't modify a settled/refunded payment (code 512)
    #[error(code = 512, name = "PaymentAlreadySettled")]
    PaymentAlreadySettled = 512,

    /// AssetTransferFailed: Token contract returned error (code 513)
    #[error(code = 513, name = "AssetTransferFailed")]
    AssetTransferFailed = 513,

    /// RoutingFailed: Cross-border path not found or expired (code 514)
    #[error(code = 514, name = "RoutingFailed")]
    RoutingFailed = 514,

    /// ComplianceCheckFailed: KYC/AML hook rejected (code 515)
    #[error(code = 515, name = "ComplianceCheckFailed")]
    ComplianceCheckFailed = 515,

    /// BatchLimitExceeded: Too many orders in batch (code 516)
    #[error(code = 516, name = "BatchLimitExceeded")]
    BatchLimitExceeded = 516,

    /// StorageFull: Contract storage exhausted (code 517)
    #[error(code = 517, name = "StorageFull")]
    StorageFull = 517,
}

impl From<PaymentError> for u32 {
    fn from(err: PaymentError) -> u32 {
        err as u32
    }
}
