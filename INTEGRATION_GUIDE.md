# Alchemy4 - Integration Guide

This guide shows how to integrate Alchemy4 payment infrastructure into smart contracts.

---

## Overview

The `alchemy4-payment-types` crate provides canonical types and traits that all payment-aware contracts share. By importing these types, your contract can:

1. Accept payments in multiple Stellar assets (XLM, USDC, custom tokens)
2. Validate payment amounts before processing
3. Query payment status
4. Handle escrow and conditional payments
5. Implement batched settlement

---

## Quick Integration: 5 Steps

### Step 1: Add Dependency

Update your contract's `Cargo.toml`:

```toml
[dependencies]
soroban-sdk = "22.0.0"
alchemy4-payment-types = { path = "../../crates/alchemy4-payment-types" }
```

### Step 2: Import Types

In your contract's `lib.rs`:

```rust
use alchemy4_payment_types::{
    PaymentOrder, PaymentStatus, PaymentError, PaymentResult,
    FeeConfig, PaymentGateway,
};
```

### Step 3: Accept Payment Orders

Modify your entrypoint to accept a `PaymentOrder`:

```rust
#[contract]
pub struct BettingContract;

#[contractimpl]
impl BettingContract {
    pub fn place_bet_with_payment(
        env: &Env,
        payer: Address,
        bet_amount: i128,
        asset: Address,
        prediction: String,
    ) -> Result<u32, PaymentError> {
        payer.require_auth();

        // Create payment order
        let order = PaymentOrder {
            id: 0,  // Will be assigned by stellar-payments
            payer: payer.clone(),
            recipient: env.current_contract_address(),
            amount: bet_amount,
            asset_contract: asset,
            created_at: env.ledger().timestamp(),
            expected_settlement: env.ledger().timestamp() + 3600,
            status: PaymentStatus::Pending,
            escrow_unlock_time: None,
            condition_hash: None,
        };

        // Forward to stellar-payments contract for settlement
        // This pattern assumes stellar-payments is accessible
        // In practice, you'd call it via contract invocation or
        // handle payment internally

        // For now, store bet with pending payment status
        let bet_id = generate_bet_id(env);
        store_bet(env, bet_id, payer, prediction, bet_amount, asset);

        Ok(bet_id)
    }
}

fn generate_bet_id(env: &Env) -> u32 {
    // Implementation: generate unique bet ID
    env.storage().persistent().get(&DataKey::BetCounter).unwrap_or(0) + 1
}

fn store_bet(
    env: &Env,
    bet_id: u32,
    payer: Address,
    prediction: String,
    amount: i128,
    asset: Address,
) {
    // Implementation: store bet data
}
```

### Step 4: Query Payment Status

Check if payment has settled before processing bet:

```rust
pub fn settle_bet(
    env: &Env,
    bet_id: u32,
    outcome: String,
) -> Result<String, PaymentError> {
    // In production, query stellar-payments contract
    // For now, local storage model:

    let payment_status = get_payment_status(env, bet_id)?;

    match payment_status {
        PaymentStatus::Settled => {
            // Process bet outcome
            process_bet_outcome(env, bet_id, outcome)
        }
        PaymentStatus::Pending => {
            Err(PaymentError::PaymentNotFound)
        }
        PaymentStatus::Refunded => {
            Err(PaymentError::PaymentAlreadySettled)
        }
        _ => Err(PaymentError::PaymentNotFound),
    }
}

fn get_payment_status(env: &Env, bet_id: u32) -> Result<PaymentStatus, PaymentError> {
    // Implementation: query from storage
    Ok(PaymentStatus::Settled)
}

fn process_bet_outcome(env: &Env, bet_id: u32, outcome: String) -> Result<String, PaymentError> {
    // Implementation: update bet with outcome
    Ok(format!("Bet {} settled with outcome: {}", bet_id, outcome))
}
```

### Step 5: Handle Payment Errors

Gracefully handle payment-related errors:

