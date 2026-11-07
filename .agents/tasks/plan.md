# Stellar Payment Integration Implementation Plan

**Project:** Hermes Platform — Stellar Payments & Cross-Border Router  
**Status:** Planning Phase  
**Design Reference:** `payment-design.md` (fully reviewed, approved)

---

## Overview

This plan implements Stellar payment integration for Hermes comprising:

1. **hermes-payment-types crate** — Shared types, traits, and error codes for payment operations
2. **stellar-payments contract** — Core payment engine (deposits, withdrawals, escrow, settlements)
3. **cross-border-router contract** — FX routing and multi-hop currency conversion
4. **Root Cargo.toml updates** — Workspace member additions and dependency management
5. **Documentation** — Integration guide and changelog

**Key Design Decisions:**

- **Modular abstraction via PaymentGateway trait** — Contracts can be swapped without breaking callers
- **Event-driven indexing** — All state changes emit versioned events; no polling required
- **Off-chain route discovery** — Routes cached in contract (60-min TTL) supplied by external bot
- **Decentralized DEX routing** — No centralized exchange or oracle for FX; uses Stellar DEX liquidity pools directly
- **Compliance-ready** — Optional KYC/AML hooks; pluggable oracle interface; no enforcement unless registered
- **Two-phase batch validation** — All-or-nothing validation; sequential (non-atomic) execution with partial settlement semantics
- **Fee model** — Deducted at settlement time (not upfront); separate per-payment calculation in batches
- **Soroban SDK 22.0.0 — Full compliance** with platform constraints (no path-finding APIs, 30-second invocation timeout, TTL management required)

---

## Implementation Sequence

Items are ordered by dependency. Each leaves the codebase in a buildable state.

### Phase 1: Shared Error Registry & Types Crate

- [ ] 1. **Extend hermes-errors error registry with payment error codes.**
      Add error definitions for code range 500–599 (PaymentError codes):
      - 505: Unauthorized
      - 506: PaymentNotFound
      - 507: InvalidAmount
      - 508: InvalidAsset
      - 509: InsufficientBalance
      - 510: ConditionNotMet
      - 511: PaymentLocked
      - 512: PaymentAlreadySettled
      - 513: AssetTransferFailed
      - 514: RoutingFailed
      - 515: ComplianceCheckFailed
      - 516: BatchLimitExceeded
      - 517: StorageFull
      
      Files: `crates/hermes-errors/src/lib.rs`
      
      Verify: Run the hermes-errors build with `cargo build -p hermes-errors --target wasm32-unknown-unknown`. Ensure no compilation errors and error codes are present in the registry.

---

- [ ] 2. **Create hermes-payment-types crate structure.**
      Create new crate with Cargo.toml and lib.rs stub.
      
      Files:
      - `crates/hermes-payment-types/Cargo.toml` (add workspace dependencies: soroban-sdk 22.0.0, hermes-errors)
      - `crates/hermes-payment-types/src/lib.rs` (module exports)
      
      Verify: `cargo build -p hermes-payment-types --target wasm32-unknown-unknown` succeeds with no errors.

---

- [ ] 3. **Implement hermes-payment-types/src/errors.rs.**
      Define PaymentError enum mapping to error codes 505–517.
      Implement From<PaymentError> → u32 for error code conversion.
      Implement Display trait for error messages.
      
      Files: `crates/hermes-payment-types/src/errors.rs`
      
      Verify: `cargo build -p hermes-payment-types --target wasm32-unknown-unknown`. Confirm all error variants compile and error codes are sequential.

---

