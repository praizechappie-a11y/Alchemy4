# Alchemy4 - Deployment Guide

## Quick Start

This guide covers building, testing, and deploying Alchemy4, the global Stellar payment platform.

---

## Prerequisites

### System Requirements
- **OS:** Linux, macOS, or Windows (with WSL2 recommended)
- **Rust:** 1.75+ (install from https://rustup.rs/)
- **Soroban CLI:** Latest stable
- **WASM Target:** `wasm32-unknown-unknown`

### Installation Steps

#### 1. Install Rust & Soroban
```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Add WASM target
rustup target add wasm32-unknown-unknown

# Install Soroban CLI
cargo install --locked soroban-cli

# Verify installations
rustc --version
cargo --version
soroban --version
```

#### 2. Clone/Navigate to Alchemy4
```bash
cd alchemy4
```

#### 3. Verify Rust Toolchain
```bash
rustc --version  # Should be 1.75+
cargo --version
```

---

## Building

### Build All Crates (Development)
```bash
cargo build
```

### Build for Wasm (Production)
```bash
# Build with release optimizations (creates WASM artifacts)
cargo build --release --target wasm32-unknown-unknown

# Output location:
# target/wasm32-unknown-unknown/release/*.wasm
```

### Build Specific Crates
```bash
# Build payment types library
cargo build -p alchemy4-payment-types

# Build stellar-payments contract
cargo build -p stellar-payments --release --target wasm32-unknown-unknown

# Build cross-border-router contract
cargo build -p cross-border-router --release --target wasm32-unknown-unknown
```

### Build Options

**Optimized WASM (Smallest Size)**
```bash
RUSTFLAGS="-C link-arg=-s" cargo build --release --target wasm32-unknown-unknown
```

**With Logging**
```bash
cargo build --release --target wasm32-unknown-unknown --profile release-with-logs
```

---

## Testing

### Run All Tests
```bash
cargo test
```

### Run Tests for Specific Crate
```bash
cargo test -p stellar-payments
cargo test -p cross-border-router
cargo test -p alchemy4-payment-types
```

### Run Tests with Output
```bash
cargo test -- --nocapture
```

### Run Integration Tests Only
```bash
cargo test --test '*' -- --nocapture
```

### Coverage Report
```bash
# Requires tarpaulin (install: cargo install cargo-tarpaulin)
cargo tarpaulin --out Html --output-dir coverage
```

---

## Deployment to Stellar Testnet

### Step 1: Set Up Environment

```bash
# Configure Soroban network (Testnet)
soroban network add --rpc-url https://soroban-testnet.stellar.org:443 --network-passphrase "Test SDF Network ; September 2015" testnet

# Or use Futurenet
soroban network add --rpc-url https://rpc-futurenet.stellar.org:443 --network-passphrase "Test SDF Future Network ; October 2022" futurenet

# Set default network
soroban network set testnet  # or futurenet
```

### Step 2: Create or Import Account

```bash
# Generate new keypair
soroban keys generate --name alchemy4-admin

# Or import existing
soroban keys generate --name alchemy4-admin --secret-key "S..."
```

### Step 3: Fund Account (Testnet)

Visit https://friendbot.stellar.org/ and paste your public key to fund it with 10,000 XLM.

### Step 4: Build WASM Artifacts

```bash
cargo build --release --target wasm32-unknown-unknown
```

### Step 5: Deploy Contracts

#### A. Deploy Payment Types (Library Only - No Deployment Needed)
The `alchemy4-payment-types` crate is a library and does not need deployment. It's imported by other contracts.

#### B. Deploy Stellar Payments Contract

```bash
# Build
cargo build -p stellar-payments --release --target wasm32-unknown-unknown

# Deploy
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/stellar_payments.wasm \
  --source alchemy4-admin \
  --network testnet

# Save contract address (output)
# Example: CAKQ65F3I2RKBAHW7Z2E3JGYQZZ3H2FKQNRQU5KXTV5HZFQY4S3AYBC
```

#### C. Deploy Cross-Border Router Contract

```bash
# Build
cargo build -p cross-border-router --release --target wasm32-unknown-unknown

# Deploy
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/cross_border_router.wasm \
  --source alchemy4-admin \
  --network testnet

# Save contract address (output)
```

### Step 6: Initialize Contracts

#### Initialize Stellar Payments

```bash
# Set up admin and fee config
soroban contract invoke \
  --id <STELLAR_PAYMENTS_CONTRACT_ID> \
  --source alchemy4-admin \
  --network testnet \
  -- initialize \
  --admin <YOUR_PUBLIC_KEY> \
  --platform_fee_bps 25 \
  --min_fee_stroops 100 \
  --fee_recipient <FEE_RECIPIENT_ADDRESS>
```

#### Initialize Cross-Border Router

```bash
soroban contract invoke \
  --id <CROSS_BORDER_ROUTER_CONTRACT_ID> \
  --source alchemy4-admin \
  --network testnet \
  -- initialize \
  --admin <YOUR_PUBLIC_KEY>
```

---

## Deployment to Stellar Mainnet

**WARNING: Only deploy to mainnet when thoroughly tested on testnet/futurenet.**

### Pre-Mainnet Checklist
- [ ] All tests passing locally
- [ ] Deployed and tested on Testnet for 1+ week
- [ ] Code audited by security firm
- [ ] Admin key stored securely (hardware wallet recommended)
- [ ] Fee parameters finalized
- [ ] Compliance hooks configured (if applicable)

### Mainnet Deployment

```bash
# Configure mainnet network
soroban network add \
  --rpc-url https://soroban-mainnet.stellar.org:443 \
  --network-passphrase "Public Global Stellar Network ; August 2014" \
  mainnet

# Set mainnet as default
soroban network set mainnet

# Create mainnet admin key (use hardware wallet)
soroban keys generate --name alchemy4-mainnet-admin

# Fund account (requires sending XLM from exchange)

# Deploy (same steps as testnet, use mainnet network)
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/stellar_payments.wasm \
  --source alchemy4-mainnet-admin \
  --network mainnet

# Initialize with production parameters
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source alchemy4-mainnet-admin \
  --network mainnet \
  -- initialize \
  --admin <MAINNET_ADMIN_KEY> \
  --platform_fee_bps 25 \
  --min_fee_stroops 100 \
  --fee_recipient <MAINNET_FEE_ADDRESS>
```

---

## Contract Addresses Reference

Create a `.env` file or environment variables:

```bash
# Testnet
STELLAR_PAYMENTS_TESTNET=CAxxxxxx
CROSS_BORDER_ROUTER_TESTNET=CAxxxxxx

# Mainnet
STELLAR_PAYMENTS_MAINNET=CAxxxxxx
CROSS_BORDER_ROUTER_MAINNET=CAxxxxxx

# Assets (pre-existing or custom)
USDC_CONTRACT=CAxxxxxx
NATIVE_STELLAR_XLM=native
```

---

## Verification

### Check Deployment

```bash
# Verify contract is deployed
soroban contract info --id <CONTRACT_ID> --network testnet

# Check contract state
soroban contract invoke \
  --id <STELLAR_PAYMENTS_CONTRACT_ID> \
  --network testnet \
  -- get_payment_status \
  --order_id 1
```

### Run Integration Test

```bash
# Full integration test (requires deployed contracts)
cargo test --test integration_test -- --nocapture --ignored
```

---

## Monitoring & Maintenance

### Query Contract State

```bash
# Get user balance
soroban contract invoke \
  --id <STELLAR_PAYMENTS_CONTRACT_ID> \
  --source <USER_ADDRESS> \
  --network testnet \
  -- get_user_balance \
  --user <USER_ADDRESS> \
  --asset <ASSET_CONTRACT>

# Get payment status
soroban contract invoke \
  --id <STELLAR_PAYMENTS_CONTRACT_ID> \
  --network testnet \
  -- get_payment_status \
  --order_id <ORDER_ID>

# Estimate fee
soroban contract invoke \
  --id <STELLAR_PAYMENTS_CONTRACT_ID> \
  --network testnet \
  -- estimate_fee \
  --amount 1000000 \
  --asset <ASSET_CONTRACT>
```

### Withdraw Fees

```bash
soroban contract invoke \
  --id <STELLAR_PAYMENTS_CONTRACT_ID> \
  --source alchemy4-admin \
  --network testnet \
  -- withdraw_fees \
  --asset <ASSET_CONTRACT>
```

### Update Fee Configuration

```bash
soroban contract invoke \
  --id <STELLAR_PAYMENTS_CONTRACT_ID> \
  --source alchemy4-admin \
  --network testnet \
  -- update_fee_config \
  --platform_fee_bps 30 \
  --min_fee_stroops 50 \
  --fee_recipient <NEW_RECIPIENT>
```

---

## Troubleshooting

### Build Errors

**Error: `error: linker 'cc' not found`**
- **Fix:** Install C compiler (MSVC on Windows, gcc/clang on Linux/macOS)
- Windows: Install Visual Studio Build Tools
- Linux: `sudo apt-get install build-essential`
- macOS: `xcode-select --install`

**Error: `error: failed to download`**
- **Fix:** Check internet connection and network proxy settings
```bash
cargo clean
cargo build --release
```

### Deployment Errors

**Error: `error: Account not found`**
- **Fix:** Ensure account is funded on the network you're deploying to
- Testnet: Use https://friendbot.stellar.org/

**Error: `error: Insufficient balance`**
- **Fix:** Account needs XLM to pay for transaction/deployment
- Testnet: Fund with Friendbot
- Mainnet: Send XLM from exchange

**Error: `error: Invalid network`**
- **Fix:** Verify network is configured correctly
```bash
soroban network list
soroban network set <NETWORK_NAME>
```

### Runtime Errors

**"PaymentNotFound" when querying payment**
- Verify payment ID exists
- Check correct network (testnet vs mainnet)
- Confirm contract address

**"InsufficientBalance" on payment**
- User must deposit funds first via `deposit()` entrypoint
- Verify balance: `get_user_balance()`

**"InvalidAsset" on payment**
- Ensure asset contract address is valid
- Asset must be Stellar asset or SAC token

---

## Performance Optimization

### WASM Size Reduction
```bash
# Current optimization level is -z (smallest)
# For even smaller, use wasm-opt:
cargo install wasm-opt
wasm-opt -Oz -o output.wasm input.wasm
```

### Gas Optimization
- Batch payments reduce per-payment overhead
- Escrow with time-locks is cheaper than conditional settlements
- Consider caching frequently-used routes

---

## Security Considerations

### Admin Key Management
- Store admin key in hardware wallet
- Use air-gapped signing for mainnet
- Rotate admin key periodically
- Never share private key

### Fee Configuration
- Start with conservative fees on testnet
- Monitor fee collection rates
- Update fee_recipient only via admin function
- Keep fee_config up-to-date

### Compliance Hooks
- Register compliance oracles before accepting payments
- Test compliance checks on testnet
- Have legal review compliance rules before mainnet

---

## References

- **Design Document:** `.agents/tasks/payment-design.md`
- **API Reference:** `docs/API_REFERENCE.md`
- **Integration Guide:** `docs/INTEGRATION_GUIDE.md`

---

## Support

For issues or questions:
1. Check `.agents/tasks/impl-review.md` for known findings
2. Review design document (`.agents/tasks/payment-design.md`)
3. Check Soroban documentation (https://soroban.stellar.org/)
4. Review integration examples in `docs/INTEGRATION_GUIDE.md`