```rust
pub fn attempt_payment(
    env: &Env,
    payer: Address,
    amount: i128,
    asset: Address,
) -> Result<String, String> {
    payer.require_auth();

    let order = PaymentOrder {
        id: 0,
        payer: payer.clone(),
        recipient: env.current_contract_address(),
        amount,
        asset_contract: asset,
        created_at: env.ledger().timestamp(),
        expected_settlement: env.ledger().timestamp() + 3600,
        status: PaymentStatus::Pending,
        escrow_unlock_time: None,
        condition_hash: None,
    };

    // Try to process payment
    match submit_payment(env, order) {
        Ok(result) => {
            if result.success {
                Ok(format!("Payment {} processed successfully", result.order_id))
            } else {
                Err(format!(
                    "Payment failed with code: {:?}",
                    result.error_code
                ))
            }
        }
        Err(PaymentError::InsufficientBalance) => {
            Err("Insufficient balance for payment".to_string())
        }
        Err(PaymentError::InvalidAsset) => {
            Err("Invalid asset for payment".to_string())
        }
        Err(PaymentError::ComplianceCheckFailed) => {
            Err("Payment blocked by compliance check".to_string())
        }
        Err(e) => {
            Err(format!("Payment error: {:?}", e))
        }
    }
}

fn submit_payment(env: &Env, order: PaymentOrder) -> Result<PaymentResult, PaymentError> {
    // Implementation: call stellar-payments contract
    // For now, placeholder
    Ok(PaymentResult {
        order_id: 0,
        success: true,
        ledger_timestamp: env.ledger().timestamp(),
        error_code: None,
    })
}
```

---

## Pattern 1: Immediate Payments (Betting)

For instant settlement (e.g., sports betting):

```rust
pub fn place_instant_bet(
    env: &Env,
    payer: Address,
    amount: i128,
    asset: Address,
    team: String,
) -> Result<u32, PaymentError> {
    payer.require_auth();

    // Create immediate payment (no escrow)
    let order = PaymentOrder {
        id: 0,
        payer: payer.clone(),
        recipient: env.current_contract_address(),
        amount,
        asset_contract: asset,
        created_at: env.ledger().timestamp(),
        expected_settlement: env.ledger().timestamp(), // Immediate
        status: PaymentStatus::Pending,
        escrow_unlock_time: None,    // No time-lock
        condition_hash: None,        // No HTLC condition
    };

    // Settlement happens immediately upon submitting
    match submit_payment(env, order) {
        Ok(result) if result.success => {
            let bet_id = store_bet(env, payer, team, amount, asset);
            Ok(bet_id)
        }
        Ok(result) => Err(PaymentError::PaymentNotFound),
        Err(e) => Err(e),
    }
}
```

---

## Pattern 2: Escrow Payments (Markets)

For conditional settlement (e.g., prediction markets):

```rust
use alchemy4_payment_types::BytesN;

pub fn create_market_with_escrow(
    env: &Env,
    creator: Address,
    deposit_amount: i128,
    asset: Address,
    resolution_time: u64,
) -> Result<u32, PaymentError> {
    creator.require_auth();

    // Create escrow payment (released after resolution_time)
    let order = PaymentOrder {
        id: 0,
        payer: creator.clone(),
        recipient: env.current_contract_address(),
        amount: deposit_amount,
        asset_contract: asset,
        created_at: env.ledger().timestamp(),
        expected_settlement: resolution_time,
        status: PaymentStatus::Pending,
        escrow_unlock_time: Some(resolution_time), // Time-locked!
        condition_hash: None,                        // No HTLC
    };

    match submit_payment(env, order) {
        Ok(result) if result.success => {
            let market_id = create_market(env, creator, deposit_amount);
            Ok(market_id)
        }
        Ok(_) => Err(PaymentError::PaymentNotFound),
        Err(e) => Err(e),
    }
}

fn create_market(env: &Env, creator: Address, amount: i128) -> u32 {
    // Implementation
    1
}
```

---

## Pattern 3: HTLC Payments (Disputes)

For conditional release (e.g., dispute resolution):

```rust
pub fn escrow_dispute_settlement(
    env: &Env,
    payer: Address,
    recipient: Address,
    amount: i128,
    asset: Address,
    preimage: BytesN<32>,
) -> Result<u32, PaymentError> {
    payer.require_auth();

    // Hash the preimage (assumed to be SHA256 by caller)
    // In real code, you'd hash it here or accept pre-hashed
    let condition_hash = preimage;

    let order = PaymentOrder {
        id: 0,
        payer: payer.clone(),
        recipient,
        amount,
        asset_contract: asset,
        created_at: env.ledger().timestamp(),
        expected_settlement: env.ledger().timestamp() + 86400 * 7, // 1 week
        status: PaymentStatus::Pending,
        escrow_unlock_time: Some(env.ledger().timestamp() + 86400 * 7),
        condition_hash: Some(condition_hash),  // HTLC condition!
    };

    match submit_payment(env, order) {
        Ok(result) if result.success => {
            let settlement_id = store_settlement(env, result.order_id);
            Ok(settlement_id)
        }
        Ok(_) => Err(PaymentError::PaymentNotFound),
        Err(e) => Err(e),
    }
}

fn store_settlement(env: &Env, order_id: u32) -> u32 {
    order_id
}
```

---

## Pattern 4: Batch Payments (Payouts)

For multiple recipients in one transaction (e.g., dividend payouts):

