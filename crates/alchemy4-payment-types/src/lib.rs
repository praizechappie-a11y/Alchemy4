//! # Hermes Payment Types
//!
//! Canonical payment types, traits, and error codes shared across all Hermes
//! contracts that integrate payment functionality.
//!
//! ## Overview
//!
//! This crate provides:
//!
//! - **Error Definitions** — PaymentError enum with codes 505–599
//! - **Core Types** — PaymentOrder, PaymentResult, PaymentStatus, FeeConfig, CorridorConfig
//! - **Trait Abstractions** — PaymentGateway and ComplianceOracle traits
//!
//! ## Design Principles
//!
//! - **Modular** — Contracts implementing PaymentGateway can be swapped without breaking callers
//! - **Event-driven** — All state changes emit versioned events for off-chain indexing
//! - **Backward compatible** — Existing contracts remain unchanged; new features are opt-in
//! - **Compliance-ready** — KYC/AML checks are pluggable, not mandatory
//!
//! ## Usage
//!
//! Import types and traits:
//!
//! ```ignore
//! use hermes_payment_types::{
//!     errors::PaymentError,
//!     types::{PaymentOrder, PaymentStatus, FeeConfig},
//!     traits::PaymentGateway,
//! };
//! ```
//!
//! Implement the PaymentGateway trait in a contract to provide payment operations.

#![no_std]

pub mod errors;
pub mod traits;
pub mod types;

pub use errors::PaymentError;
pub use traits::{ComplianceOracle, PaymentGateway};
pub use types::{
    CorridorConfig, CrossBorderPayment, FeeConfig, PaymentOrder, PaymentResult, PaymentStatus,
};
