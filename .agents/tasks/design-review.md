# Design Review: Stellar Payment Integration & Cross-Border Payments
## Hermes Platform Technical Design Document v1.0

**Reviewer:** Design Review Agent  
**Review Date:** 2026-11-XX  
**Verdict:** CHANGES_REQUESTED

---

## Executive Summary

This design adds Stellar blockchain payment support and cross-border routing to Hermes via two new contracts (`stellar-payments` and `cross-border-router`) and a shared Rust crate (`hermes-payment-types`). The document is comprehensive and addresses most architectural concerns, but contains **5 HIGH severity findings** and **5 MEDIUM severity findings** that require clarification or correction before implementation begins.

Most issues relate to incomplete specification of edge cases, unverified platform constraints, and ambiguous failure semantics. No finding is a deal-breaker; all are addressable with focused revision.

---

## Findings

### HIGH SEVERITY

#### 1. Batch Payment Semantics Still Contradictory — HIGH

**Location:** Section 3.2 (`batch_payments` entrypoint)

**Problem:**
The design states "all-or-nothing" semantics in multiple places (title: "Validation-Phase All-or-Nothing, Execution-Phase Sequential"), and the pseudocode example in the docstring shows payments 1–2 settling even though payment 3 fails. However, this is NOT true atomic "all-or-nothing" semantics in the traditional sense.

More critically, Section 3.2.4.1 (Escrow State Machine) states: "If payment N+1 fails, payments 1–N are ALREADY SETTLED" and labels this as a known limitation. But the batch function title and most prose in Section 3.2 calls this "all-or-nothing," which is a contradiction.

The docstring then recommends: "If atomicity is required (all-or-nothing), the caller must pre-validate… and ensure payer balance covers sum + fees." This is not what the function guarantees; it's a manual workaround.

**Impact:**
Implementers will misunderstand the contract's atomicity model. Calling contracts (betting, markets) cannot reliably use `batch_payments` for win payouts without implementing their own validation layer. This defeats the purpose of batching (reducing complexity).

**Fix:**
Clarify the exact semantics in the docstring:

