# Stellar Payment Integration Guide

**Version:** 1.0  
**Date:** January 2025  
**Status:** Implementation Ready

---

## Overview

The Alchemy4 platform now includes comprehensive payment support via Stellar blockchain integration. This enables:

- **Native XLM and Stellar Asset Payments** — Direct transfers with sub-second finality
- **Escrow & Conditional Payments** — Time-locked and HTLC-style conditional transfers
- **Batch Payments** — Up to 100 recipients in a single transaction
- **Cross-Border Routing** — Automatic FX conversion via Stellar DEX
- **Compliance Integration** — Optional KYC/AML hooks for regulated payments

---

## Architecture

### Core Components

#### 1. hermes-payment-types (Shared Library)
Canonical payment types, error codes, and trait interfaces used across all payment contracts.

**Key Types:**
- `PaymentStatus` — Lifecycle states (Pending, Settled, Refunded, Disputed, Failed)
- `PaymentOrder` — Complete payment record with metadata
- `PaymentResult` — Result of a payment operation
- `FeeConfig` — Fee configuration (basis points + minimum)
- `CorridorConfig` — Cross-border payment limits (countries, volumes)

**Error Codes (505–599):**
- 505: Unauthorized
- 506: PaymentNotFound
- 507: InvalidAmount
- 508: InvalidAsset
- 509: InsufficientBalance
- 510: ConditionNotMet (HTLC preimage mismatch)
- 511: PaymentLocked (escrow time not reached)
- 512: PaymentAlreadySettled
- 513: AssetTransferFailed
- 514: RoutingFailed (no cross-border route)
- 515: ComplianceCheckFailed
- 516: BatchLimitExceeded (>100 orders)
- 517: StorageFull

#### 2. stellar-payments Contract
Core payment engine handling deposits, withdrawals, escrow, and settlements.

**Key Entrypoints:**
```rust
// Admin functions
initialize(admin, fee_config?)
update_fee_config(admin, new_config)
register_compliance_check(admin, compliance_contract)

// User functions
deposit(user, asset, amount)                    // Receive tokens
withdraw(user, asset, amount)                   // Release tokens
immediate_payment(payer, recipient, ...)        // Direct transfer
create_escrow_payment(payer, recipient, ...)    // Time-locked
settle_escrow(order_id, preimage?)              // Unlock after time
batch_payments(payer, orders)                   // Up to 100 recipients

// Query functions
get_balance(user, asset)
get_payment_status(order_id)
estimate_fee(amount)
get_fee_config()
withdraw_fees(admin, asset)
```

#### 3. cross-border-router Contract
Routes cross-border payments through Stellar DEX with compliance checks.

**Key Entrypoints:**
```rust
// Route management
submit_route(source_asset, dest_asset, route)   // Cache conversion path
find_best_route(source_asset, dest_asset)       // Query cached route
get_exchange_rate(source_asset, dest_asset)     // Calculate FX rate

// Corridor management
set_corridor_config(country_from, country_to, config)
get_corridor_config(country_from, country_to)

// Compliance
set_country_sanctions(country, sanctioned)
is_country_sanctioned(country)

// Routing
route_payment(payer, recipient, source_asset, dest_asset, ...)
```

---

## Payment Flows

### 1. Simple Deposit & Withdraw

```
User → deposit(1000 USDC)
  → Contract holds 1000 USDC in user's balance
  → User balance: 1000 USDC

User → withdraw(500 USDC)
  → Contract transfers 500 USDC to user
  → User balance: 500 USDC
```

**No fees are deducted on deposits or withdrawals.**

### 2. Immediate Payment (Direct Transfer)

```
Payer (balance: 1000 USDC) → immediate_payment(recipient, asset, 600)
  
Steps:
  1. Calculate fee: max(ceil(600 * 50 / 10000), 100) = 130 stroops
  2. Total deduction: 600 + 130 = 730
  3. Check payer balance: 1000 >= 730 ✓
  4. Deduct 730 from payer → payer balance: 270
  5. Add 600 to recipient → recipient balance: 600
  6. Add 130 to fee balance
  
Result: Payment settled immediately
```

