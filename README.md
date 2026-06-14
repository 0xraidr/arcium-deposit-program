# Encrypted deposit MXE

Arcium integration layer for encrypted in-game SOL deposits. This repo is **not** the full game program — it sits beside our main **shard** program and handles the MPC path only.

## Key IDs

| Item | Value |
|------|-------|
| Encrypted deposit program (devnet) | `D29HHVZiZWc1cbR8nDnjfTg8CHEJmLrau3NbcnteFzi` |
| Shard program (CPI target) | `CjBEGXTxN4cMrvNJW6XkLM424pJ6g5aMNfWagDYwtPZy` |
| Devnet cluster offset | `456` |
| Circuit | `deposit` (compiled as `arcium/test-fixtures/deposit.arcis`) |

## What it does

Players submit an encrypted deposit with up to **3 faction legs** (faction index + SOL amount per leg).

On `deposit`:

1. CPI into the shard program to record the commitment, move SOL to the vault, and store ciphertext on user state.
2. Queue one Arcium computation with encrypted args (x25519 pubkey, nonce, leg count + leg ciphertexts).
3. On `deposit_callback`, verify MPC output, validate legs, then CPI back into shard to apply faction SOL updates and mark the deposit receipt consumed.

## Circuit (`encrypted-ixs`)

The `deposit` circuit decrypts/reveals up to 3 `(faction, amount)` legs in one MPC job. Inactive slots use faction `255` and amount `0`.

Source: `arcium/encrypted-ixs/src/lib.rs`

## Program instructions

| Instruction | Role |
|-------------|------|
| `init_deposit_round_comp_def` | Registers the comp def (devnet uses off-chain circuit via public `deposit.arcis` URL) |
| `deposit` | Shard CPI + queue MPC |
| `deposit_callback` | Verify output + shard CPI to finalize |

Deposits require an **active shard round** and open deposit window. Round lifecycle lives in the shard program, not here.

## Issue we're hitting

Deposit txs can succeed (computation queued), but on **devnet cluster 456** we see MPC failures / callbacks returning `AbortedComputation` (6000) after `verify_output` rejects failed cluster output — including cases where on-chain circuit bytes match our local `deposit.arcis`.

Looking for help determining whether this is stale circuit upload, cluster-side circuit fetch/preprocess, or something else in the integration.

## Repo layout

```
arcium/
  encrypted-ixs/     # Arcis circuit source
  programs/encrypted_deposit/   # Anchor + Arcium program
  test-fixtures/     # deposit.arcis + localnet VRF fixtures
  scripts/           # devnet deploy / circuit reset helpers
```

## Setup (local)

```bash
cd arcium
yarn install
cp .env.example .env   # set RPC + wallet paths locally; do not commit .env
```

Requires Anchor `1.0.2`, Arcium CLI `0.10.x`, and the Rust toolchain from `rust-toolchain.toml`.
