//! Event emission for the cross-border-router contract.

use soroban_sdk::{contracttype, Address, Env, Symbol, Vec as SorobanVec};

/// Route cached event.
#[contracttype]
#[derive(Clone, Debug)]
pub struct RouteCachedEvent {
    pub source_asset: Address,
    pub dest_asset: Address,
    pub hop_count: u32,
    pub cached_at: u64,
    pub nonce: u64,
}

/// Payment routed event.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PaymentRoutedEvent {
    pub order_id: u32,
    pub source_amount: i128,
    pub destination_amount: i128,
    pub hops: u32,
    pub effective_rate: i128,
    pub routed_at: u64,
    pub nonce: u64,
}

/// Routing failed event.
#[contracttype]
#[derive(Clone, Debug)]
pub struct RoutingFailedEvent {
    pub source_asset: Address,
    pub dest_asset: Address,
    pub reason: u32,
    pub attempted_at: u64,
    pub nonce: u64,
}

/// Emit RouteCached event.
pub fn emit_route_cached(
    env: &Env,
    source_asset: Address,
    dest_asset: Address,
    hop_count: u32,
    nonce: u64,
) {
    let event = RouteCachedEvent {
        source_asset,
        dest_asset,
        hop_count,
        cached_at: env.ledger().timestamp(),
        nonce,
    };

    env.events().publish(
        (Symbol::new(env, "route_cached"),),
        event,
    );
}

/// Emit PaymentRouted event.
pub fn emit_payment_routed(
    env: &Env,
    order_id: u32,
    source_amount: i128,
    destination_amount: i128,
    hops: u32,
    effective_rate: i128,
    nonce: u64,
) {
    let event = PaymentRoutedEvent {
        order_id,
        source_amount,
        destination_amount,
        hops,
        effective_rate,
        routed_at: env.ledger().timestamp(),
        nonce,
    };

    env.events().publish(
        (Symbol::new(env, "payment_routed"),),
        event,
    );
}

/// Emit RoutingFailed event.
pub fn emit_routing_failed(
    env: &Env,
    source_asset: Address,
    dest_asset: Address,
    reason: u32,
    nonce: u64,
) {
    let event = RoutingFailedEvent {
        source_asset,
        dest_asset,
        reason,
        attempted_at: env.ledger().timestamp(),
        nonce,
    };

    env.events().publish(
        (Symbol::new(env, "routing_failed"),),
        event,
    );
}