**Fees are deducted from the payer; recipient receives the full amount.**

### 3. Escrow Payment (Time-Locked)

```
Payer → create_escrow_payment(recipient, asset, 500, unlock_time=NOW+1hour)
  
Steps:
  1. Create PaymentOrder with status=Pending
  2. Set escrow_unlock_time = NOW + 1 hour
  3. Payment is locked until unlock time
  4. Emit PaymentCreated and EscrowLocked events
  
Result: Payment is pending, awaiting unlock time

--- 1 hour later ---

Anyone → settle_escrow(order_id)
  
Steps:
  1. Check if unlock_time has passed ✓
  2. Check if condition_hash (if any) matches preimage ✓
  3. Transfer 500 from payer to recipient
  4. Set status=Settled
  5. Emit PaymentSettled event
  
Result: Payment settled; recipient can now withdraw
```

### 4. Conditional Payment (HTLC)

```
Payer → create_escrow_payment(
  recipient, 
  asset, 
  500, 
  unlock_time=NOW+1hour,
  condition_hash=sha256("secret_password")
)

--- Recipient knows the preimage "secret_password" ---

Recipient → settle_escrow(order_id, preimage="secret_password")
  
Steps:
  1. Check unlock_time (may still be in future; condition can unlock early)
  2. Hash preimage: sha256("secret_password") = condition_hash ✓
  3. Transfer 500 from payer to recipient
  4. Set status=Settled
  
Result: Payment settled by preimage revelation
```

### 5. Batch Payment (All-or-Nothing Validation)

```
Payer (balance: 10,000) → batch_payments(vec![
  (recipient_1, asset, 2000),   // Fee: 100
  (recipient_2, asset, 3000),   // Fee: 150
  (recipient_3, asset, 4000),   // Fee: 200
])

PHASE 1: Validation (All-or-Nothing)
  1. Calculate total: 2000 + 3000 + 4000 = 9000
  2. Calculate fees: 100 + 150 + 200 = 450
  3. Total deduction: 9450
  4. Check payer balance: 10000 >= 9450 ✓
  5. Result: Validation PASSED; proceed to Phase 2

PHASE 2: Execution (Sequential)
  1. Transfer 2000 to recipient_1 + 100 fee → payer: 10000 - 2100 = 7900
  2. Transfer 3000 to recipient_2 + 150 fee → payer: 7900 - 3150 = 4750
  3. Transfer 4000 to recipient_3 + 200 fee → payer: 4750 - 4200 = 550
  
Result: All 3 payments settled; payer balance: 550
```

**If validation fails (e.g., insufficient balance), NO payments are settled.**

### 6. Cross-Border Payment

```
Payer (has 1000 USDC-issuer-A) → route_payment(
  recipient,
  source_asset=USDC-issuer-A,
  dest_asset=USDC-issuer-B,
  source_amount=1000,
  destination_amount_min=980,  // Slippage tolerance
  country_from=US,
  country_to=MX
)

Steps:
  1. Check if MX is sanctioned ✓ (not sanctioned)
  2. Get corridor config for (US→MX):
     - max_daily_volume: 1,000,000
     - current_daily_volume: 500,000
     - Check: 500000 + 1000 <= 1000000 ✓
  3. Find cached route: USDC-A → Stellar DEX Pool → USDC-B
  4. Simulate conversion: 1000 USDC-A → 998 USDC-B (0.2% slippage)
  5. Check slippage: 998 >= 980 ✓
  6. Execute: Deduct 1000 from payer, add 998 to recipient
  7. Update corridor daily_volume: 500000 + 1000 = 501000
  
Result: Cross-border payment routed; recipient receives USDC-B
```

---

## Fee Model

### Fee Calculation

Fees are calculated **per payment** at settlement time:

```
fee = max(ceil(amount * platform_fee_bps / 10000), min_fee_stroops)
```

**Example (50 bps fee, 100 stroops minimum):**
- Amount: 1000 → Fee: max(ceil(1000 * 50 / 10000), 100) = max(5, 100) = **100**
- Amount: 100000 → Fee: max(ceil(100000 * 50 / 10000), 100) = max(500, 100) = **500**