- [ ] 4. **Implement hermes-payment-types/src/types.rs.**
      Define core types:
      - PaymentStatus enum (Pending, Settled, Refunded, Disputed, Failed)
      - PaymentOrder struct (#[contracttype] with id, payer, recipient, amount, asset_contract, created_at, expected_settlement, status, escrow_unlock_time, condition_hash)
      - PaymentResult struct (order_id, success, ledger_timestamp, error_code)
      - FeeConfig struct (platform_fee_bps, min_fee_stroops, fee_recipient)
      - CorridorConfig struct (max_daily_volume, current_daily_volume, max_per_payment, slippage_tolerance, enabled)
      - CrossBorderPayment struct (id, payer, recipient, source_asset, destination_asset, source_amount, destination_amount_min, country_from, country_to, status, route_taken, effective_rate, created_at)
      
      All structs use #[contracttype] annotation per Soroban SDK patterns.
      
      Files: `crates/hermes-payment-types/src/types.rs`
      
      Verify: `cargo build -p hermes-payment-types --target wasm32-unknown-unknown`. Confirm all types compile and contracttype macro is accepted.

---

- [ ] 5. **Implement hermes-payment-types/src/traits.rs.**
      Define PaymentGateway trait with methods:
      - submit_payment(env: &Env, order: PaymentOrder) → Result<PaymentResult, PaymentError>
      - claim_payment(env: &Env, order_id: u32, recipient: Address) → Result<PaymentResult, PaymentError>
      - refund_payment(env: &Env, order_id: u32, reason: u32) → Result<(), PaymentError>
      - get_payment_status(env: &Env, order_id: u32) → Result<PaymentStatus, PaymentError>
      
      Define ComplianceOracle trait with method:
      - check_payment(env: Env, payer: Address, recipient: Address, country_from: Symbol, country_to: Symbol, amount: i128, asset: Address) → Result<(), PaymentError>
      
      Files: `crates/hermes-payment-types/src/traits.rs`
      
      Verify: `cargo build -p hermes-payment-types --target wasm32-unknown-unknown`. Confirm trait definitions compile without errors.

---

- [ ] 6. **Update hermes-payment-types/src/lib.rs to export all modules.**
      Export errors, types, and traits modules. Include doc comments for the crate.
      
      Files: `crates/hermes-payment-types/src/lib.rs`
      
      Verify: `cargo build -p hermes-payment-types --target wasm32-unknown-unknown && cargo doc -p hermes-payment-types --no-deps` succeeds. Ensure public API is visible in generated docs.

---

### Phase 2: stellar-payments Contract

- [ ] 7. **Create stellar-payments contract scaffold.**
      Create new contract with Cargo.toml and src/lib.rs stub.
      
      Files:
      - `contracts/stellar-payments/Cargo.toml` (dependencies: soroban-sdk 22.0.0, hermes-payment-types, hermes-errors)
      - `contracts/stellar-payments/src/lib.rs` (module declarations)
      
      Verify: `cargo build -p stellar-payments --target wasm32-unknown-unknown` succeeds.

---

- [ ] 8. **Implement stellar-payments/src/errors.rs.**
      Define ContractError enum wrapping PaymentError variants specific to stellar-payments.
      Import error codes from hermes-errors.
      
      Files: `contracts/stellar-payments/src/errors.rs`
      
      Verify: `cargo build -p stellar-payments --target wasm32-unknown-unknown` compiles without errors.

---

- [ ] 9. **Implement stellar-payments/src/storage.rs.**
      Define DataKey enum with all storage keys:
      - Admin, PaymentCounter, Payment(u32), UserBalance(Address, Address), EscrowLock(u32), SettledPayment(u32), ComplianceRegistry(Address), FeeConfig, FeeBalance(Address), Initialized
      
      Implement helper functions:
      - get_next_payment_id(env: &Env) → u32 (increments counter)
      - get_user_balance(env: &Env, user: &Address, asset: &Address) → i128
      - set_user_balance(env: &Env, user: &Address, asset: &Address, balance: i128)
      - get_fee_config(env: &Env) → Option<FeeConfig>
      - set_fee_config(env: &Env, config: FeeConfig)
      
      Files: `contracts/stellar-payments/src/storage.rs`
      
      Verify: `cargo build -p stellar-payments --target wasm32-unknown-unknown` compiles without errors.

---

- [ ] 10. **Implement stellar-payments/src/events.rs.**
       Define PaymentEvent enum with variants:
       - PaymentCreated { order_id, payer, recipient, asset, amount, unlock_time, created_at, nonce }
       - PaymentSettled { order_id, asset, amount, settled_at, ledger_timestamp, nonce }
       - PaymentRefunded { order_id, asset, amount, refunded_at, reason, nonce }
       - EscrowLocked { order_id, unlock_time, has_condition, locked_at, nonce }
       - BalanceUpdated { user, asset, new_balance, updated_at, nonce }
       
       Define PaymentEventSchema struct with schema_version and published_at fields.
       
       Implement helpers:
       - emit_payment_created(env: &Env, ...) — publishes event with topic "payment_created"
       - emit_payment_settled(env: &Env, ...) — publishes event with topic "payment_settled"
       - emit_payment_refunded(env: &Env, ...) — publishes event with topic "payment_refunded"
       - emit_escrow_locked(env: &Env, ...) — publishes event with topic "escrow_locked"
       - emit_balance_updated(env: &Env, ...) — publishes event with topic "balance_updated"
       - get_nonce(env: &Env, event_type: Symbol) → u64 (retrieves and increments per-event nonce)
       
       Files: `contracts/stellar-payments/src/events.rs`
       
       Verify: `cargo build -p stellar-payments --target wasm32-unknown-unknown` compiles without errors.

---

- [ ] 11. **Implement stellar-payments/src/lib.rs — initialization and admin functions.**
        Implement StellarPaymentsContract struct with #[contract] macro.
        Implement #[contractimpl] block with entrypoints:
        
        - **initialize(env: Env, admin: Address, fee_config: Option<FeeConfig>) → Result<(), PaymentError>**
          - Check Initialized flag; return error if already initialized (idempotent check)
          - Set Admin, FeeConfig (or default if None), Initialized flag
          - Emit initialization event (optional)
          - Return Ok(())
        
        - **get_fee_config(env: Env) → Result<FeeConfig, PaymentError>**
          - Query FeeConfig from storage
          - Return or default if not set
        
        - **update_fee_config(env: Env, admin: Address, new_config: FeeConfig) → Result<(), PaymentError>**
          - Call admin.require_auth()
          - Validate new_config (platform_fee_bps <= 10000)
          - Set FeeConfig in storage
          - Return Ok(())
        
        - **estimate_fee(env: Env, amount: i128) → Result<i128, PaymentError>**
          - Validate amount > 0 and <= i128::MAX / 2
          - Get FeeConfig
          - Calculate: fee_calculated = ceil(amount * platform_fee_bps / 10000)
          - Return max(fee_calculated, min_fee_stroops)
        
        - **register_compliance_check(env: Env, admin: Address, compliance_contract: Address) → Result<(), PaymentError>**
          - Call admin.require_auth()
          - Store compliance_contract in ComplianceRegistry
          - Return Ok(())
        
        Files: `contracts/stellar-payments/src/lib.rs`
        
        Verify: `cargo build -p stellar-payments --target wasm32-unknown-unknown` compiles without errors. Run basic unit tests confirming initialization idempotency and fee config updates.

---

- [ ] 12. **Implement stellar-payments/src/lib.rs — deposit and withdrawal functions.**
        Add to StellarPaymentsContract:
        
        - **deposit(env: Env, user: Address, asset: Address, amount: i128) → Result<(), PaymentError>**
          - Call user.require_auth()
          - Validate amount > 0, asset is non-zero
          - Call token.transfer_from(user, contract, amount) to lock balance
          - If transfer fails, return AssetTransferFailed
          - Get current balance, add amount, set new balance
          - Emit BalanceUpdated event
          - Return Ok(())
        
        - **withdraw(env: Env, user: Address, asset: Address, amount: i128) → Result<(), PaymentError>**
          - Call user.require_auth()
          - Validate amount > 0, asset is non-zero
          - Get user balance; if < amount, return InsufficientBalance
          - Call token.transfer(user, amount) to release balance
          - If transfer fails, return AssetTransferFailed
          - Update user balance (subtract amount)
          - Emit BalanceUpdated event
          - Return Ok(())
        
        - **get_balance(env: Env, user: Address, asset: Address) → Result<i128, PaymentError>**
          - Query UserBalance(user, asset) from storage
          - Return 0 if not found
        
        Files: `contracts/stellar-payments/src/lib.rs`
        
        Verify: `cargo build -p stellar-payments --target wasm32-unknown-unknown`. Unit tests confirm deposits/withdrawals update balances correctly and emit events.

---

- [ ] 13. **Implement stellar-payments/src/lib.rs — escrow payment creation and settlement.**
        Add to StellarPaymentsContract:
        
        - **create_escrow_payment(env: Env, payer: Address, recipient: Address, asset: Address, amount: i128, unlock_time: u64, condition_hash: Option<BytesN<32>>) → Result<PaymentOrder, PaymentError>**
          - Call payer.require_auth()
          - Validate amount > 0, unlock_time > env.ledger().timestamp(), asset is non-zero
          - Get next payment ID
          - Create PaymentOrder struct with status=Pending, escrow_unlock_time=Some(unlock_time), condition_hash
          - Store in Payment(id)
          - Store escrow lock in EscrowLock(id) with (unlock_time, condition_hash)
          - If condition_hash is set, store separately for lookup
          - Emit PaymentCreated and EscrowLocked events
          - Return Ok(PaymentOrder)
        
        - **settle_escrow(env: Env, order_id: u32, preimage: Option<BytesN<32>>) → Result<PaymentResult, PaymentError>**
          - Query Payment(order_id); return PaymentNotFound if not exists
          - If order.status != Pending, return PaymentAlreadySettled
          - If env.ledger().timestamp() < order.escrow_unlock_time, return PaymentLocked
          - If order.condition_hash is set, require preimage; compute hash(preimage) and compare; return ConditionNotMet if mismatch
          - Get payer balance; if < order.amount, return InsufficientBalance (payer must still have balance; escrow is held separately)
          - Actually: escrow amount is pre-locked at creation time (subtracted from payer balance or held in escrow storage). If payer balance < order.amount + fees, return InsufficientBalance
          - Transfer order.amount from payer to recipient balance
          - Deduct fees from payer
          - Set order.status = Settled
          - Emit PaymentSettled and BalanceUpdated events
          - Return Ok(PaymentResult { order_id, success: true, ... })
        
        Files: `contracts/stellar-payments/src/lib.rs`
        
        Verify: `cargo build -p stellar-payments --target wasm32-unknown-unknown`. Unit tests confirm escrow locks until unlock_time, condition matching, and settlement transitions status correctly.

---

- [ ] 14. **Implement stellar-payments/src/lib.rs — immediate payment function.**
        Add to StellarPaymentsContract:
        
        - **immediate_payment(env: Env, payer: Address, recipient: Address, asset: Address, amount: i128) → Result<PaymentResult, PaymentError>**
          - Call payer.require_auth()
          - Validate amount > 0, asset is non-zero
          - Calculate fee using estimate_fee()
          - Total deduction = amount + fee
          - Get payer balance; if < total deduction, return InsufficientBalance
          - Create PaymentOrder with status=Pending (non-escrow)
          - Store Payment(id)
          - Transfer amount to recipient balance
          - Deduct total deduction from payer balance
          - Increment fee_balance[asset]
          - Set order.status = Settled
          - Emit PaymentCreated, PaymentSettled, BalanceUpdated events
          - Return Ok(PaymentResult { order_id, success: true, ... })
        
        Files: `contracts/stellar-payments/src/lib.rs`
        
        Verify: `cargo build -p stellar-payments --target wasm32-unknown-unknown`. Unit tests confirm immediate payments deduct fees correctly and emit events.

---

- [ ] 15. **Implement stellar-payments/src/lib.rs — batch payment function.**
        Add to StellarPaymentsContract:
        
        - **batch_payments(env: Env, payer: Address, orders: Vec<(Address, Address, i128)>) → Result<Vec<PaymentResult>, PaymentError>**
          
          **VALIDATION PHASE (All-or-Nothing):**
          - Call payer.require_auth()
          - Validate orders.len() <= 100; return BatchLimitExceeded if not
          - For each (recipient, asset, amount):
            - Validate recipient and asset are non-zero
            - Validate amount > 0
            - Validate asset contract is valid (call token.balance() or use cached validation)
          - Calculate total_amount = sum of all amounts
          - Calculate total_fees = sum of (estimate_fee(amount) for each amount)
          - Total deduction = total_amount + total_fees
          - Get payer balance; if < total deduction, return InsufficientBalance (ALL ORDERS REJECTED; NO STATE CHANGE)
          
          **EXECUTION PHASE (Sequential, NOT Atomic):**
          - Initialize result_vec = Vec::new()
          - For each (i, (recipient, asset, amount)):
            - Get next payment ID
            - Create PaymentOrder with status=Pending
            - Store Payment(id)
            - Calculate fee_i using estimate_fee(amount)
            - Transfer amount to recipient balance (COMMITTED IMMEDIATELY)
            - Deduct (amount + fee_i) from payer balance (COMMITTED IMMEDIATELY)
            - Increment fee_balance[asset] by fee_i
            - Set order.status = Settled
            - Create PaymentResult with success=true
            - Push to result_vec
            - Emit PaymentCreated, PaymentSettled, BalanceUpdated events
            - If ANY token transfer fails (recipient not accessible, token error, etc.):
              - Return Err(PaymentError::AssetTransferFailed) with first failed order index
              - PARTIAL STATE CHANGE: payments 0..i have ALREADY SETTLED; payments i+1.. NOT ATTEMPTED
              - Caller must handle partial failure and retry separately
          
          Return Ok(result_vec) if all payments settle successfully
        
        Files: `contracts/stellar-payments/src/lib.rs`
        
        Verify: `cargo build -p stellar-payments --target wasm32-unknown-unknown`. Unit tests confirm:
          - Validation-phase all-or-nothing (no state change on insufficient balance)
          - Execution-phase sequential settlement (partial success on mid-batch failure)
          - Correct fee deduction per payment
          - Batch limit enforcement (101+ orders rejected)

---

- [ ] 16. **Implement stellar-payments/src/lib.rs — refund and query functions.**
        Add to StellarPaymentsContract:
        
        - **refund_payment(env: Env, order_id: u32, reason: u32) → Result<(), PaymentError>**
          - Query Payment(order_id); return PaymentNotFound if not exists
          - If order.status != Pending, return PaymentAlreadySettled
          - Caller must be order.payer or admin (check via require_auth)
          - Return amount from recipient or escrow back to payer
          - Set order.status = Refunded
          - Emit PaymentRefunded event
          - Return Ok(())
        
        - **get_payment_status(env: Env, order_id: u32) → Result<PaymentStatus, PaymentError>**
          - Query Payment(order_id); return PaymentNotFound if not exists
          - Return order.status
        
        - **validate_token_contract(env: Env, asset: Address) → Result<(), PaymentError>**
          - Attempt to call token.balance(&env.current_contract_address())
          - If call succeeds, return Ok(())
          - If call fails, return Err(InvalidAsset)
        
        - **withdraw_fees(env: Env, admin: Address, asset: Address) → Result<i128, PaymentError>**
          - Call admin.require_auth()
          - Query fee_balance[asset]; if 0, return 0
          - Call token.transfer(fee_recipient, fee_amount)
          - Set fee_balance[asset] = 0
          - Return Ok(fee_amount)
        
        Files: `contracts/stellar-payments/src/lib.rs`
        
        Verify: `cargo build -p stellar-payments --target wasm32-unknown-unknown`. Unit tests confirm refunds restore payer balance, status queries work, and fee withdrawal transfers correctly.

---

- [ ] 17. **Create stellar-payments/tests/ test suite.**
        Create test files covering:
        - `tests/error_tests.rs` — Verify all PaymentError codes are correctly mapped
        - `tests/deposit_withdraw.rs` — Deposit/withdrawal flow with balance validation
        - `tests/immediate_payment.rs` — Immediate payment with fee deduction
        - `tests/escrow_payment.rs` — Escrow creation, time-lock, settlement, and preimage validation
        - `tests/batch_payments.rs` — Batch validation phase (all-or-nothing), execution phase (sequential), and partial failure
        - `tests/fee_config.rs` — Fee configuration updates and estimate_fee()
        
        Files: `contracts/stellar-payments/tests/*.rs`
        
        Verify: `cargo test -p stellar-payments` passes all tests. Confirm test coverage for happy path and error conditions.

---

### Phase 3: cross-border-router Contract

- [ ] 18. **Create cross-border-router contract scaffold.**
       Create new contract with Cargo.toml and src/lib.rs stub.
       
       Files:
       - `contracts/cross-border-router/Cargo.toml` (dependencies: soroban-sdk 22.0.0, hermes-payment-types, stellar-payments, hermes-errors)
       - `contracts/cross-border-router/src/lib.rs` (module declarations)
       
       Verify: `cargo build -p cross-border-router --target wasm32-unknown-unknown` succeeds.

---

- [ ] 19. **Implement cross-border-router/src/errors.rs.**
       Define ContractError enum wrapping PaymentError and routing-specific errors.
       
       Files: `contracts/cross-border-router/src/errors.rs`
       
       Verify: `cargo build -p cross-border-router --target wasm32-unknown-unknown` compiles.

---

- [ ] 20. **Implement cross-border-router/src/storage.rs.**
        Define DataKey enum with all storage keys:
        - Admin, RoutePath(Address, Address), RouteExpiry(Address, Address), Corridor(Symbol, Symbol), CountrySanctioned(Symbol), CrossBorderPayment(u32), PaymentBatch(u32), SettledCrossBorder(u32)
        
        Implement helper functions:
        - get_route(env: &Env, source: &Address, dest: &Address) → Option<Vec<Address>>
        - set_route(env: &Env, source: &Address, dest: &Address, route: Vec<Address>) — with TTL renewal (60-minute TTL)
        - get_corridor_config(env: &Env, country_from: Symbol, country_to: Symbol) → Option<CorridorConfig>
        - set_corridor_config(env: &Env, country_from: Symbol, country_to: Symbol, config: CorridorConfig)
        - is_sanctioned(env: &Env, country: Symbol) → bool
        - set_sanctioned(env: &Env, country: Symbol, sanctioned: bool)
        - get_next_payment_id(env: &Env) → u32 (increments counter)
        
        Files: `contracts/cross-border-router/src/storage.rs`
        
        Verify: `cargo build -p cross-border-router --target wasm32-unknown-unknown` compiles without errors.

---

- [ ] 21. **Implement cross-border-router/src/routing.rs.**
        Implement route management and multi-hop execution:
        
        - **submit_route(env: &Env, source_asset: Address, dest_asset: Address, route: Vec<Address>) → Result<(), PaymentError>**
          - Validate route is non-empty
          - For each pool in route, validate it's a valid liquidity pool (attempt light call or check contract interface)
          - Simulate a 1-stroop transfer through the route to confirm path is executable
          - If any pool fails, return RoutingFailed
          - Store route in RoutePath(source_asset, dest_asset)
          - Set route expiry to current_ledger + 60 minutes (in ledgers)
          - Return Ok(())
        
        - **find_best_route(env: &Env, source_asset: Address, dest_asset: Address, amount: i128) → Result<Vec<Address>, PaymentError>**
          - Query RoutePath(source_asset, dest_asset)
          - If found and not expired, renew TTL to 60 minutes from now and return route
          - If not found or expired, return RoutingFailed
        
        - **execute_multi_hop(env: &Env, route: Vec<Address>, input_amount: i128, min_output: i128, slippage_tolerance_bps: u32) → Result<i128, PaymentError>**
          - Start with input_amount in source_asset
          - For each pool[i] in route:
            - Call pool[i].swap() to convert to next asset in route
            - If swap fails, return AssetTransferFailed (intermediate outputs left in contract; not recovered)
            - Track output amount from this hop; becomes input for next hop
          - After all hops, final_output = output from last pool
          - Validate final_output >= min_output (slippage check); if not, attempt to recover intermediate outputs and return SlippageExceeded
          - Return Ok(final_output)
        
        Files: `contracts/cross-border-router/src/routing.rs`
        
        Verify: `cargo build -p cross-border-router --target wasm32-unknown-unknown` compiles without errors.

---

- [ ] 22. **Implement cross-border-router/src/events.rs.**
        Define CrossBorderPaymentEvent enum with variants:
        - RouteCached { source_asset, dest_asset, hop_count, cached_at, nonce }
        - PaymentRouted { order_id, source_amount, destination_amount, hops, effective_rate, routed_at, nonce }
        - RoutingFailed { source_asset, dest_asset, reason, attempted_at, nonce }
        
        Implement event emission helpers similar to stellar-payments.
        
        Files: `contracts/cross-border-router/src/events.rs`
        
        Verify: `cargo build -p cross-border-router --target wasm32-unknown-unknown` compiles.

---

- [ ] 23. **Implement cross-border-router/src/lib.rs — initialization and admin functions.**
        Implement CrossBorderRouterContract struct with #[contract] macro.
        Implement #[contractimpl] block with entrypoints:
        
        - **initialize(env: Env, admin: Address, corridors: Option<Map<Symbol, CorridorConfig>>) → Result<(), PaymentError>**
          - Similar to stellar-payments initialization
          - Set Admin, Initialized flag
          - If corridors provided, store each corridor config
          - Return Ok(())
        
        - **set_corridor_config(env: Env, admin: Address, country_from: Symbol, country_to: Symbol, config: CorridorConfig) → Result<(), PaymentError>**
          - Call admin.require_auth()
          - Validate country codes are 2-character symbols
          - Store CorridorConfig
          - Return Ok(())
        
        - **get_corridor_config(env: Env, country_from: Symbol, country_to: Symbol) → Result<CorridorConfig, PaymentError>**
          - Query and return CorridorConfig or default if not found
        
        - **set_country_sanctions(env: Env, admin: Address, country: Symbol, sanctioned: bool) → Result<(), PaymentError>**
          - Call admin.require_auth()
          - Validate country is 2-character symbol
          - Set CountrySanctioned(country) = sanctioned
          - Return Ok(())
        
        Files: `contracts/cross-border-router/src/lib.rs`
        
        Verify: `cargo build -p cross-border-router --target wasm32-unknown-unknown` compiles.

---

- [ ] 24. **Implement cross-border-router/src/lib.rs — route discovery and submission.**
        Add to CrossBorderRouterContract:
        
        - **submit_route(env: Env, source_asset: Address, dest_asset: Address, route: Vec<Address>) → Result<(), PaymentError>**
          - Call routing::submit_route(env, source_asset, dest_asset, route)
          - Emit RouteCached event
          - Return Ok(())
        
        - **find_best_route(env: Env, source_asset: Address, dest_asset: Address, amount: i128) → Result<Vec<Address>, PaymentError>**
          - Call routing::find_best_route(env, source_asset, dest_asset, amount)
          - Return route or RoutingFailed
        
        - **get_exchange_rate(env: Env, source_asset: Address, dest_asset: Address, amount: i128) → Result<i128, PaymentError>**
          - Find best route for source_asset → dest_asset
          - Simulate 1-unit transfer through route to determine exchange rate
          - Calculate effective_rate = destination_amount / source_amount (scaled to i128)
          - Return Ok(effective_rate)
        
        - **invalidate_route_cache(env: Env, admin: Address, source_asset: Option<Address>, dest_asset: Option<Address>) → Result<(), PaymentError>**
          - Call admin.require_auth()
          - If both Some: delete RoutePath(source, dest) only
          - If source Some, dest None: delete all routes starting with source
          - If dest Some, source None: delete all routes ending with dest
          - If both None: delete entire route cache
          - Return Ok(())
        
        Files: `contracts/cross-border-router/src/lib.rs`
        
        Verify: `cargo build -p cross-border-router --target wasm32-unknown-unknown` compiles.

---

- [ ] 25. **Implement cross-border-router/src/lib.rs — cross-border payment routing.**
        Add to CrossBorderRouterContract:
        
        - **route_payment(env: Env, payer: Address, recipient: Address, source_asset: Address, dest_asset: Address, source_amount: i128, destination_amount_min: i128, country_from: Symbol, country_to: Symbol) → Result<PaymentResult, PaymentError>**
          
          - Call payer.require_auth()
          - Validate source_amount > 0, destination_amount_min > 0, country codes are valid 2-character symbols
          
          **Compliance Check:**
          - Call check_compliance(env, country_from, country_to, source_amount)
            - Check if country_to is sanctioned; return ComplianceCheckFailed if yes
            - Get corridor config; check if source_amount + current_daily_volume <= max_daily_volume; return ComplianceCheckFailed if not
            - If compliance oracle registered, call oracle.check_payment(...); return ComplianceCheckFailed if oracle fails
          
          **Route Discovery:**
          - Call find_best_route(env, source_asset, dest_asset, source_amount)
          - If route not found, return RoutingFailed
          
          **Slippage Validation:**
          - Simulate multi-hop transfer through route (without committing)
          - Calculate actual_destination_amount
          - If actual_destination_amount < destination_amount_min, return SlippageExceeded (no state change)
          
          **Settlement:**
          - Get next payment ID
          - Create CrossBorderPayment struct with all details
          - Store CrossBorderPayment(id)
          - Execute multi-hop settlement (commit token transfers)
          - Update corridor daily volume
          - Set payment.status = Settled
          - Emit PaymentRouted event
          - Return Ok(PaymentResult { order_id, success: true, ... })
        
        - **check_compliance(env: &Env, country_from: Symbol, country_to: Symbol, amount: i128) → Result<(), PaymentError>**
          - Sanction check: if is_sanctioned(country_to), return ComplianceCheckFailed
          - Corridor limit check: get_corridor_config and compare volume
          - Optional oracle check: call registered compliance oracle if available
          - Return Ok(()) if all checks pass
        
        Files: `contracts/cross-border-router/src/lib.rs`
        
        Verify: `cargo build -p cross-border-router --target wasm32-unknown-unknown` compiles. Unit tests confirm compliance checks, slippage validation, and multi-hop settlement.

---

- [ ] 26. **Create cross-border-router/tests/ test suite.**
        Create test files covering:
        - `tests/error_tests.rs` — Verify error codes
        - `tests/route_management.rs` — Route submission, discovery, and TTL renewal
        - `tests/corridor_config.rs` — Corridor configuration and daily volume tracking
        - `tests/sanction_checks.rs` — Sanction list enforcement
        - `tests/slippage_protection.rs` — Slippage validation and protection
        - `tests/multi_hop_settlement.rs` — Multi-hop token transfers and execution
        - `tests/compliance_oracle.rs` — Oracle invocation and fallback on error
        
        Files: `contracts/cross-border-router/tests/*.rs`
        
        Verify: `cargo test -p cross-border-router` passes all tests.

---

### Phase 4: Root Workspace & Documentation

- [ ] 27. **Update root Cargo.toml to include new crates and contracts.**
        Add to workspace members array:
        - `crates/hermes-payment-types`
        - `contracts/stellar-payments`
        - `contracts/cross-border-router`
        
        Files: `Cargo.toml` (workspace.members section)
        
        Verify: `cargo build --workspace --target wasm32-unknown-unknown` succeeds for all workspace members.

---

- [ ] 28. **Create docs/PAYMENT_INTEGRATION.md guide.**
        Document:
        - High-level architecture and data flow
        - stellar-payments contract overview (deposits, withdrawals, escrow, immediate payments, batching)
        - cross-border-router contract overview (route discovery, DEX routing, slippage protection)
        - Integration guide for other Hermes contracts (adopt PaymentGateway trait, import types)
        - Off-chain route discovery bot requirements and API
        - Compliance oracle interface specification
        - Fee model and calculation examples
        - Example: deposit → immediate_payment → withdrawal flow
        - Example: cross-border payment with multi-hop routing
        - Error handling and recovery patterns
        - Event schema for indexers
        
        Files: `docs/PAYMENT_INTEGRATION.md`
        
        Verify: Markdown syntax is valid; documentation is readable and complete.

---

- [ ] 29. **Update CHANGELOG.md with payment integration entries.**
        Add entries for:
        - New crates: hermes-payment-types
        - New contracts: stellar-payments, cross-border-router
        - Feature summary: Stellar asset payments, escrow, cross-border routing, compliance hooks
        - Breaking changes: None (backward compatible)
        - Deprecations: None
        - Dependencies: Soroban SDK 22.0.0
        
        Files: `CHANGELOG.md`
        
        Verify: CHANGELOG.md is properly formatted (Markdown) and entries are clear.

---

- [ ] 30. **Final workspace build and validation.**
        Run full build and test suite to confirm all components integrate correctly.
        
        Commands:
        - `cargo build --workspace --target wasm32-unknown-unknown`
        - `cargo test --workspace`
        
        Verify:
        - All crates and contracts compile without warnings or errors
        - All unit and integration tests pass
        - WASM binaries are produced for stellar-payments and cross-border-router contracts
        - No unused imports or dead code

---

## Testing & Verification Strategy

### Unit Tests
- Each crate/contract includes unit tests for all public functions
- Error conditions are tested (invalid amounts, insufficient balance, etc.)
- Event emission is verified

### Integration Tests
- stellar-payments and cross-border-router are tested together
- Deposit → payment → withdrawal flows are exercised
- Cross-border routing with multi-hop settlement is tested
- Partial batch failures are verified

### Coverage Metrics
- Target: >= 85% code coverage (unit + integration tests)
- All error codes from hermes-errors are tested

### Manual Verification
- Compile with `cargo build --target wasm32-unknown-unknown`
- Generate WASM binaries for deployment
- Verify contract sizes are reasonable (< 1 MB each, typical for Soroban)

---

## Known Constraints & Design Trade-offs

1. **No In-Contract Path Finding** — Routes are supplied by off-chain bot due to Soroban SDK 22.0.0 limitations. Route cache requires TTL management and periodic renewal.

2. **Sequential Multi-Hop Execution** — Multi-hop settlement is not atomic. Intermediate outputs can be lost if a hop fails mid-execution. Documented as limitation in cross-border-router API.

3. **Batch Partial Failure** — batch_payments() execution phase is sequential; mid-batch failure results in partial settlement. Callers must handle retry logic.

4. **Compliance Oracle Timeout** — No per-call timeout within Soroban. Oracle calls inherit platform 30-second timeout. Slow oracles (> 5 seconds) degrade payment latency.

5. **No Automatic Route Expiry Handling** — When route cache expires, contract returns RoutingFailed. Caller must request fresh discovery; off-chain bot must re-submit.

6. **Fee Deduction Point** — Fees are deducted at settlement time (not upfront). This is by design but means payer must maintain sufficient balance through settlement.

---

## Implementation Order & Dependencies

- **Phase 1 (Serial):** Error registry → Payment types crate (2–3 days)
  - Must complete before contracts can compile

- **Phase 2 (Serial):** stellar-payments contract (4–5 days)
  - Depends on Phase 1
  - Build, test, and validate independently

- **Phase 3 (Serial):** cross-border-router contract (3–4 days)
  - Depends on Phase 1
  - Can be developed in parallel with Phase 2 final stages
  - Final integration tests depend on Phase 2 completion

- **Phase 4 (Serial):** Documentation and root workspace updates (1 day)
  - Depends on Phases 2 & 3 completion

**Total Estimated Effort:** 10–14 days for full implementation + testing

---

## Deliverables

Upon completion of all plan items:

1. ✅ Three new Rust crates/contracts (fully tested, buildable)
2. ✅ hermes-payment-types library (reusable across contracts)
3. ✅ stellar-payments contract (deposits, withdrawals, escrow, immediate & batch payments)
4. ✅ cross-border-router contract (DEX routing, FX conversion, slippage protection, compliance)
5. ✅ Updated root Cargo.toml (workspace members)
6. ✅ Integration guide (docs/PAYMENT_INTEGRATION.md)
7. ✅ Updated CHANGELOG.md
8. ✅ Full test coverage (unit + integration tests)
9. ✅ WASM binaries ready for deployment

---

## Notes for Implementation Team

1. **Soroban SDK 22.0.0 API Reference** — Consult official Soroban docs for contracttype, contract macros, and storage patterns.

2. **Error Code Management** — All error codes must be registered in hermes-errors before contracts can use them. Do not invent new codes outside the 500–599 range.

3. **Event Schema Versioning** — Ensure schema_version is frozen at deployment and included in one canonical record (not repeated per event). Indexers depend on this.

4. **Fee Configuration** — Default fee config (if None provided at initialization) should be sensible: platform_fee_bps=50 (0.5%), min_fee_stroops=1.

5. **Off-Chain Bot Expectations** — Document clearly what the route discovery bot must do: monitor DEX liquidity pools, compute optimal paths, call submit_route() to cache results.

6. **Compliance Oracle Design** — Keep oracle interface simple. Fast < 5 seconds. Document fail-secure (if oracle is unavailable, payment fails).

7. **Testing Approach** — Test contracts locally using soroban-sdk test utilities before deployment. Use Stellar testnet for integration testing with real token contracts.

---

**Status:** Ready for Implementation  
**Approval:** Design Document Approved (payment-design.md)  
**Next Step:** Assign implementation tasks to coding agents (Phase 1 → Phase 4)
