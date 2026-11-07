//! Storage key definitions and helper functions for the stellar-payments contract.

use soroban_sdk::{contracttype, Address, Env, Symbol};
use hermes_payment_types::types::{PaymentOrder, FeeConfig};
use crate::errors::{Error, PaymentError};

/// Storage key enum for all persistent data.
#[contracttype]
#[derive(Clone, Debug)]
pub enum DataKey {
    Admin,
    Initialized,
    PaymentCounter,
    FeeConfig,
    UserBalance(Address, Address),      // (user, asset) -> balance
    FeeBalance(Address),                // asset -> fee_balance
    Payment(u32),                       // order_id -> PaymentOrder
    ComplianceRegistry(Address),        // compliance_contract -> bool
}

/// Helper: get storage key for admin.
pub fn key_admin() -> DataKey {
    DataKey::Admin
}

/// Helper: get storage key for initialized flag.
pub fn key_initialized() -> DataKey {
    DataKey::Initialized
}

/// Helper: get storage key for payment counter.
pub fn key_payment_counter() -> DataKey {
    DataKey::PaymentCounter
}

/// Helper: get storage key for fee config.
pub fn key_fee_config() -> DataKey {
    DataKey::FeeConfig
}

/// Helper: get the next incremental payment ID (increments counter).
pub fn get_next_payment_id(env: &Env) -> Result<u32, Error> {
    let key = key_payment_counter();
    let current: u32 = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Ok(0u32))
        .unwrap_or(0);

    let next = current
        .checked_add(1)
        .ok_or(PaymentError::StorageFull)?;

    env.storage().persistent().set(&key, &next);

    Ok(current)
}

/// Helper: get user balance for a specific asset.
pub fn get_user_balance(env: &Env, user: &Address, asset: &Address) -> Result<i128, Error> {
    let key = DataKey::UserBalance(user.clone(), asset.clone());
    Ok(env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Ok(0i128))
        .unwrap_or(0))
}

/// Helper: set user balance for a specific asset.
pub fn set_user_balance(
    env: &Env,
    user: &Address,
    asset: &Address,
    balance: i128,
) -> Result<(), Error> {
    let key = DataKey::UserBalance(user.clone(), asset.clone());
    env.storage().persistent().set(&key, &balance);
    Ok(())
}

/// Helper: get fee configuration (or default if not set).
pub fn get_fee_config(env: &Env) -> Result<FeeConfig, Error> {
    let key = key_fee_config();
    Ok(env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|_| {
            Ok(FeeConfig {
                platform_fee_bps: 0,
                min_fee_stroops: 0,
                fee_recipient: Address::from_contract_id(&env.current_contract_address()),
            })
        })
        .unwrap_or_else(|_| FeeConfig {
            platform_fee_bps: 0,
            min_fee_stroops: 0,
            fee_recipient: Address::from_contract_id(&env.current_contract_address()),
        }))
}

/// Helper: set fee configuration.
pub fn set_fee_config(env: &Env, config: &FeeConfig) -> Result<(), Error> {
    let key = key_fee_config();
    env.storage().persistent().set(&key, config);
    Ok(())
}

/// Helper: get fee balance for a specific asset.
pub fn get_fee_balance(env: &Env, asset: &Address) -> Result<i128, Error> {
    let key = DataKey::FeeBalance(asset.clone());
    Ok(env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Ok(0i128))
        .unwrap_or(0))
}

/// Helper: set fee balance for a specific asset.
pub fn set_fee_balance(env: &Env, asset: &Address, balance: i128) -> Result<(), Error> {
    let key = DataKey::FeeBalance(asset.clone());
    env.storage().persistent().set(&key, &balance);
    Ok(())
}

/// Helper: get stored payment order.
pub fn get_payment(env: &Env, id: u32) -> Result<Option<PaymentOrder>, Error> {
    let key = DataKey::Payment(id);
    Ok(env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Ok(None))
        .unwrap_or(None))
}

/// Helper: set stored payment order.
pub fn set_payment(env: &Env, id: u32, order: &PaymentOrder) -> Result<(), Error> {
    let key = DataKey::Payment(id);
    env.storage().persistent().set(&key, order);
    Ok(())
}

/// Helper: check if a compliance check contract is registered.
pub fn is_compliance_registered(env: &Env, compliance_contract: &Address) -> Result<bool, Error> {
    let key = DataKey::ComplianceRegistry(compliance_contract.clone());
    Ok(env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Ok(false))
        .unwrap_or(false))
}

/// Helper: register a compliance check contract.
pub fn register_compliance(env: &Env, compliance_contract: &Address) -> Result<(), Error> {
    let key = DataKey::ComplianceRegistry(compliance_contract.clone());
    env.storage().persistent().set(&key, &true);
    Ok(())
}
