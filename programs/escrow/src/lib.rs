//! SPL Token Escrow Program: deposit to vault, release to receiver, or cancel to refund sender.

use anchor_lang::prelude::*;
use anchor_lang::space::InitSpace;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("Esc111111111111111111111111111111111111111");

#[program]
pub mod escrow {
    use super::*;

    /// Creates a new escrow with optional vault ATA for the mint.
    pub fn create_escrow(
        ctx: Context<CreateEscrow>,
        amount: u64,
        expiry_time: i64,
        release_authority: Option<Pubkey>,
    ) -> Result<()> {
        require!(amount > 0, EscrowError::ZeroAmount);

        let escrow = &mut ctx.accounts.escrow;
        escrow.sender = ctx.accounts.sender.key();
        escrow.receiver = ctx.accounts.receiver.key();
        escrow.mint = ctx.accounts.mint.key();
        escrow.amount = amount;
        escrow.is_completed = false;
        escrow.expiry_time = expiry_time;
        escrow.release_authority = release_authority;
        escrow.vault_bump = ctx.bumps.vault_authority;
        escrow.bump = ctx.bumps.escrow;

        emit!(EscrowCreated {
            escrow: escrow.key(),
            sender: escrow.sender,
            receiver: escrow.receiver,
            mint: escrow.mint,
            amount: escrow.amount,
            expiry_time: escrow.expiry_time,
            release_authority: escrow.release_authority,
        });
        Ok(())
    }

    /// Moves tokens from sender to the vault.
    pub fn deposit_tokens(ctx: Context<DepositTokens>) -> Result<()> {
        let escrow = &ctx.accounts.escrow;
        require!(!escrow.is_completed, EscrowError::AlreadyCompleted);

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.sender_token_account.to_account_info(),
                    to: ctx.accounts.vault_token_account.to_account_info(),
                    authority: ctx.accounts.sender.to_account_info(),
                },
            ),
            escrow.amount,
        )?;

        emit!(TokensDeposited {
            escrow: escrow.key(),
            mint: escrow.mint,
            amount: escrow.amount,
        });
        Ok(())
    }

    /// Transfers tokens to the receiver. Can be called by receiver or optional authority.
    pub fn release_tokens(ctx: Context<ReleaseTokens>) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;
        require!(!escrow.is_completed, EscrowError::AlreadyCompleted);

        let now = Clock::get()?.unix_timestamp;
        if escrow.expiry_time > 0 {
            require!(now < escrow.expiry_time, EscrowError::EscrowExpired);
        }

        let seeds: &[&[u8]] = &[b"vault", escrow.key().as_ref(), &[escrow.vault_bump]];
        let signer = &[seeds];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    to: ctx.accounts.receiver_token_account.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                signer,
            ),
            escrow.amount,
        )?;

        escrow.is_completed = true;

        emit!(TokensReleased {
            escrow: escrow.key(),
            receiver: escrow.receiver,
            mint: escrow.mint,
            amount: escrow.amount,
        });
        Ok(())
    }

    /// Refunds tokens to the sender and closes vault & escrow accounts.
    pub fn cancel_escrow(ctx: Context<CancelEscrow>) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;
        require!(!escrow.is_completed, EscrowError::AlreadyCompleted);

        let now = Clock::get()?.unix_timestamp;
        if escrow.expiry_time > 0 {
            require!(now >= escrow.expiry_time, EscrowError::CancelTooEarly);
        }

        let seeds: &[&[u8]] = &[b"vault", escrow.key().as_ref(), &[escrow.vault_bump]];
        let signer = &[seeds];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    to: ctx.accounts.sender_token_account.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                signer,
            ),
            escrow.amount,
        )?;

        token::close_account(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::CloseAccount {
                account: ctx.accounts.vault_token_account.to_account_info(),
                destination: ctx.accounts.sender.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            },
            signer,
        ))?;

        escrow.is_completed = true;

        emit!(EscrowCancelled {
            escrow: escrow.key(),
            sender: escrow.sender,
            mint: escrow.mint,
            amount: escrow.amount,
        });

        Ok(())
    }
}

// --- State ---
#[account]
#[derive(InitSpace)]
pub struct EscrowAccount {
    pub sender: Pubkey,
    pub receiver: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub is_completed: bool,
    pub expiry_time: i64,
    pub release_authority: Option<Pubkey>,
    pub vault_bump: u8,
    pub bump: u8,
}

// --- Events ---
#[event]
pub struct EscrowCreated {
    pub escrow: Pubkey,
    pub sender: Pubkey,
    pub receiver: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub expiry_time: i64,
    pub release_authority: Option<Pubkey>,
}

#[event]
pub struct TokensDeposited {
    pub escrow: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

#[event]
pub struct TokensReleased {
    pub escrow: Pubkey,
    pub receiver: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

#[event]
pub struct EscrowCancelled {
    pub escrow: Pubkey,
    pub sender: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

// --- Errors ---
#[error_code]
pub enum EscrowError {
    #[msg("Escrow already completed or closed.")]
    AlreadyCompleted,
    #[msg("Amount must be greater than zero.")]
    ZeroAmount,
    #[msg("Release window ended; cancel to refund sender.")]
    EscrowExpired,
    #[msg("Cancellation before expiry_time is not allowed.")]
    CancelTooEarly,
    #[msg("Signer not authorized to release.")]
    UnauthorizedRelease,
    #[msg("Sender does not match escrow.")]
    InvalidSender,
    #[msg("Vault already holds tokens; single deposit only.")]
    VaultNotEmpty,
}
