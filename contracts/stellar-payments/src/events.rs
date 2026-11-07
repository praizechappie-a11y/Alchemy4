//! Event emission and versioning for the stellar-payments contract.

use soroban_sdk::{contracttype, Address, BytesN, Env, Symbol, Vec as SorobanVec};

/// Schema version for events (frozen at deployment).
pub const EVENT_SCHEMA_VERSION: u32 = 1;

/// Event nonce tracking (incremented per event type).
#[contracttype]
#[derive(Clone, Debug)]
pub struct EventNonce {
    pub payment_created: u64,
    pub payment_settled: u64,
    pub payment_refunded: u64,
    pub escrow_locked: u64,
    pub balance_updated: u64,
}

impl EventNonce {
    pub fn new() -> Self {
        Self {
            payment_created: 0,
            payment_settled: 0,
            payment_refunded: 0,
            escrow_locked: 0,
            balance_updated: 0,
        }
    }
}

/// Payment created event.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PaymentCreatedEvent {
    pub order_id: u32,
    pub payer: Address,
    pub recipient: Address,
    pub asset: Address,
    pub amount: i128,
    pub unlock_time: Option<u64>,
    pub created_at: u64,
    pub nonce: u64,
}

/// Payment settled event.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PaymentSettledEvent {
    pub order_id: u32,
    pub asset: Address,
    pub amount: i128,
    pub settled_at: u64,
    pub ledger_timestamp: u64,
    pub nonce: u64,
}

/// Payment refunded event.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PaymentRefundedEvent {
    pub order_id: u32,
    pub asset: Address,
    pub amount: i128,
    pub refunded_at: u64,
    pub reason: u32,
    pub nonce: u64,
}

/// Escrow locked event.
#[contracttype]
#[derive(Clone, Debug)]
pub struct EscrowLockedEvent {
    pub order_id: u32,
    pub unlock_time: u64,
    pub has_condition: bool,
    pub locked_at: u64,
    pub nonce: u64,
}

/// Balance updated event.
#[contracttype]
#[derive(Clone, Debug)]
pub struct BalanceUpdatedEvent {
    pub user: Address,
    pub asset: Address,
    pub new_balance: i128,
    pub updated_at: u64,
    pub nonce: u64,
}

/// Emit PaymentCreated event.
pub fn emit_payment_created(
    env: &Env,
    order_id: u32,
    payer: Address,
    recipient: Address,
    asset: Address,
    amount: i128,
    unlock_time: Option<u64>,
    nonce: u64,
) {
    let event = PaymentCreatedEvent {
        order_id,
        payer,
        recipient,
        asset,
        amount,
        unlock_time,
        created_at: env.ledger().timestamp(),
        nonce,
    };

    env.events().publish(
        (Symbol::new(env, "payment_created"),),
        event,
    );
}

/// Emit PaymentSettled event.
pub fn emit_payment_settled(
    env: &Env,
    order_id: u32,
    asset: Address,
    amount: i128,
    nonce: u64,
) {
    let event = PaymentSettledEvent {
        order_id,
        asset,
        amount,
        settled_at: env.ledger().timestamp(),
        ledger_timestamp: env.ledger().timestamp(),
        nonce,
    };

    env.events().publish(
        (Symbol::new(env, "payment_settled"),),
        event,
    );
}

/// Emit PaymentRefunded event.
pub fn emit_payment_refunded(
    env: &Env,
    order_id: u32,
    asset: Address,
    amount: i128,
    reason: u32,
    nonce: u64,
) {
    let event = PaymentRefundedEvent {
        order_id,
        asset,
        amount,
        refunded_at: env.ledger().timestamp(),
        reason,
        nonce,
    };

    env.events().publish(
        (Symbol::new(env, "payment_refunded"),),
        event,
    );
}

/// Emit EscrowLocked event.
pub fn emit_escrow_locked(
    env: &Env,
    order_id: u32,
    unlock_time: u64,
    has_condition: bool,
    nonce: u64,
) {
    let event = EscrowLockedEvent {
        order_id,
        unlock_time,
        has_condition,
        locked_at: env.ledger().timestamp(),
        nonce,
    };

    env.events().publish(
        (Symbol::new(env, "escrow_locked"),),
        event,
    );
}

/// Emit BalanceUpdated event.
pub fn emit_balance_updated(
    env: &Env,
    user: Address,
    asset: Address,
    new_balance: i128,
    nonce: u64,
) {
    let event = BalanceUpdatedEvent {
        user,
        asset,
        new_balance,
        updated_at: env.ledger().timestamp(),
        nonce,
    };

    env.events().publish(
        (Symbol::new(env, "balance_updated"),),
        event,
    );
}
