# Alchemy4 - API Reference

Complete API documentation for the Alchemy4 payment platform built on Stellar.

---

## Table of Contents

1. [hermes-payment-types](#hermes-payment-types-crate)
2. [stellar-payments Contract](#stellar-payments-contract)
3. [cross-border-router Contract](#cross-border-router-contract)
4. [Error Codes](#error-codes)
5. [Events](#events)

---

## hermes-payment-types Crate

### Overview

Shared library providing canonical types and traits for all payment-aware Alchemy4 contracts.

**Location:** `crates/alchemy4-payment-types/`  
**Dependencies:** `soroban-sdk = "22.0.0"`

---

### Types

#### PaymentStatus

```rust
#[derive(Clone, Debug, PartialEq, Copy)]
#[contracttype]
pub enum PaymentStatus {
    Pending,      // Created, awaiting settlement
    Settled,      // Transferred and confirmed
    Refunded,     // Returned to payer
    Disputed,     // Under review
    Failed,       // Terminal failure
}
```

**Usage:**
- Query payment state: `get_payment_status(order_id) -> PaymentStatus`
- Check if settled: `if status == PaymentStatus::Settled { ... }`

---

#### PaymentOrder

```rust
#[derive(Clone, Debug)]
#[contracttype]
pub struct PaymentOrder {
    pub id: u32,                          // Unique order ID (auto-assigned)
    pub payer: Address,                   // Payment initiator
    pub recipient: Address,               // Payment destination
    pub amount: i128,                     // Amount (stroops for XLM, base units for tokens)
    pub asset_contract: Address,          // Token contract address
    pub created_at: u64,                  // Ledger timestamp at creation
    pub expected_settlement: u64,         // Advisory settlement time
    pub status: PaymentStatus,            // Current lifecycle state
    pub escrow_unlock_time: Option<u64>,  // Time-lock (if escrow)
    pub condition_hash: Option<BytesN<32>>, // HTLC condition (if conditional)
}
```

**Fields:**
- `id`: Auto-incremented by contract; caller passes 0
- `amount`: In stroops (1 XLM = 10^7 stroops) for XLM; base units for other assets
- `escrow_unlock_time`: If set, payment locked until this timestamp
- `condition_hash`: If set, preimage required to settle (HTLC)

**Example:**
```rust
let order = PaymentOrder {
    id: 0,  // Will be assigned
    payer: user_address,
    recipient: contract_address,
    amount: 1_000_000,  // 0.1 XLM
    asset_contract: xlm_asset,
    created_at: env.ledger().timestamp(),
    expected_settlement: env.ledger().timestamp() + 3600,
    status: PaymentStatus::Pending,
    escrow_unlock_time: None,
    condition_hash: None,
};
```

---

#### PaymentResult

```rust
#[derive(Clone, Debug)]
#[contracttype]
pub struct PaymentResult {
    pub order_id: u32,           // Assigned order ID
    pub success: bool,           // Payment succeeded
    pub ledger_timestamp: u64,   // Settlement timestamp
    pub error_code: Option<u32>, // Error code if failed (see Error Codes)
}
```

**Usage:**
```rust
match submit_payment(env, order) {
    Ok(result) if result.success => {
        println!("Order {} settled", result.order_id);
    }
    Ok(result) => {
        println!("Order {} failed: code {:?}", result.order_id, result.error_code);
    }
    Err(e) => {
        println!("Payment error: {:?}", e);
    }
}
```

---

#### FeeConfig

```rust
#[derive(Clone, Debug)]
#[contracttype]
pub struct FeeConfig {
    pub platform_fee_bps: u32,    // Basis points (e.g., 25 = 0.25%)
    pub min_fee_stroops: i128,    // Minimum fee in stroops
    pub fee_recipient: Address,   // Where fees are sent
}
```

**Fields:**
- `platform_fee_bps`: 0-10000 (10000 = 100%)
- `min_fee_stroops`: Minimum fee in stroops (e.g., 100 = 0.00001 XLM)
- `fee_recipient`: Typically admin or contract address

**Example:**
```rust
let fee_config = FeeConfig {
    platform_fee_bps: 25,    // 0.25% fee
    min_fee_stroops: 100,    // Minimum 0.00001 XLM
    fee_recipient: admin_address,
};
```

---

#### PaymentError

```rust
#[derive(Clone, Debug, PartialEq)]
#[contracterror]
#[repr(u32)]
pub enum PaymentError {
    Unauthorized = 505,              // Caller not authorized
    PaymentNotFound = 506,           // Order ID doesn't exist
    InvalidAmount = 507,             // Amount invalid (0, negative, overflow)
    InvalidAsset = 508,              // Asset contract invalid
    InsufficientBalance = 509,       // Payer insufficient balance
    ConditionNotMet = 510,           // HTLC preimage mismatch
    PaymentLocked = 511,             // Escrow time not reached
    PaymentAlreadySettled = 512,     // Payment terminal state
    AssetTransferFailed = 513,       // Token transfer failed
    RoutingFailed = 514,             // Route not found (cross-border)
    ComplianceCheckFailed = 515,     // KYC/AML hook rejected
    BatchLimitExceeded = 516,        // >100 orders in batch
    StorageFull = 517,               // Contract storage exhausted
}
```

**Error Code Ranges:**
- 505-517: Reserved for payment errors
- Stable across contract upgrades (frozen at compile-time)

---

#### ComplianceOracle

```rust
pub trait ComplianceOracle {
    fn check_payment(
        env: &Env,
        payer: Address,
        recipient: Address,
        amount: i128,
        asset: Address,
        source_country: &str,
        dest_country: &str,
    ) -> Result<bool, String>;
}
```

**Usage:**
```rust
// Implement compliance checks in your contract
impl ComplianceOracle for MyCompliance {
    fn check_payment(
        env: &Env,
        payer: Address,
        recipient: Address,
        amount: i128,
        asset: Address,
        source_country: &str,
        dest_country: &str,
    ) -> Result<bool, String> {
        // Check KYC status
        if !is_kyc_verified(env, &payer) {
            return Ok(false); // Payment blocked
        }

        // Check sanctions lists
        if is_sanctioned_address(env, &payer) {
            return Ok(false); // Payment blocked
        }

        Ok(true) // Payment allowed
    }
}
```

---

#### PaymentGateway Trait

```rust
pub trait PaymentGateway {
    fn submit_payment(
        env: &Env,
        order: PaymentOrder,
    ) -> Result<PaymentResult, PaymentError>;

    fn claim_payment(
        env: &Env,
        order_id: u32,
        recipient: Address,
    ) -> Result<PaymentResult, PaymentError>;

    fn refund_payment(
        env: &Env,
        order_id: u32,
        reason: u32,
    ) -> Result<(), PaymentError>;

    fn get_payment_status(
        env: &Env,
        order_id: u32,
    ) -> Result<PaymentStatus, PaymentError>;
}
```

**Implemented by:** `stellar-payments` contract

---

## stellar-payments Contract

### Overview

Core payment engine handling deposits, withdrawals, immediate payments, and escrow.

**Location:** `contracts/stellar-payments/`  
**Implements:** `PaymentGateway` trait  
**Dependencies:**
- `soroban-sdk = "22.0.0"`
- `hermes-payment-types = { path = "../../crates/hermes-payment-types" }`

---

### Entrypoints

#### initialize

```rust
pub fn initialize(
    env: Env,
    admin: Address,
    platform_fee_bps: u32,
    min_fee_stroops: i128,
    fee_recipient: Address,
) -> Result<(), String>
```

**Purpose:** Set up contract with admin and fee config

**Parameters:**
- `admin`: Admin address (can update fees, withdraw)
- `platform_fee_bps`: Fee in basis points (0-10000)
- `min_fee_stroops`: Minimum fee in stroops
- `fee_recipient`: Address to receive collected fees

**Returns:** `Ok(())` on success, `Err(msg)` on failure

**Error Cases:**
- Admin is invalid address
- platform_fee_bps > 10000

**Example:**
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  -- initialize \
  --admin <ADMIN_ADDRESS> \
  --platform_fee_bps 25 \
  --min_fee_stroops 100 \
  --fee_recipient <FEE_ADDRESS>
```

---

#### deposit

```rust
pub fn deposit(
    env: Env,
    user: Address,
    amount: i128,
    asset: Address,
) -> Result<i128, PaymentError>
```

**Purpose:** Deposit tokens into user's balance

**Parameters:**
- `user`: User depositing tokens
- `amount`: Amount in stroops/base units
- `asset`: Token contract address

**Returns:** Updated balance, or `PaymentError`

**Authorization:** Requires `user.require_auth()`

**Error Cases:**
- `InvalidAmount`: amount <= 0
- `InvalidAsset`: Asset contract invalid
- `AssetTransferFailed`: Token transfer failed
- `StorageFull`: Balance overflow

**Example:**
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source user \
  -- deposit \
  --user <USER_ADDRESS> \
  --amount 1000000 \
  --asset <ASSET_CONTRACT>
```

---

#### withdraw

```rust
pub fn withdraw(
    env: Env,
    user: Address,
    amount: i128,
    asset: Address,
) -> Result<i128, PaymentError>
```

**Purpose:** Withdraw tokens from user's balance

**Parameters:**
- `user`: User withdrawing
- `amount`: Amount to withdraw
- `asset`: Token contract address

**Returns:** Remaining balance, or `PaymentError`

**Authorization:** Requires `user.require_auth()`

**Error Cases:**
- `InvalidAmount`: amount <= 0
- `InsufficientBalance`: User has no balance
- `AssetTransferFailed`: Token transfer failed

**Example:**
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source user \
  -- withdraw \
  --user <USER_ADDRESS> \
  --amount 500000 \
  --asset <ASSET_CONTRACT>
```

---

#### immediate_payment

```rust
pub fn immediate_payment(
    env: Env,
    payer: Address,
    recipient: Address,
    asset: Address,
    amount: i128,
) -> Result<PaymentResult, PaymentError>
```

**Purpose:** Transfer funds immediately with fees deducted

**Parameters:**
- `payer`: Source of funds
- `recipient`: Destination address
- `asset`: Token contract
- `amount`: Amount to transfer (fees added on top)

**Returns:** `PaymentResult` with order ID and settlement timestamp

**Authorization:** Requires `payer.require_auth()`

**Fee Calculation:**
```
fee = max(
    ceil(amount * platform_fee_bps / 10000),
    min_fee_stroops
)
total_deducted = amount + fee
recipient_receives = amount
```

**Error Cases:**
- `InvalidAmount`: amount <= 0
- `InvalidAsset`: Asset invalid
- `InsufficientBalance`: Payer balance < amount + fee
- `AssetTransferFailed`: Token transfer failed

**Example:**
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source payer \
  -- immediate_payment \
  --payer <PAYER_ADDRESS> \
  --recipient <RECIPIENT_ADDRESS> \
  --asset <ASSET_CONTRACT> \
  --amount 1000000
```

---

#### batch_payments

```rust
pub fn batch_payments(
    env: Env,
    payer: Address,
    orders: Vec<(Address, i128)>,  // (recipient, amount) tuples
    asset: Address,
) -> Result<Vec<PaymentResult>, PaymentError>
```

**Purpose:** Process multiple payments in one transaction

**Parameters:**
- `payer`: Source of funds for all orders
- `orders`: Vector of (recipient, amount) pairs (max 100)
- `asset`: Single asset for all payments

**Returns:** Vec of `PaymentResult` for each payment

**Authorization:** Requires `payer.require_auth()`

**Semantics:**
1. **Validation Phase (Atomic):** Check all orders valid, balance sufficient. If ANY check fails, entire batch rejected.
2. **Execution Phase (Sequential):** Execute orders in sequence. Can partially succeed (order N fails, but orders 1..N-1 already settled).

**Error Cases:**
- `BatchLimitExceeded`: orders.len() > 100
- `InvalidAmount`: Any amount invalid
- `InsufficientBalance`: Insufficient balance for all (amount + fees)
- `AssetTransferFailed`: Token transfer failed on execution

**Partial Settlement:** If order N fails during execution, orders 1..N-1 are settled. Caller must handle retry.

**Example:**
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source payer \
  -- batch_payments \
  --payer <PAYER_ADDRESS> \
  --orders '[["ADDR1", 100000], ["ADDR2", 200000]]' \
  --asset <ASSET_CONTRACT>
```

---

#### create_escrow_payment

```rust
pub fn create_escrow_payment(
    env: Env,
    payer: Address,
    recipient: Address,
    asset: Address,
    amount: i128,
    unlock_time: u64,
    condition_hash: Option<BytesN<32>>,
) -> Result<PaymentResult, PaymentError>
```

**Purpose:** Create time-locked (and optionally conditional) payment

**Parameters:**
- `payer`: Source of funds
- `recipient`: Destination (can claim after conditions met)
- `asset`: Token contract
- `amount`: Amount to escrow
- `unlock_time`: Unix timestamp when funds can be released
- `condition_hash`: Optional SHA256 hash of preimage (for HTLC)

**Returns:** `PaymentResult` with order ID

**Authorization:** Requires `payer.require_auth()`

**Conditions for Settlement:**
1. Current time >= `unlock_time`, AND
2. If `condition_hash` set, preimage must match (provided in `settle_escrow`)

**Error Cases:**
- `InvalidAmount`: amount <= 0
- `InvalidAsset`: Asset invalid
- `InsufficientBalance`: Payer balance < amount
- `AssetTransferFailed`: Token transfer failed

**Example:**
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source payer \
  -- create_escrow_payment \
  --payer <PAYER_ADDRESS> \
  --recipient <RECIPIENT_ADDRESS> \
  --asset <ASSET_CONTRACT> \
  --amount 1000000 \
  --unlock_time 1700000000 \
  --condition_hash <HASH>
```

---

#### settle_escrow

```rust
pub fn settle_escrow(
    env: Env,
    order_id: u32,
    preimage: Option<BytesN<32>>,
) -> Result<PaymentResult, PaymentError>
```

**Purpose:** Release escrowed funds to recipient

**Parameters:**
- `order_id`: ID from `create_escrow_payment`
- `preimage`: SHA256 preimage (required if condition_hash was set)

**Returns:** `PaymentResult` with settlement timestamp

**Authorization:** Requires `recipient.require_auth()`

**Validation:**
1. Payment exists and status is Pending
2. Current time >= unlock_time
3. If condition_hash set, preimage must hash to condition_hash

**Error Cases:**
- `PaymentNotFound`: Order doesn't exist
- `PaymentLocked`: unlock_time not reached yet
- `ConditionNotMet`: Preimage doesn't match condition_hash
- `PaymentAlreadySettled`: Payment already settled/refunded

**Example:**
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source recipient \
  -- settle_escrow \
  --order_id 1 \
  --preimage <PREIMAGE_HEX>
```

---

#### refund_payment

```rust
pub fn refund_payment(
    env: Env,
    order_id: u32,
    reason: u32,
) -> Result<(), PaymentError>
```

**Purpose:** Reverse an escrow payment

**Parameters:**
- `order_id`: Payment to refund
- `reason`: Error code or reason (for logging)

**Returns:** `Ok(())` or `PaymentError`

**Authorization:** Requires `payer.require_auth()`

**Conditions:**
- Payment status must be Pending
- Only payer can refund (not admin or recipient)

**Effect:**
- Payment status becomes Refunded
- Funds returned to payer
- No fees deducted on refund

**Error Cases:**
- `PaymentNotFound`: Order doesn't exist
- `PaymentAlreadySettled`: Cannot refund settled/refunded payment
- `Unauthorized`: Only payer can refund

**Example:**
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source payer \
  -- refund_payment \
  --order_id 1 \
  --reason 999
```

---

#### estimate_fee

```rust
pub fn estimate_fee(
    env: Env,
    amount: i128,
    asset: Address,
) -> Result<i128, PaymentError>
```

**Purpose:** Calculate upfront fee for a payment

**Parameters:**
- `amount`: Payment amount (not including fee)
- `asset`: Token contract

**Returns:** Calculated fee amount

**Calculation:**
```
fee = max(
    ceil(amount * platform_fee_bps / 10000),
    min_fee_stroops
)
```

**Error Cases:**
- `InvalidAmount`: amount <= 0
- `InvalidAsset`: Asset invalid

**Example:**
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- estimate_fee \
  --amount 1000000 \
  --asset <ASSET_CONTRACT>
```

---

#### get_payment_status

```rust
pub fn get_payment_status(
    env: Env,
    order_id: u32,
) -> Result<PaymentStatus, PaymentError>
```

**Purpose:** Query current status of a payment

**Parameters:**
- `order_id`: Payment to query

**Returns:** `PaymentStatus` (Pending, Settled, Refunded, Disputed, Failed)

**Error Cases:**
- `PaymentNotFound`: Order doesn't exist

**Example:**
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- get_payment_status \
  --order_id 1
```

---

#### get_user_balance

```rust
pub fn get_user_balance(
    env: Env,
    user: Address,
    asset: Address,
) -> Result<i128, PaymentError>
```

**Purpose:** Query user's deposited balance

**Parameters:**
- `user`: User address
- `asset`: Token contract

**Returns:** Balance in stroops/base units (0 if never deposited)

**Error Cases:**
- `InvalidAsset`: Asset invalid

**Example:**
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- get_user_balance \
  --user <USER_ADDRESS> \
  --asset <ASSET_CONTRACT>
```

---

#### update_fee_config

```rust
pub fn update_fee_config(
    env: Env,
    admin: Address,
    platform_fee_bps: u32,
    min_fee_stroops: i128,
    fee_recipient: Address,
) -> Result<(), String>
```

**Purpose:** Update fee configuration (admin only)

**Parameters:**
- `admin`: Admin address
- `platform_fee_bps`: New fee in basis points
- `min_fee_stroops`: New minimum fee
- `fee_recipient`: New fee recipient address

**Returns:** `Ok(())` or error message

**Authorization:** Requires `admin.require_auth()`

**Validation:**
- platform_fee_bps <= 10000

**Example:**
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  -- update_fee_config \
  --admin <ADMIN_ADDRESS> \
  --platform_fee_bps 30 \
  --min_fee_stroops 50 \
  --fee_recipient <NEW_RECIPIENT>
```

---

#### withdraw_fees

```rust
pub fn withdraw_fees(
    env: Env,
    admin: Address,
    asset: Address,
) -> Result<i128, PaymentError>
```

**Purpose:** Withdraw accumulated fees (admin only)

**Parameters:**
- `admin`: Admin address
- `asset`: Token to withdraw fees for

**Returns:** Amount withdrawn

**Authorization:** Requires `admin.require_auth()`

**Effect:**
- Transfers all accumulated fees for asset to admin
- Sets fee_balance for asset to 0

**Error Cases:**
- `Unauthorized`: Only admin can withdraw
- `AssetTransferFailed`: Token transfer failed

**Example:**
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  -- withdraw_fees \
  --admin <ADMIN_ADDRESS> \
  --asset <ASSET_CONTRACT>
```

---

#### register_compliance_check

```rust
pub fn register_compliance_check(
    env: Env,
    admin: Address,
    compliance_contract: Address,
) -> Result<(), String>
```

**Purpose:** Register optional compliance oracle

**Parameters:**
- `admin`: Admin address
- `compliance_contract`: Compliance contract implementing `ComplianceOracle`

**Returns:** `Ok(())` or error

**Authorization:** Requires `admin.require_auth()`

**Note:** Compliance hooks are optional and not enforced in Phase 1.

**Example:**
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  -- register_compliance_check \
  --admin <ADMIN_ADDRESS> \
  --compliance_contract <COMPLIANCE_CONTRACT>
```

---

## cross-border-router Contract

### Overview

Routes payments across currencies and jurisdictions using Stellar DEX.

**Location:** `contracts/cross-border-router/`  
**Dependencies:** `soroban-sdk = "22.0.0"`, `hermes-payment-types`

---

### Entrypoints

#### initialize

```rust
pub fn initialize(
    env: Env,
    admin: Address,
) -> Result<(), String>
```

**Purpose:** Set up router with admin

**Parameters:**
- `admin`: Admin address

**Returns:** `Ok(())` on success

---

#### submit_route

```rust
pub fn submit_route(
    env: Env,
    source_asset: Address,
    dest_asset: Address,
    route: Vec<Address>,
) -> Result<(), String>
```

**Purpose:** Cache a routing path for conversion

**Parameters:**
- `source_asset`: Source token contract
- `dest_asset`: Destination token contract
- `route`: List of intermediate token contracts (path through DEX)

**Returns:** `Ok(())` on success

**Cache TTL:** 60 minutes from last access

---

#### find_best_route

```rust
pub fn find_best_route(
    env: Env,
    source_asset: Address,
    dest_asset: Address,
) -> Result<Vec<Address>, PaymentError>
```

**Purpose:** Query cached route or return error

**Parameters:**
- `source_asset`: Source token
- `dest_asset`: Destination token

**Returns:** Route (list of intermediate tokens) or `RoutingFailed`

**Note:** TTL renewal is deferred (Phase 2)

---

#### get_exchange_rate

```rust
pub fn get_exchange_rate(
    env: Env,
    source_asset: Address,
    dest_asset: Address,
) -> Result<i128, PaymentError>
```

**Purpose:** Get conversion rate between two assets

**Parameters:**
- `source_asset`: Source token
- `dest_asset`: Destination token

**Returns:** Rate as i128 fixed-point (multiplier * 10^8)

**Note:** Currently returns 1:1 placeholder (Phase 2 feature)

---

#### set_corridor_config

```rust
pub fn set_corridor_config(
    env: Env,
    admin: Address,
    source_country: String,
    dest_country: String,
    enabled: bool,
) -> Result<(), String>
```

**Purpose:** Enable/disable payment corridor

**Parameters:**
- `admin`: Admin address
- `source_country`: Source country code (e.g., "US")
- `dest_country`: Destination country code (e.g., "MX")
- `enabled`: true to allow, false to block

**Returns:** `Ok(())` on success

**Authorization:** Requires `admin.require_auth()`

---

#### check_corridor

```rust
pub fn check_corridor(
    env: Env,
    source_country: String,
    dest_country: String,
) -> Result<bool, String>
```

**Purpose:** Check if payment corridor is allowed

**Parameters:**
- `source_country`: Source country code
- `dest_country`: Destination country code

**Returns:** `Ok(true)` if allowed, `Ok(false)` if blocked

---

#### set_country_sanctions

```rust
pub fn set_country_sanctions(
    env: Env,
    admin: Address,
    country: String,
    sanctioned: bool,
) -> Result<(), String>
```

**Purpose:** Mark country as sanctioned/unsanctioned

**Parameters:**
- `admin`: Admin address
- `country`: Country code
- `sanctioned`: true to add to sanctions list, false to remove

**Returns:** `Ok(())` on success

**Authorization:** Requires `admin.require_auth()`

---

#### is_country_sanctioned

```rust
pub fn is_country_sanctioned(
    env: Env,
    country: String,
) -> Result<bool, String>
```

**Purpose:** Check sanctions status of a country

**Parameters:**
- `country`: Country code

**Returns:** `Ok(true)` if sanctioned, `Ok(false)` otherwise

---

#### route_payment

```rust
pub fn route_payment(
    env: Env,
    payer: Address,
    recipient: Address,
    source_asset: Address,
    dest_asset: Address,
    amount_in_source: i128,
    source_country: String,
    dest_country: String,
) -> Result<PaymentResult, PaymentError>
```

**Purpose:** Execute cross-border payment with FX conversion

**Parameters:**
- `payer`: Source of funds
- `recipient`: Destination address
- `source_asset`: Source token contract
- `dest_asset`: Destination token contract
- `amount_in_source`: Amount to send (in source token)
- `source_country`: Payer's country
- `dest_country`: Recipient's country

**Returns:** `PaymentResult` with order ID

**Validation:**
1. Route exists (source_asset -> dest_asset)
2. Corridor enabled (source_country -> dest_country)
3. Neither country is sanctioned
4. Payer has sufficient balance

**Effect:**
- Converts amount_in_source to dest_asset via route
- Transfers converted amount to recipient
- Deducts fees from destination amount

**Error Cases:**
- `RoutingFailed`: Route not found
- `ComplianceCheckFailed`: Corridor disabled or sanctioned
- `InsufficientBalance`: Payer insufficient balance
- `InvalidAsset`: Asset invalid

---

## Error Codes

| Code | Name | Description |
|------|------|-------------|
| 505 | Unauthorized | Caller not authorized for operation |
| 506 | PaymentNotFound | Payment order doesn't exist |
| 507 | InvalidAmount | Amount invalid (0, negative, overflow) |
| 508 | InvalidAsset | Asset contract address invalid |
| 509 | InsufficientBalance | Payer insufficient balance |
| 510 | ConditionNotMet | HTLC preimage doesn't match |
| 511 | PaymentLocked | Escrow time-lock not reached |
| 512 | PaymentAlreadySettled | Payment already in terminal state |
| 513 | AssetTransferFailed | Token contract transfer failed |
| 514 | RoutingFailed | Cross-border route not found |
| 515 | ComplianceCheckFailed | KYC/AML or corridor check failed |
| 516 | BatchLimitExceeded | Batch has >100 orders |
| 517 | StorageFull | Contract storage exhausted |

---

## Events

### stellar-payments Events

#### PaymentCreated

```rust
pub struct PaymentCreatedEvent {
    pub order_id: u32,
    pub payer: Address,
    pub recipient: Address,
    pub amount: i128,
    pub asset: Address,
    pub nonce: u64,
}
```

Emitted when payment order is created.

#### PaymentSettled

```rust
pub struct PaymentSettledEvent {
    pub order_id: u32,
    pub recipient: Address,
    pub amount: i128,
    pub nonce: u64,
}
```

Emitted when payment is settled.

#### PaymentRefunded

```rust
pub struct PaymentRefundedEvent {
    pub order_id: u32,
    pub payer: Address,
    pub amount: i128,
    pub reason: u32,
    pub nonce: u64,
}
```

Emitted when payment is refunded.

#### BalanceUpdated

```rust
pub struct BalanceUpdatedEvent {
    pub user: Address,
    pub asset: Address,
    pub new_balance: i128,
    pub nonce: u64,
}
```

Emitted when user balance changes.

### cross-border-router Events

#### RoutingDecision

```rust
pub struct RoutingDecisionEvent {
    pub source_asset: Address,
    pub dest_asset: Address,
    pub route_found: bool,
    pub nonce: u64,
}
```

Emitted when route is queried.

#### ConversionExecuted

```rust
pub struct ConversionExecutedEvent {
    pub source_amount: i128,
    pub dest_amount: i128,
    pub exchange_rate: i128,
    pub nonce: u64,
}
```

Emitted when FX conversion is executed.

---

## Quick Reference

### Common Flows

**Deposit and Pay:**
```bash
# 1. Deposit tokens
soroban contract invoke ... -- deposit --user <U> --amount <A> --asset <ASSET>

# 2. Send immediate payment
soroban contract invoke ... -- immediate_payment --payer <U> --recipient <R> --asset <ASSET> --amount <A>

# 3. Check balance
soroban contract invoke ... -- get_user_balance --user <U> --asset <ASSET>
```

**Escrow with Time-Lock:**
```bash
# 1. Create escrow
soroban contract invoke ... -- create_escrow_payment --payer <P> --recipient <R> --asset <ASSET> --amount <A> --unlock_time <T>

# 2. After time, settle
soroban contract invoke ... -- settle_escrow --order_id <ID>
```

**Batch Payments:**
```bash
# Send to multiple recipients in one transaction
soroban contract invoke ... -- batch_payments --payer <P> --orders '[["ADDR1",100],["ADDR2",200]]' --asset <ASSET>
```

---

## References

- **Design Document:** `.agents/tasks/payment-design.md`
- **Integration Guide:** `INTEGRATION_GUIDE.md`
- **Deployment Guide:** `DEPLOYMENT.md`
- **Source Code:** `contracts/stellar-payments/src/lib.rs`, `contracts/cross-border-router/src/lib.rs`