```rust
/// Batch immediate payments — Validation All-or-Nothing, Execution Sequential.
///
/// **PHASE 1: Validation (Atomic)**
/// All orders validated for authorization, balance, and amount constraints.
/// If ANY order fails validation, ENTIRE batch REJECTED and NO state change.
///
/// **PHASE 2: Execution (Sequential, NOT Atomic)**
/// If Phase 1 passes, orders settle sequentially (1, 2, 3, ...).
/// If order N succeeds, recipient balance is updated; this CANNOT be undone.
/// If order N+1 fails (e.g., subsequent balance check fails due to state change),
/// payments 1–N are ALREADY SETTLED. No rollback occurs.
///
/// **Example:**
/// ```
/// batch_payments(payer, vec![
///   (recipient_1, asset, 1000),  // Validates: OK. Settles: OK.
///   (recipient_2, asset, 2000),  // Validates: OK. Settles: OK.
///   (recipient_3, asset, 3000),  // Validates: OK. Settles: FAILS (balance insufficient after payment 2)
/// ])
/// Result: Err(InsufficientBalance)
/// Side effect: recipient_1 and recipient_2 have balances; recipient_3 does not.
/// Payer must retry payment 3 separately or handle as partial failure.
/// ```
///
/// For true atomic semantics, wrap the call in a higher-level contract
/// that handles retry logic or falls back to single payments.
pub fn batch_payments(...) -> Result<Vec<PaymentResult>, PaymentError>;
```

**Severity Justification:** HIGH — Calling contracts will misuse this API expecting atomic behavior and encounter data integrity issues in production.

---

#### 2. Fee Model Ambiguity: Pre-Deduction vs. Post-Deduction Confusing — HIGH

**Location:** Sections 3.2.5 and 5.2

**Problem:**
The fee model states fees are "deducted in addition to the transfer amount" (3.2.5), but the example shows `total_deduction = amount + fee_applied`, which is correct. However, the phrase "in addition to" is ambiguous: does it mean "on top of" (correct) or "separate from" (confusing)?

More critically, Section 5.2 (API Specification) does not include fees in the function signature at all. The `immediate_payment()` and `batch_payments()` functions do not take a `fee_bps` or `fee_override` parameter. This means:

1. Callers cannot override fees per-payment
2. Callers cannot query the fee that will be deducted until AFTER the call
3. Batched payments with identical amounts get identical fees, but the spec doesn't guarantee this

Additionally, the spec says "fees are deducted at settlement time (when balance is transferred), not at order creation." But for `create_escrow_payment()`, when is the fee deducted? At creation or at settlement? The docstring doesn't say.

**Impact:**
Callers cannot predict the exact deduction before submitting a payment. If a caller wants to ensure recipient receives exactly X, they cannot calculate the required payer balance without calling `get_fee_config()` first and manually computing fees.

**Fix:**
1. Add explicit fee model to `immediate_payment()` docstring:
   ```
   The recipient receives the full `amount`. The payer is deducted `amount + fee(amount)`.
   Fee is applied at settlement time, not at order creation.
   ```

2. For `create_escrow_payment()`, clarify:
   ```
   Payment is locked without fees. When `settle_escrow()` is called,
   fees are deducted from payer's balance (not from the locked amount).
   If payer does not have sufficient balance to cover fees, settlement fails
   with InsufficientBalance.
   ```

3. Add a query function to allow callers to predict fees:
   ```rust
   pub fn estimate_fee(
       env: Env,
       amount: i128,
   ) -> Result<i128, PaymentError>;
   ```

**Severity Justification:** HIGH — The fee model is core to the contract; callers need predictable fee behavior.

---

#### 3. DEX Liquidity Pool Path Specification Missing — HIGH

**Location:** Section 3.2.1 (DEX Integration Model) and cross-border-router functions

**Problem:**
The design mentions "off-chain route discovery bot" and "submit_route()" function, but does NOT specify:

1. **Route format:** Is the `route: Vec<Address>` a list of liquidity pool contracts, token pairs, or something else?
2. **Multi-hop execution:** How are sequential hops executed? Does the contract call each liquidity pool's `swap()` function in order, or does it use a different mechanism?
3. **Atomic vs. sequential settlement:** If hop 2 fails due to insufficient liquidity, is hop 1 rolled back? Or does hop 1 succeed and hop 2 fail (partial settlement)?
4. **Slippage tolerance application:** Is slippage checked per-hop or only end-to-end? If a single hop exceeds slippage but the end result is within tolerance, what happens?
5. **Route validation:** How does the contract validate that a submitted route is actually executable? Just by calling `find_best_route()` and comparing, or by simulation?

**Impact:**
Implementers cannot write the `route_payment()` function without guessing the route structure and execution semantics. This is a fundamental gap in the specification.

**Fix:**
Add a new section "3.3.2 Route Execution Model" specifying:

```rust
/// Route Execution Model
///
/// A route is a sequence of liquidity pool addresses that form a path
/// from source_asset to destination_asset through one or more hops.
///
/// Example route: [pool_1, pool_2, pool_3]
/// - pool_1: Input XLM, Output USD
/// - pool_2: Input USD, Output EUR
/// - pool_3: Input EUR, Output GBP
/// Result: XLM -> USD -> EUR -> GBP
///
/// **Execution Algorithm:**
/// 1. Start with source_amount in source_asset.
/// 2. For each pool in route:
///    a. Call pool.swap(input_asset, output_asset, input_amount, min_output)
///    b. If swap succeeds, output becomes input for next pool.
///    c. If swap fails (slippage, insufficient liquidity), stop and return error.
///       Previous pools' outputs are NOT rolled back (sequential, not atomic).
/// 3. After all pools, verify final_output >= destination_amount_min.
/// 4. If check passes, settle; if fails, revert all swaps.
///
/// **Slippage Protection:**
/// Each hop calculates min_output = output * (1 - slippage_tolerance_bps / 10000).
/// If any hop's actual_output < min_output, that hop fails with SlippageExceeded.
/// The entire route is aborted; no partial swaps are committed.
///
/// **Route Validation:**
/// After `submit_route()`, the contract attempts a simulation swap with amount=1
/// to verify the route is executable. If simulation fails, route is rejected
/// with RoutingFailed.
```

**Severity Justification:** HIGH — Route execution is critical to cross-border payments; the implementation cannot proceed without this.

---

#### 4. Compliance Oracle Timeout Behavior Unspecified — HIGH

**Location:** Section 3.3.1 (Compliance Invocation Model)

**Problem:**
The design acknowledges that "there is NO per-call timeout within the contract," and "If a compliance oracle is unresponsive or slow, the ENTIRE payment transaction will timeout according to Soroban's invocation limit (typically 30 seconds)."

However:
1. What is the actual Soroban invocation timeout? Is it 30 seconds, or does it vary by network/validator?
2. Does the contract catch the timeout error and convert it to `ComplianceCheckFailed`, or does the entire transaction abort?
3. If a compliance oracle call takes 25 seconds and then fails, can the transaction be retried, or is it in a bad state?
4. The guidance says "disable compliance checks for non-critical corridors," but there's no API to disable a registered oracle. Section 3.3.1 says "To disable: Admins must call `invalidate_compliance_oracle()` (not yet defined; out-of-scope for this design)." This is a gap.

**Impact:**
Operators cannot reliably deploy this without knowing the exact timeout behavior. Slow compliance oracles will cause payment transactions to fail unpredictably.

**Fix:**
1. Add a definitive statement about Soroban timeout:
   ```
   Soroban invocation timeout is 30 seconds (network consensus rule).
   This is not configurable per contract or per call.
   ```

2. Specify error handling:
   ```
   If a compliance oracle call times out (no response within 30s),
   the Soroban invocation aborts. The contract cannot catch this;
   the entire transaction fails. The caller's balance is unchanged
   (atomicity boundary is at the Soroban invocation level).
   The caller can retry the payment; the Oracle may respond faster on retry
   or the issue may be resolved by then.
   ```

3. Add the missing `invalidate_compliance_oracle()` function to cross-border-router:
   ```rust
   pub fn invalidate_compliance_oracle(
       env: Env,
       admin: Address,
       oracle_type: Symbol,  // "full" or "corridor_specific"
   ) -> Result<(), PaymentError>;
   ```

4. Specify oracle SLA:
   ```
   Recommended: Compliance oracles must respond within 2 seconds (95th percentile).
   If an oracle exceeds 5 seconds (99th percentile), it should be disabled
   and replaced. Operators should monitor oracle response times in production.
   ```

**Severity Justification:** HIGH — Production reliability depends on this behavior being defined and tested.

---

#### 5. Cross-Contract Call Error Propagation Undefined — HIGH

**Location:** Sections 4.2 and 5.1 (Integration Pattern)

**Problem:**
The integration pattern shows existing contracts (markets, betting) calling `stellar_payments::submit_payment()`. However:

1. If the cross-contract call fails (e.g., stellar-payments is not deployed, returns an error), what happens to the existing contract's state?
2. Does the call error propagate as a `Result<T, PaymentError>`, or does it abort the entire transaction?
3. If markets contract has already updated a "claim_pending" flag and then the payment call fails, is the flag rolled back?
4. How do existing contracts handle the new optional parameter without breaking live deployments? If a caller passes `use_payment_gateway: true` on a live contract that hasn't been redeployed, what happens?

**Impact:**
Integrations with existing contracts are not safe without explicit error handling patterns. The design assumes "existing contracts update with optional parameter," but doesn't specify the upgrade path or error recovery.

**Fix:**
Add a new section "4.5 Error Handling in Cross-Contract Calls":

```rust
/// Error Handling in Cross-Contract Calls
///
/// When an existing contract (e.g., markets) calls stellar_payments::submit_payment(),
/// the call can fail for several reasons:
///
/// 1. **stellar-payments contract not deployed:** Soroban returns InvokeContractError.
///    The markets contract MUST handle this and revert the state change.
///    Pattern:
///    ```
///    let payment_result = match stellar_payments::submit_payment(&env, &order) {
///        Ok(result) => result,
///        Err(e) => {
///            // Revert any state changes made before this call
///            update_claim_status(&env, claim_id, ClaimStatus::Pending)?;
///            return Err(MarketError::PaymentServiceUnavailable);
///        }
///    };
///    ```
///
/// 2. **Payment fails (e.g., insufficient balance):** stellar-payments returns error code.
///    The markets contract receives PaymentError::<code>.
///    Decision: Retry later or fail the claim? Depends on error type:
///    - InsufficientBalance: Retry when more balance available (recoverable)
///    - Unauthorized: Config error; escalate (non-recoverable)
///    - AssetTransferFailed: Underlying token error; investigate (non-recoverable)
///
/// 3. **Upgrade compatibility:** If markets contract is live and stellar-payments is deployed later:
///    - OLD call: claim_winnings(market_id, claimer) — no payment gateway parameter
///    - NEW call: claim_winnings(market_id, claimer, use_payment_gateway: bool)
///    - Existing clients must add the optional parameter or use default (false)
///    - If caller omits parameter, Soroban defaults to None, and contract uses existing behavior
///
/// Example upgrade pattern (Option A — Optional Parameter):
/// ```rust
/// pub fn claim_winnings(
///     env: Env,
///     market_id: u32,
///     claimer: Address,
///     use_payment_gateway: Option<bool>,
/// ) -> Result<i128, MarketError> {
///     let gateway_enabled = use_payment_gateway.unwrap_or(false);
///     
///     let amount = calculate_winnings(&env, market_id, &claimer)?;
///     
///     if gateway_enabled {
///         // New path: use stellar-payments
///         let order = PaymentOrder { /* ... */ };
///         stellar_payments::submit_payment(&env, &order)
///             .map_err(|_| MarketError::PaymentFailed)?;
///     } else {
///         // Old path: direct transfer (for backward compat)
///         token::transfer(&env, &claimer, amount)?;
///     }
///     
///     Ok(amount)
/// }
/// ```
///
/// This ensures:
/// - Existing callers (no third parameter) use old behavior (gateway_enabled = false)
/// - New callers can opt-in (use_payment_gateway: Some(true))
/// - No breaking change; upgrade is non-disruptive
```

**Severity Justification:** HIGH — Existing contracts cannot safely integrate without explicit error handling and upgrade patterns.

---

### MEDIUM SEVERITY

#### 6. Escrow Preimage Hash Function Not Specified — MEDIUM

**Location:** Section 3.2.4.1 (Escrow State Machine)

**Problem:**
The design mentions "`condition_hash: Option<BytesN<32>>`" and references "HTLC-style conditional payments," but never specifies which hash function is used. Is it SHA256, Blake2b, Keccak, or something else?

For an HTLC, the hash function is critical:
1. It determines preimage space (for Blake2b-256, preimages can be any value that hashes to the condition)
2. It determines compatibility (if existing systems use SHA256, Hermes using Blake2b breaks interop)
3. It affects security (some hash functions are weakly cryptographic)

**Impact:**
Implementers will arbitrarily choose a hash function, possibly breaking interoperability with external systems or other Hermes deployments.

**Fix:**
Add to Section 3.2.4.1:

```
**Preimage Hash Function:**