```rust
pub fn distribute_winnings_batch(
    env: &Env,
    admin: Address,
    winners: Vec<(Address, i128)>, // (winner_address, amount)
    asset: Address,
) -> Result<Vec<u32>, PaymentError> {
    admin.require_auth();

    // Create payment orders for each winner
    let mut orders = Vec::new();
    for (winner, amount) in winners.iter() {
        let order = PaymentOrder {
            id: 0,
            payer: admin.clone(),
            recipient: winner.clone(),
            amount: *amount,
            asset_contract: asset.clone(),
            created_at: env.ledger().timestamp(),
            expected_settlement: env.ledger().timestamp(),
            status: PaymentStatus::Pending,
            escrow_unlock_time: None,
            condition_hash: None,
        };
        orders.push(order);
    }

    // Submit batch (up to 100 orders)
    if orders.len() > 100 {
        return Err(PaymentError::BatchLimitExceeded);
    }

    // Process batch
    let mut order_ids = Vec::new();
    for order in orders {
        match submit_payment(env, order) {
            Ok(result) if result.success => {
                order_ids.push(result.order_id);
            }
            Ok(_) => return Err(PaymentError::PaymentNotFound),
            Err(e) => return Err(e),
        }
    }

    Ok(order_ids)
}
```

---

## Pattern 5: Multi-Asset Support

Accept multiple Stellar assets in one contract:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct AssetConfig {
    pub contract: Address,
    pub accepted: bool,
    pub min_amount: i128,
    pub max_amount: i128,
}

pub fn register_accepted_asset(
    env: &Env,
    admin: Address,
    asset: Address,
    min_amount: i128,
    max_amount: i128,
) -> Result<(), String> {
    admin.require_auth();

    let config = AssetConfig {
        contract: asset,
        accepted: true,
        min_amount,
        max_amount,
    };

    // Store asset config
    store_asset_config(env, asset, config);

    Ok(())
}

pub fn accept_payment_multi_asset(
    env: &Env,
    payer: Address,
    amount: i128,
    asset: Address,
) -> Result<u32, String> {
    payer.require_auth();

    // Validate asset is accepted
    let config = load_asset_config(env, &asset)
        .ok_or("Asset not accepted")?;

    if !config.accepted {
        return Err("Asset not accepted".to_string());
    }

    if amount < config.min_amount || amount > config.max_amount {
        return Err(format!(
            "Amount must be between {} and {}",
            config.min_amount, config.max_amount
        ));
    }

    // Create payment with validated asset
    let order = PaymentOrder {
        id: 0,
        payer: payer.clone(),
        recipient: env.current_contract_address(),
        amount,
        asset_contract: asset,
        created_at: env.ledger().timestamp(),
        expected_settlement: env.ledger().timestamp(),
        status: PaymentStatus::Pending,
        escrow_unlock_time: None,
        condition_hash: None,
    };

    match submit_payment(env, order) {
        Ok(result) if result.success => Ok(result.order_id),
        _ => Err("Payment failed".to_string()),
    }
}

fn store_asset_config(env: &Env, asset: Address, config: AssetConfig) {
    // Implementation
}

fn load_asset_config(env: &Env, asset: &Address) -> Option<AssetConfig> {
    // Implementation
    None
}
```

---

## Pattern 6: Cross-Border Payments

Use cross-border router for FX conversion:

```rust
pub fn send_cross_border_payment(
    env: &Env,
    sender: Address,
    recipient_country: String,
    amount_in_source: i128,
    source_asset: Address,
    dest_asset: Address,
) -> Result<u32, PaymentError> {
    sender.require_auth();

    // Query exchange rate from cross-border-router
    let exchange_rate = get_exchange_rate(env, source_asset, dest_asset)?;

    // Calculate destination amount
    let dest_amount = (amount_in_source * exchange_rate) / 100_000_000;

    // Validate recipient country is in allowed corridors
    validate_corridor(env, "US", &recipient_country)?;

    // Create cross-border payment
    let order = PaymentOrder {
        id: 0,
        payer: sender.clone(),
        recipient: Address::from_contract_id(&env, &cross_border_router_id()),
        amount: amount_in_source,
        asset_contract: source_asset,
        created_at: env.ledger().timestamp(),
        expected_settlement: env.ledger().timestamp() + 600,
        status: PaymentStatus::Pending,
        escrow_unlock_time: None,
        condition_hash: None,
    };

    // Route through cross-border-router
    match route_payment(env, order, dest_asset) {
        Ok(result) if result.success => Ok(result.order_id),
        Ok(_) => Err(PaymentError::RoutingFailed),
        Err(e) => Err(e),
    }
}

fn get_exchange_rate(
    env: &Env,
    source: Address,
    dest: Address,
) -> Result<i128, PaymentError> {
    // Query from cross-border-router contract
    // Placeholder: return 1:1
    Ok(100_000_000)
}

