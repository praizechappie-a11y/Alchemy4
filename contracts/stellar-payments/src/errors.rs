//! Local error re-exports from hermes-payment-types.
//!
//! Re-exports PaymentError as the contract's error type.

pub use hermes_payment_types::PaymentError;

/// Type alias for error handling in this contract.
pub type Error = PaymentError;