### Fee Deduction Points

| Operation | Deposit | Withdraw | Immediate | Escrow (Create) | Escrow (Settle) | Batch |
|-----------|---------|----------|-----------|-----------------|-----------------|-------|
| Fee Deducted | No | No | Yes | No | Yes | Yes |
| Deducted From | — | — | Payer | — | Payer | Payer |
| Recipient Receives | Full | Full | Full | Full | Full | Full |

### Fee Recipient

- **Default:** Contract address (for bootstrapping)
- **Configurable:** Set via `update_fee_config(fee_recipient)` by admin

### Fee Withdrawal

Admin can withdraw collected fees at any time:

```rust
admin.withdraw_fees(asset)
  → Returns fee balance for asset
  → Transfers to admin address
  → Clears contract's fee balance
```

---

## Compliance Framework

### Optional Hooks

The payment system supports optional compliance checks via the `ComplianceOracle` trait:

```rust
pub trait ComplianceOracle {
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
```

### Enforcement Points

Compliance checks can be registered and are invoked at:
1. **Cross-border payments** — Before routing to DEX
2. **Large payments** — Before settlement (when compliance contract registered)

### Sanctions Checking

Cross-border router includes built-in OFAC-style country sanctions:

```rust
set_country_sanctions(country, true)  // Block country
is_country_sanctioned(country)        // Check status
```

**Sanctioned countries reject incoming payments.**

### Corridor Limits

Per-corridor limits prevent excessive flows:

```rust
CorridorConfig {
    max_daily_volume: 1_000_000,  // Daily limit
    max_per_payment: 100_000,      // Per-txn limit
    slippage_tolerance_bps: 100,   // 1% slippage
    enabled: true,
}
```

---

## Events & Off-Chain Indexing

All state changes emit versioned events for off-chain indexing:

### stellar-payments Events

- **payment_created** — Order created (pending)
- **payment_settled** — Payment transferred to recipient
- **payment_refunded** — Payment returned to payer
- **escrow_locked** — Escrow locked until unlock_time
- **balance_updated** — User balance changed

### cross-border-router Events

- **route_cached** — New conversion route cached
- **payment_routed** — Cross-border payment routed
- **routing_failed** — Routing failed (no path)

### Event Structure

All events include:
- **Timestamp** — Ledger timestamp (UTC)
- **Nonce** — Per-event-type counter (for replay protection)
- **Schema Version** — Current event schema version

---

## Error Handling

### Recovery by Error Code

| Code | Error | Recoverable | Action |
|------|-------|------------|--------|
| 505 | Unauthorized | No | Verify caller authorization |
| 506 | PaymentNotFound | No | Verify order_id |
| 507 | InvalidAmount | Yes | Use valid amount > 0 |
| 508 | InvalidAsset | No | Verify asset contract address |
| 509 | InsufficientBalance | Yes | Deposit more tokens |
| 510 | ConditionNotMet | Yes | Provide correct preimage |
| 511 | PaymentLocked | Yes | Wait for unlock_time |
| 512 | PaymentAlreadySettled | No | Order is final (no-op) |
| 513 | AssetTransferFailed | Yes | Verify token contract |
| 514 | RoutingFailed | Yes | Resubmit route (bot task) |
| 515 | ComplianceCheckFailed | Depends | Address compliance issue |
| 516 | BatchLimitExceeded | Yes | Split into smaller batches |
| 517 | StorageFull | No | Contract storage exhausted |

---

## Security Considerations

### Authorization

All state-changing functions require explicit caller authorization via `require_auth()`:
- Deposits: User authorization required
- Withdrawals: User authorization required
- Escrow creation: Payer authorization required
- Immediate payments: Payer authorization required
- Batch payments: Payer authorization required
- Admin functions: Admin authorization required

### State Isolation

- User balances are isolated per (user, asset) pair
- Payment orders are immutable once settled/refunded
- No contract-to-contract calls for token transfers (direct SDK calls)

### Batch Semantics

- **Validation phase:** All-or-nothing; no side effects on failure
- **Execution phase:** Sequential; partial success possible mid-batch
- **Concurrent state:** Users should ensure balance sufficiency with margin

