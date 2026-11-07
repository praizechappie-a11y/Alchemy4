//! Storage key definitions and helper functions for cross-border router.

use soroban_sdk::{Address, Env, Symbol, Vec as SorobanVec};
use hermes_payment_types::types::CorridorConfig;
use crate::errors::{Error, PaymentError};

/// Storage key helper for admin.
fn key_admin() -> Symbol {
    Symbol::short("admin")
}

/// Storage key helper for initialized flag.
fn key_init() -> Symbol {
    Symbol::short("init")
}

/// Storage key helper for payment counter.
fn key_counter() -> Symbol {
    Symbol::short("ctr")
}

/// Helper: get the next incremental payment ID.
pub fn get_next_payment_id(env: &Env) -> Result<u32, Error> {
    let key = key_counter();
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

/// Helper: get cached route (placeholder).
pub fn get_route(
    env: &Env,
    source: &Address,
    dest: &Address,
) -> Result<Option<SorobanVec<Address>>, Error> {
    Ok(None)
}

/// Helper: set cached route with TTL (placeholder).
pub fn set_route(
    env: &Env,
    source: &Address,
    dest: &Address,
    route: &SorobanVec<Address>,
) -> Result<(), Error> {
    Ok(())
}

/// Helper: check if route is expired (placeholder).
pub fn is_route_expired(env: &Env, source: &Address, dest: &Address) -> Result<bool, Error> {
    Ok(true)
}

/// Helper: get corridor configuration (placeholder).
pub fn get_corridor(
    env: &Env,
    country_from: &Symbol,
    country_to: &Symbol,
) -> Result<Option<CorridorConfig>, Error> {
    Ok(None)
}

/// Helper: set corridor configuration (placeholder).
pub fn set_corridor(
    env: &Env,
    country_from: &Symbol,
    country_to: &Symbol,
    config: &CorridorConfig,
) -> Result<(), Error> {
    Ok(())
}

/// Helper: check if country is sanctioned (placeholder).
pub fn is_sanctioned(env: &Env, country: &Symbol) -> Result<bool, Error> {
    Ok(false)
}

/// Helper: set country sanctions status (placeholder).
pub fn set_sanctioned(env: &Env, country: &Symbol, sanctioned: bool) -> Result<(), Error> {
    Ok(())
}