fn validate_corridor(env: &Env, source: &str, dest: &str) -> Result<(), PaymentError> {
    // Validate country pair is allowed
    Ok(())
}

fn cross_border_router_id() -> soroban_sdk::BytesN<32> {
    // Return router contract ID
    Default::default()
}

fn route_payment(
    env: &Env,
    order: PaymentOrder,
    dest_asset: Address,
) -> Result<PaymentResult, PaymentError> {
    // Call router contract
    Ok(PaymentResult {
        order_id: 0,
        success: true,
        ledger_timestamp: env.ledger().timestamp(),
        error_code: None,
    })
}
```

---

## Error Handling

All payment operations return `Result<T, PaymentError>`. Handle errors:

```rust
pub enum PaymentError {
    Unauthorized,              // 505
    PaymentNotFound,           // 506
    InvalidAmount,             // 507
    InvalidAsset,              // 508
    InsufficientBalance,       // 509
    ConditionNotMet,           // 510
    PaymentLocked,             // 511
    PaymentAlreadySettled,     // 512
    AssetTransferFailed,       // 513
    RoutingFailed,             // 514
    ComplianceCheckFailed,     // 515
    BatchLimitExceeded,        // 516
    StorageFull,               // 517
}
```

Map to user-friendly errors:

```rust
fn user_friendly_error(err: PaymentError) -> String {
    match err {
        PaymentError::InsufficientBalance => {
            "You don't have enough balance. Please deposit more.".to_string()
        }
        PaymentError::InvalidAsset => {
            "This asset is not supported.".to_string()
        }
        PaymentError::ConditionNotMet => {
            "Payment condition (HTLC) not satisfied.".to_string()
        }
        PaymentError::PaymentLocked => {
            "Payment is time-locked and cannot be claimed yet.".to_string()
        }
        PaymentError::ComplianceCheckFailed => {
            "Payment blocked by compliance check (KYC/AML).".to_string()
        }
        PaymentError::RoutingFailed => {
            "Cross-border routing path not available.".to_string()
        }
        _ => format!("Payment error: {:?}", err),
    }
}
```

---

## Testing Payments

Write tests that include payment flows:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_place_bet_with_payment() {
        let env = Env::default();
        let contract = BettingContract;

        let payer = Address::generate(&env);
        let asset = Address::generate(&env);

        let result = contract.place_bet_with_payment(
            &env,
            payer,
            1000000, // 1 XLM in stroops
            asset,
            "Home Team Wins".to_string(),
        );

        assert!(result.is_ok());
        let bet_id = result.unwrap();
        assert_eq!(bet_id, 1);
    }

    #[test]
    fn test_insufficient_balance_error() {
        let env = Env::default();
        let contract = BettingContract;

        let payer = Address::generate(&env);
        let asset = Address::generate(&env);

        let result = contract.place_bet_with_payment(
            &env,
            payer,
            9_999_999_999_999_999, // Excessive amount
            asset,
            "Team A".to_string(),
        );

        // Should fail due to insufficient balance
        assert!(result.is_err());
    }
}
```

---

## Migration Checklist

When migrating an existing contract to use payments:

- [ ] Add `alchemy4-payment-types` dependency to `Cargo.toml`
- [ ] Import payment types: `use alchemy4_payment_types::*`
- [ ] Identify which operations require payments
- [ ] Create `PaymentOrder` structs for those operations
- [ ] Add error handling for `PaymentError`
- [ ] Update entrypoint signatures to accept payment params
- [ ] Write integration tests with payment flows
- [ ] Test on Stellar Testnet with deployed stellar-payments contract
- [ ] Review with security team
- [ ] Deploy to mainnet after testing

---

## Best Practices

1. **Always authorize payer:** `payer.require_auth()`
2. **Validate asset before payment:** Check asset contract is trusted
3. **Handle partial batch failure:** Payment N may fail while 1..N-1 succeed
4. **Set realistic time-locks:** Don't over-lock escrow (users get frustrated)
5. **Document fee structure:** Users need to understand fee deductions
6. **Log payment events:** Emit events for off-chain indexing
7. **Test on testnet first:** Deploy to Stellar Testnet before mainnet
8. **Plan compliance:** If operating in regulated markets, implement compliance hooks

---

## References

- **Type Definitions:** `crates/alchemy4-payment-types/src/types.rs`
- **Error Codes:** `crates/alchemy4-payment-types/src/errors.rs`
- **Traits:** `crates/alchemy4-payment-types/src/traits.rs`
- **Design Document:** `.agents/tasks/payment-design.md`
- **API Reference:** `docs/API_REFERENCE.md`
- **Deployment Guide:** `DEPLOYMENT.md`