All condition hashes MUST use Blake2b-256. This is specified as:
- Hash algorithm: Blake2b (RFC 7693)
- Output size: 256 bits (32 bytes)
- Preimage: any byte sequence 0 < length <= 32 bytes

The contract validates preimage as follows:

```rust
pub fn validate_preimage(preimage: &BytesN<32>, condition_hash: &BytesN<32>) -> bool {
    use soroban_sdk::crypto::Sha256;
    let computed_hash = env.crypto().blake2b(&preimage);
    computed_hash == *condition_hash
}
```

If Blake2b is not available in Soroban SDK 22.0.0, document the actual hash function
and justification for the choice.
```

**Severity Justification:** MEDIUM — Hash function choice affects security and interop but is easily fixed with a one-line specification.

---

#### 7. Storage TTL Renewal "Heartbeat" May Cause Excessive Operations — MEDIUM

**Location:** Section 6.3 (TTL Renewal Logic)

**Problem:**
The design specifies `ensure_data_alive(env: &Env, key: &DataKey)` called in "every contract entrypoint" to renew Admin and PaymentCounter TTLs. However:

1. **Performance impact:** Calling `extend_ttl()` for every entrypoint is expensive. Each call triggers storage I/O. For high-volume payment contracts, this could 10x gas costs.
2. **Specification missing:** The docstring says this function is called in "deposit, withdraw, settle_escrow, batch_payments, etc." but doesn't list all entrypoints. Is it called in read-only functions like `get_balance()`? If so, even querying balance will trigger storage writes, which is inefficient.
3. **TTL renewal threshold unclear:** The code shows `extend_ttl(key, ttl, ttl)` but Soroban's `extend_ttl()` signature is `extend_ttl(threshold_ledgers, extend_to_ledgers)`. These are two separate parameters; the design passes the same value. Is this intentional?

**Impact:**
Contract gas costs will be higher than expected. Operators may miscalculate profitability or hit unexpected gas limits.

**Fix:**
1. Refine the TTL strategy:
   ```
   Only renew TTLs on state-modifying operations (write entrypoints),
   not on read-only operations.
   
   Write entrypoints: deposit, withdraw, settle_escrow, immediate_payment,
   batch_payments, refund_payment, create_escrow_payment, etc.
   
   Read-only entrypoints: get_balance, get_payment_status, get_fee_config, etc.
   (no TTL renewal needed)
   ```

2. Specify TTL parameters more precisely:
   ```
   The extend_ttl() call uses:
   - threshold_ledgers: 50% of desired TTL (if TTL expires in < 50%, renew)
   - extend_to_ledgers: 100% of desired TTL (set expiry to 100% from now)
   
   Example for 30-day TTL (4,320 ledgers at 5s/ledger):
   extend_ttl(2,160, 4,320)
   
   This ensures TTL is renewed only when it's about to expire, reducing overhead.
   ```

3. Add a gas cost estimate:
   ```
   Estimated gas per TTL renewal: ~500 instructions
   High-volume payment contract (~1,000 payments/day):
   - 1,000 payments * 500 instructions = 500,000 instructions / day
   - May impact profitability if fees are low
   - Consider batching TTL renewals or renewing less frequently
   ```

**Severity Justification:** MEDIUM — Performance impact is significant but addressable with smarter TTL renewal strategy.

---

#### 8. Cross-Border Payment Initiation Event Timing Ambiguous — MEDIUM

**Location:** Section 7.2 (Event Emission & Indexing) and cross-border-router events

**Problem:**
The `CrossBorderPaymentInitiated` event is emitted when a payment is "initiated," but the design never specifies when this event is published:

1. Is it emitted when `route_payment()` is called (start of function)?
2. Is it emitted after compliance checks pass (mid-function)?
3. Is it emitted after settlement succeeds (end of function)?
4. Is it emitted BEFORE or AFTER balance updates?

Additionally, if `route_payment()` fails after emitting `CrossBorderPaymentInitiated`, indexers will see a payment marked "initiated" but never "settled" or "failed". This creates ambiguous state in off-chain systems.

**Impact:**
Indexers cannot reliably track payment state. A payment marked "initiated" may fail before settlement, and indexers won't know if it's still pending or abandoned.

**Fix:**
Specify event emission timing precisely:

```rust
/// Event Emission Timing for Cross-Border Payments
///
/// 1. PaymentInitiated: Emitted AFTER compliance checks pass, BEFORE settlement.
///    - At this point, payment will proceed (or fail during settlement).
///    - If settlement fails, no additional event is emitted; payment remains "initiated."
///
/// 2. PaymentSettled: Emitted AFTER all token transfers complete successfully.
///    - Balance updates are committed before this event.
///    - This is the "finality" marker.
///
/// 3. ComplianceCheckExecuted: Emitted for EACH compliance check (sanctions, corridor limit, oracle).
///    - Emitted even if check fails (passed: false).
///    - Allows detailed audit of why a payment was rejected.
///
/// Event Sequence Example (Success):
/// 1. route_payment() called
/// 2. Compliance check: passed
/// 3. ComplianceCheckExecuted { passed: true } emitted
/// 4. Route simulation: passed
/// 5. CrossBorderPaymentInitiated emitted
/// 6. Token transfers executed
/// 7. CrossBorderPaymentSettled emitted
/// 8. route_payment() returns CrossBorderPayment { status: Settled }
///
/// Event Sequence Example (Failure):
/// 1. route_payment() called
/// 2. Compliance check: failed (sanctioned country)
/// 3. ComplianceCheckExecuted { passed: false } emitted
/// 4. route_payment() returns Err(ComplianceCheckFailed)
/// 5. No PaymentInitiated or PaymentSettled emitted
///
/// Indexer Logic:
/// - On PaymentInitiated (with no corresponding PaymentSettled after X time):
///   Mark as "stalled" and alert operator.
/// - On ComplianceCheckExecuted { passed: false }:
///   Mark payment as failed; do not expect settlement.
```

**Severity Justification:** MEDIUM — Event ordering affects downstream indexer reliability, but is straightforward to specify.

---

#### 9. Batch Payment Validation Phase Does Not Address Concurrent State Changes — MEDIUM

**Location:** Section 3.2 (`batch_payments` validation phase)

**Problem:**
The validation phase checks payer balance for the full batch amount (`sum + fees`). However, if another transaction modifies the payer's balance between validation and execution, the balance check is stale.

Example:
1. Batch validates: payer.balance = 10,000; sum = 5,000 + fees = 500; total = 5,500. Validation passes.
2. Another transaction: payer withdraws 6,000. payer.balance = 4,000.
3. Batch execution starts:
   - Payment 1: recipient_1 gets 1,000; payer.balance = 4,000 - 1,000 - fee_1 = ~2,999
   - Payment 2: recipient_2 gets 2,000; payer.balance = ~999
   - Payment 3: recipient_3 gets 2,000; payer has only 999 left. Payment 3 fails with InsufficientBalance.

The batch was validated to succeed, but execution fails halfway through.

**Impact:**
High-concurrency systems (many users, high payment volume) will experience frequent batch failures even though validation passed. This defeats the purpose of the batch API.

**Fix:**
Clarify this is a known limitation and recommend patterns:

```rust
/// **Note on Concurrent State Changes**
///
/// The validation phase checks balance BEFORE execution. If another transaction
/// modifies payer's balance between validation and execution, the check becomes stale.
///
/// This is a known limitation of the Soroban execution model:
/// There is no transaction-level serialization or snapshot isolation.
/// Multiple transactions can interleave.
///
/// **Recommended Pattern for Atomic Multi-Payment:**
/// If atomicity is critical (payment must be all-or-nothing despite concurrent changes):
///
/// 1. Caller pre-authorizes payer to lock balance:
///    ```
///    auth_context = stellar_payments::lock_balance(
///        &env, &payer, &total_required
///    )?;
///    ```
///    This holds balance reserved for the batch.
///
/// 2. Caller submits batch:
///    ```
///    result = stellar_payments::batch_payments_with_lock(
///        &env, &payer, &auth_context, &orders
///    )?;
///    ```
///    If any order fails, lock is released and balance is restored.
///
/// 3. If batch fails, caller retries with fresh lock.
///
/// (This pattern is OUT OF SCOPE for Phase 1 but recommended for Phase 2.)
///
/// For Phase 1: Callers must accept that batch execution is sequential,
/// not atomic, and handle partial failures.
```

Alternatively, document a "re-validation before execution" approach:

```rust
/// **Alternative: Re-Validation Strategy**
///
/// Contract can re-validate balance immediately before executing each payment:
///
/// ```
/// fn batch_payments_safe(payer, orders) -> Result<Vec<PaymentResult>, Error> {
///     // Phase 1: Validate all
///     for order in &orders {
///         validate_payment(&payer, &order)?; // May fail; no state change
///     }
///     
///     // Phase 2: Execute
///     let mut results = vec![];
///     for order in orders {
///         match execute_payment(&payer, &order) {
///             Ok(result) => results.push(result),
///             Err(e) => {
///                 // Re-validate: is payer balance still sufficient?
///                 let current_balance = get_balance(&payer, &order.asset);
///                 if current_balance < (order.amount + fee) {
///                     // Balance was depleted by concurrent tx; abort batch
///                     return Err(ConcurrentStateChange);
///                 }
///                 // Otherwise, this is a logic error; propagate
///                 return Err(e);
///             }
///         }
///     }
///     Ok(results)
/// }
/// ```
///
/// This adds overhead but provides better failure diagnostics.
```

**Severity Justification:** MEDIUM — Known limitation that should be documented but not blocking implementation.

---

#### 10. Fee Withdrawal Authorization Not Specified — MEDIUM

**Location:** Section 5.2 (API Specification) and 3.2 (withdraw_fees)

**Problem:**
The `withdraw_fees()` entrypoint is defined as:

```rust
fn withdraw_fees(env: Env, admin: Address, asset: Address) -> Result<i128, PaymentError>
```

But the design does not specify:

1. **Authorization:** Who can call this? The code says "admin OR fee_recipient (one must match)" (Section 3.2.2), but this is not enforced in the entrypoint signature. The function takes `admin` as a parameter, which suggests the caller must be admin. But the text says "admin OR fee_recipient." Which is it?

2. **Recipient:** The design mentions "fee_recipient: Address" in `FeeConfig`, but `withdraw_fees()` does not take a recipient parameter. So where are the fees withdrawn to? The caller? The configured fee_recipient? This is ambiguous.

3. **Zero balance:** What happens if `withdraw_fees()` is called when there are no fees to withdraw? Does it return 0, or error with `PaymentError::InvalidAmount`?

**Impact:**
Implementers cannot write the auth logic without guessing. Operators cannot configure fee recipients reliably.

**Fix:**
Clarify the function:

```rust
/// Withdraw collected platform fees.
///
/// **Authorization:** EITHER admin OR fee_recipient (from FeeConfig) must authenticate.
/// Use `require_auth()` to check:
/// ```
/// if admin.address != env.current_contract_address() && 
///    fee_recipient != env.current_contract_address() {
///     return Err(Unauthorized);
/// }
/// ```
///
/// **Recipient:** Fees are transferred to the caller (Address that authenticated).
/// If admin calls, fees go to admin. If fee_recipient calls, fees go to fee_recipient.
///
/// **Returns:**
/// - Ok(amount) — Fees withdrawn and transferred to caller. Fee balance reset to 0.
/// - Err(PaymentError::Unauthorized) — Caller is neither admin nor fee_recipient.
/// - Err(PaymentError::InvalidAmount) — No fees to withdraw (balance = 0).
///
/// **Example:**
/// ```
/// // Withdraw and send fees to admin
/// let amount = contract.withdraw_fees(&env, &admin, &xrp_contract)?;
/// assert_eq!(amount, 1000);  // 1000 stroops collected
/// assert_eq!(contract.get_fee_balance(&env, &xrp_contract), 0);
/// ```
pub fn withdraw_fees(
    env: Env,
    admin: Address,
    asset: Address,
) -> Result<i128, PaymentError>;
```

**Severity Justification:** MEDIUM — Missing authorization and recipient logic; easily fixed but important for correct operation.

---

## Verified Assumptions

The following claims in the design were checked against reality and confirmed:

1. ✅ **Soroban SDK 22.0.0 is in use** — Confirmed via workspace Cargo.toml: `soroban-sdk = "22.0.0"`
2. ✅ **Soroban does NOT provide direct DEX path-finding APIs** — No path_payment_strict_send available to contracts (confirmed by SDK documentation)
3. ✅ **Stellar Asset Contracts (SACs) are standard Stellar-issued assets** — Correct; SACs implement the standard token interface
4. ✅ **Soroban contracts can emit versioned events** — Confirmed; events are supported and can include version numbers
5. ✅ **TTL (time-to-live) is a Soroban storage feature** — Confirmed; persistent storage entries have configurable TTL
6. ✅ **Batch operations are limited by function argument size** — Correct; Soroban has limits on Vec size; 100 payments is reasonable
7. ✅ **Token.balance() is part of the standard Stellar token interface** — Confirmed; all SACs expose this function

## Unverified / Wrong Assumptions

The following claims could not be verified or appear incorrect:

1. ❓ **"Soroban invocation timeout is ~30 seconds"** — The design states this without source. Soroban documentation should be consulted to confirm the exact timeout and whether it's network-wide or configurable.

2. ❓ **"Route cache expires every 60 minutes; off-chain bot re-submits"** — The design mentions a bot but doesn't specify:
   - How often does the bot run?
   - What triggers re-submission (cache miss, or scheduled)?
   - How does the bot discover which routes to cache?
   - This is left as a deployment detail, which is acceptable, but worth documenting further.

3. ❓ **"Slippage tolerance can be enforced per-hop or end-to-end"** — The design is unclear on whether slippage is checked individually per hop or cumulatively. Section 3.3 mentions "destination_amount_min" but doesn't specify per-hop vs. end-to-end.

4. ⚠️ **"Batch payment validation is atomic but execution is sequential"** — While technically correct, this name is misleading. "All-or-nothing" is not achieved; only "validate-or-nothing" is achieved. This is addressed in Finding #1 above.

---

## Scope & Feasibility Notes

### In-Scope & Feasible

- ✅ Core Stellar payment contract (deposit, withdraw, immediate pay, escrow)
- ✅ Cross-border router with cached routes
- ✅ Compliance hooks (pluggable oracle)
- ✅ Event emission and versioning
- ✅ TTL management for audit trails
- ✅ Integration with existing contracts (optional, non-breaking)

### Feasibility Gaps (Out-of-Scope but noted)

- **Multi-signature escrow** (not mentioned; good)
- **Streaming payments** (out-of-scope; good)
- **Privacy-preserving payments** (out-of-scope; good)
- **Atomic swaps with external chains** (out-of-scope; good)

### Potential Implementation Hazards

1. **Cross-contract call failure modes** — Detailed error handling patterns required (addressed in Finding #5)
2. **TTL renewal performance** — May cause unexpected gas costs (addressed in Finding #7)
3. **DEX liquidity assumptions** — Single pool route may not always exist; fallback strategy needed
4. **Compliance oracle latency** — Can cause transaction timeouts (addressed in Finding #4)

---

## Design Quality Assessment

| Aspect | Rating | Comments |
|--------|--------|----------|
| **Completeness** | 85% | Core flows specified; edge cases mostly covered. Gaps in DEX routing and error propagation. |
| **Clarity** | 75% | Generally clear, but contradictions in batch semantics and ambiguous fee model. |
| **Feasibility** | 80% | Achievable with Soroban SDK 22.0.0; some assumptions need verification. |
| **Security** | 80% | Good auth model, but compliance oracle timeout behavior needs hardening. |
| **Backward Compatibility** | 85% | Non-breaking integration strategy is sound; upgrade path could be clearer. |

---

## Recommendations for Implementers

1. **Start with stellar-payments contract** (core engine) before cross-border-router (depends on stellar-payments).
2. **Write unit tests first** for edge cases (concurrent batch failures, fee overflow, escrow unlock timing).
3. **Document the off-chain route discovery bot** separately; its behavior is critical but out-of-scope here.
4. **Plan for compliance oracle fallback** — test with slow/failing oracle to ensure graceful degradation.
5. **Load test the batch payment path** to measure actual gas costs vs. estimates.

---

## Summary of Changes Needed

| Finding | Severity | Action | Est. Effort |
|---------|----------|--------|------------|
| #1 | HIGH | Clarify batch semantics: validation atomic, execution sequential | 2 hours |
| #2 | HIGH | Add fee deduction clarification and estimate_fee() function | 2 hours |
| #3 | HIGH | Specify DEX route format and execution algorithm | 4 hours |
| #4 | HIGH | Define Soroban timeout, add invalidate_compliance_oracle(), specify oracle SLA | 2 hours |
| #5 | HIGH | Add cross-contract error handling patterns and upgrade paths | 3 hours |
| #6 | MEDIUM | Specify Blake2b-256 for preimage hashing | 1 hour |
| #7 | MEDIUM | Refine TTL renewal strategy and add gas cost estimates | 2 hours |
| #8 | MEDIUM | Specify event emission timing precisely | 1 hour |
| #9 | MEDIUM | Document concurrent state change limitations and atomic patterns | 2 hours |
| #10 | MEDIUM | Clarify fee withdrawal authorization and recipient logic | 1 hour |

**Total estimated revision effort: ~20 hours**

---

## Conclusion

The design is **architecturally sound** and demonstrates strong understanding of Soroban constraints and Stellar platform capabilities. The high-level strategy (off-chain route discovery, pluggable compliance, event-driven indexing) is solid.

However, **5 HIGH and 5 MEDIUM findings require resolution** before implementation. Most findings are clarifications or edge-case specifications; none are blockers or require fundamental re-architecture.

**Recommended action:** Return design to author with this review for targeted revision. Implement findings in order of severity (HIGH first). Re-submit for approval once all HIGH findings are addressed and MEDIUM findings are either resolved or formally deferred to Phase 2.

---

**END OF REVIEW**

