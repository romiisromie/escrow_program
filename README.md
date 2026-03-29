# Escrow Program on Solana

This project implements an **Escrow smart contract** using **Anchor + Rust** for Solana blockchain.  
It allows safe token transfers between users with optional cancellation and partial release.

## Features

- `create_escrow` - create a new escrow account
- `deposit_tokens` - deposit tokens into escrow
- `release_tokens` - release tokens to receiver
- `cancel_escrow` - cancel the escrow and return tokens
- Optional: time limits, partial releases, events for tracking operations

## Structure

- `programs/escrow_program` - smart contract code
- `tests/` - test scripts using `Cursor` for simulated transactions

## Installation

```bash
# Install Rust and Anchor
rustup update
cargo install --git https://github.com/coral-xyz/anchor avm --locked
avm install latest
avm use latest

# Clone project
git clone <your-repo-url>
cd escrow_program

# Build program
anchor build