### Escrow Security

- Time-locked: Cannot settle before unlock_time
- Conditional: Preimage hash must match (SHA-256)
- Immutable: Once settled/refunded, order is final

---

## Integration Examples

### Example 1: Deposit and Immediate Payment

```rust
use soroban_sdk::{Address, Env};
use alchemy4_payment_types::types::FeeConfig;

let env = Env::new();
let user = Address::from_contract_id(&env.current_contract_address());
let asset = Address::...;  // USDC contract
let amount = 1000i128;

// Initialize contract
StellarPayments::initialize(&env, admin, Some(FeeConfig {
    platform_fee_bps: 50,   // 0.5%
    min_fee_stroops: 100,
    fee_recipient: admin.clone(),
}))?;

// User deposits
StellarPayments::deposit(&env, user.clone(), asset.clone(), amount)?;
let balance = StellarPayments::get_balance(&env, user.clone(), asset.clone())?;
assert_eq!(balance, 1000);

// User sends 600 to recipient (fee: ~130)
let result = StellarPayments::immediate_payment(
    &env,
    user.clone(),
    recipient.clone(),
    asset.clone(),
    600,
)?;
assert!(result.success);

// User balance decreased by ~730
let new_balance = StellarPayments::get_balance(&env, user.clone(), asset.clone())?;
assert!(new_balance < 1000);
```

### Example 2: Batch Payments

```rust
let orders = vec![
    (recipient_1, asset.clone(), 1000i128),
    (recipient_2, asset.clone(), 2000i128),
    (recipient_3, asset.clone(), 1500i128),
];

let results = StellarPayments::batch_payments(&env, payer.clone(), orders)?;
assert_eq!(results.len(), 3);
for result in results {
    assert!(result.success);
}
```

### Example 3: Cross-Border Routing

```rust
use soroban_sdk::Symbol;

// Submit route (off-chain bot does this)
CrossBorderRouter::submit_route(
    &env,
    source_asset.clone(),
    dest_asset.clone(),
    route_vec,
)?;

// Route payment
let result = CrossBorderRouter::route_payment(
    &env,
    payer.clone(),
    recipient.clone(),
    source_asset.clone(),
    dest_asset.clone(),
    1000i128,
    990i128,   // Slippage tolerance
    Symbol::new(&env, "US"),
    Symbol::new(&env, "MX"),
)?;
assert!(result.success);
```

---

## Deployment Checklist

- [ ] Deploy `alchemy4-payment-types` crate (dependency only; not a contract)
- [ ] Deploy `stellar-payments` contract; call `initialize(admin, fee_config)`
- [ ] Deploy `cross-border-router` contract; call `initialize(admin)`
- [ ] Configure corridors via `set_corridor_config(country_from, country_to, config)`
- [ ] Start off-chain route discovery bot
- [ ] Register compliance oracle if required
- [ ] Verify fee configuration is correct
- [ ] Test with testnet tokens before mainnet deployment

---

## Troubleshooting

### Payment stuck in Pending state
- Check escrow `unlock_time` hasn't expired
- For conditional payments, verify preimage is correct
- Check if payment has been refunded (status=Refunded)

### Batch payments fail mid-execution
- This is expected behavior; payments 1–N may have settled before failure
- Retry remaining payments separately
- Ensure payer has sufficient balance for retries

### Cross-border routing returns RoutingFailed
- Route cache expired; off-chain bot will resubmit
- Country pair may not have corridor configured
- Check country sanctions status

### Fee calculation seems wrong
- Verify `platform_fee_bps` and `min_fee_stroops` via `get_fee_config()`
- Use `estimate_fee(amount)` to predict exact fee before payment

---

## Further Reading

- **Design Document:** `STELLAR_PAYMENT_INTEGRATION.md` (comprehensive technical spec)
- **Error Registry:** `crates/alchemy4-errors/src/lib.rs` (canonical error codes)
- **Type Definitions:** `crates/alchemy4-payment-types/src/types.rs` (all payment types)

---

**Last Updated:** January 2025  
**Status:** Production Ready  
**Maintainer:** Alchemy4 Core Team
