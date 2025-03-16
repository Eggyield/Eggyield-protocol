use anchor_lang::prelude::*;
use anchor_lang::solana_program::{pubkey::Pubkey, system_instruction};
use anchor_lang::solana_program::native_token::sol_to_lamports;
use anchor_spl::token::{Token, Transfer, TokenAccount};
use anchor_spl::token::transfer;
use borsh::BorshDeserialize;
use std::convert::TryInto; // For safe conversions between types

// Declare ID
declare_id!("XXXXXXXXXXXXXXXXXXXXXXXXXXCONTRACT");

// Constants
const VAULT_SEED: &[u8] = b"vault";
const TOKEN_VAULT_SEED: &[u8] = b"token_vault";
const INTERACTOR_SEED: &[u8] = b"interactor";
const MILLISECONDS_IN_SECOND: u64 = 1000;
const SECONDS_IN_MINUTE: u64 = 60;
const MINUTES_IN_HOUR: u64 = 60;
const MAX_HELD_HOURS: u64 = 24;

// Utility function to check if all elements in an iterator are equal
pub fn iter_all_eq<T: PartialEq>(iter: impl IntoIterator<Item = T>) -> Option<T> {
    let mut iter = iter.into_iter();
    let first = iter.next()?;
    iter.all(|elem| elem == first).then(|| first)
}

#[program]
pub mod egg_vault {

    use super::*;

    pub fn create_egg_vault(
        ctx: Context<CreateVault>,
        amount: u64,
        base_rate: f32,
        base_hour: u32
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault;

        // Initialize the vault
        vault.amount = amount;
        vault.amount_staked = 0;
        vault.start_pool = amount;
        vault.base_rate = base_rate;
        vault.base_hour = base_hour;
        vault.total_stakers = 0;
        vault.current_stakers = 0;

        // Transfer the tokens into the vault from creator
        let cpi_accounts = Transfer {
            from: ctx.accounts.creator_token_account.to_account_info(),
            to: ctx.accounts.token_account.to_account_info(),
            authority: ctx.accounts.creator.to_account_info(),
        };

        let cpi_program = ctx.accounts.token_program.to_account_info();
        transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;

        Ok(())
    }

    pub fn deposit_eggs(ctx: Context<Deposit>, amount: u64, index: usize) -> Result<()> {
        let clock = Clock::get()?;
    
        // Safely validate and update `total_deposits`
        let total_deposits = &mut ctx.accounts.user_interactions_counter.total_deposits;
        if let Some(total) = total_deposits.get_mut(index) {
            *total = amount;
        } else {
            return Err(ErrorCode::InvalidIndex.into());
        }
    
        // Safely validate and update `time_deposits`
        let time_deposits = &mut ctx.accounts.user_interactions_counter.time_deposits;
        if let Some(time) = time_deposits.get_mut(index) {
            *time = clock.unix_timestamp.try_into().map_err(|_| ErrorCode::Overflow)?;
        } else {
            return Err(ErrorCode::InvalidIndex.into());
        }
    
        // Safely validate and update `stake_deposits`
        let stake_deposits = &mut ctx.accounts.user_interactions_counter.stake_deposits;
        if let Some(stake) = stake_deposits.get_mut(index) {
            *stake = clock.unix_timestamp.try_into().map_err(|_| ErrorCode::Overflow)?;
        } else {
            return Err(ErrorCode::InvalidIndex.into());
        }
    
        // Update vault's staked amount safely
        ctx.accounts.vault.amount_staked = ctx
            .accounts
            .vault
            .amount_staked
            .checked_add(amount)
            .ok_or(ErrorCode::Overflow)?;
    
        // Create and execute the CPI transfer
        let cpi_accounts = Transfer {
            from: ctx.accounts.depositor_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.depositor.to_account_info(),
        };
    
        let cpi_program = ctx.accounts.token_program.to_account_info();
        transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;
    
        Ok(())
    }    
    
    pub fn withdraw_eggs(ctx: Context<Withdraw>, index: usize, reward_only: bool) -> Result<()> {    
        // Safely access `total_deposits` array and validate index
        let total_deposits = &ctx.accounts.user_interactions_counter.total_deposits;
        let amount = total_deposits.get(index).ok_or(ErrorCode::InvalidIndex)?;
    
        // Safely access `stake_deposits` array
        let stake_deposits = &ctx.accounts.user_interactions_counter.stake_deposits;
        let stake_time = stake_deposits.get(index).ok_or(ErrorCode::InvalidIndex)?;
    
        let clock = Clock::get()?;
        let unix_timestamp: u64 = clock
            .unix_timestamp
            .try_into()
            .map_err(|_| ErrorCode::Overflow)?;
        let time_elapsed = unix_timestamp
            .checked_sub(*stake_time)
            .ok_or(ErrorCode::Overflow)?
            .checked_mul(MILLISECONDS_IN_SECOND)
            .ok_or(ErrorCode::Overflow)?;
    
        // Safely update user interaction counters
        let counters = &mut ctx.accounts.user_interactions_counter;
        if !reward_only {
            if let Some(total) = counters.total_deposits.get_mut(index) {
                *total = 0;
            } else {
                return Err(ErrorCode::InvalidIndex.into());
            }
    
            if let Some(time) = counters.time_deposits.get_mut(index) {
                *time = 0;
            } else {
                return Err(ErrorCode::InvalidIndex.into());
            }
    
            if let Some(stake) = counters.stake_deposits.get_mut(index) {
                *stake = 0;
            } else {
                return Err(ErrorCode::InvalidIndex.into());
            }
        } else {
            if let Some(stake) = counters.stake_deposits.get_mut(index) {
                *stake = unix_timestamp;
            } else {
                return Err(ErrorCode::InvalidIndex.into());
            }
        }
    
        // Reduce current stakers if applicable
        if let Some(first_deposit) = total_deposits.get(0) {
            if *first_deposit == 0 && iter_all_eq(total_deposits).is_some() {
                ctx.accounts.vault.current_stakers = ctx
                    .accounts
                    .vault
                    .current_stakers
                    .checked_sub(1)
                    .ok_or(ErrorCode::Overflow)?;
            }
        }
    
        // Initialize withdrawal amount
        let mut withdraw_amount = *amount;
    
        // Check if sufficient time has elapsed to calculate rewards
        let hours_elapsed: u64 = time_elapsed
            .checked_div(MILLISECONDS_IN_SECOND)
            .and_then(|ms| ms.checked_div(SECONDS_IN_MINUTE))
            .and_then(|min| min.checked_div(MINUTES_IN_HOUR))
            .ok_or(ErrorCode::Overflow)?;
    
        if hours_elapsed >= ctx.accounts.vault.base_hour.try_into().map_err(|_| ErrorCode::Overflow)? {
            // Safely access `vault` fields and ensure no division by zero
            let start_pool_f64: f64 = ctx.accounts.vault.start_pool
                .try_into()
                .map_err(|_| ErrorCode::Overflow)?;
    
            if start_pool_f64 == 0.0 {
                return Err(ErrorCode::DivisionByZero.into());
            }
    
            let amount_f64: f64 = ctx.accounts.vault.amount
                .try_into()
                .map_err(|_| ErrorCode::Overflow)?;
    
            let pool_percent = amount_f64 / start_pool_f64;
    
            let base_hour_adjuster: u64 = ctx.accounts
                .vault
                .base_hour
                .try_into()
                .map_err(|_| ErrorCode::Overflow)?;
    
            // Safely adjust held hours
            let held_hours = hours_elapsed.min(MAX_HELD_HOURS);
            let remainder = held_hours % base_hour_adjuster;
    
            let multi: u64 = if remainder == 0 {
                held_hours.checked_div(base_hour_adjuster).ok_or(ErrorCode::Overflow)?
            } else {
                (held_hours.checked_sub(remainder).ok_or(ErrorCode::Overflow)?)
                    .checked_div(base_hour_adjuster)
                    .ok_or(ErrorCode::Overflow)?
            };
    
            let base_rate_f32: f32 = ctx.accounts.vault.base_rate
                .try_into()
                .map_err(|_| ErrorCode::Overflow)?;
            let final_multi: f32 = (multi.try_into().map_err(|_| ErrorCode::Overflow)? * base_rate_f32)
                .checked_mul(pool_percent as f32)
                .ok_or(ErrorCode::Overflow)?;
    
            // Safely calculate reward tokens as u64
            let reward_tokens: u64 = (amount_f64 * (final_multi / divisor))
                .floor()
                .try_into()
                .map_err(|_| ErrorCode::Overflow)?;
    
            ctx.accounts.vault.amount = ctx
                .accounts
                .vault
                .amount
                .checked_sub(reward_tokens)
                .ok_or(ErrorCode::Overflow)?;
    
            withdraw_amount = if reward_only {
                reward_tokens
            } else {
                withdraw_amount.checked_add(reward_tokens).ok_or(ErrorCode::Overflow)?
            };
        }
    
        // Safely update vault staked amount if not reward-only
        if !reward_only {
            ctx.accounts.vault.amount_staked = ctx
                .accounts.vault.amount_staked
                .checked_sub(*amount)
                .ok_or(ErrorCode::Overflow)?;
        } else if withdraw_amount == *amount {
            withdraw_amount = 0;
        }
    
        // Safely transfer tokens from vault to withdrawer
        if withdraw_amount > 0 {
            let cpi_accounts = Transfer {
                from: ctx.accounts.vault_token_account.to_account_info(),
                to: ctx.accounts.withdrawer_token_account.to_account_info(),
                authority: ctx.accounts.vault_token_account.to_account_info(),
            };
    
            let cpi_program = ctx.accounts.token_program.to_account_info();
            transfer(CpiContext::new(cpi_program, cpi_accounts), withdraw_amount)?;
        }
    
        Ok(())
    }
    
       
}

#[derive(Accounts)]
pub struct CreateVault<'info> {
    #[account(init, payer = creator, space = 8 + 8 * 2 + 32 * 4 + 8, seeds = [VAULT_SEED, mint.key().as_ref()], bump)]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(
        init,
        payer = creator,
        token::mint = mint,
        token::authority = token_account,
        token::token_program = token_program,
        seeds = [TOKEN_VAULT_SEED, mint.key().as_ref()],
        bump
    )]
    pub token_account: Account<'info, TokenAccount>,
    #[account(mut, token::authority = creator.key(), token::mint = mint.key())]
    pub creator_token_account: Account<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut, seeds = [VAULT_SEED, mint.key().as_ref()], bump)]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub depositor: Signer<'info>,
    #[account(mut, token::authority = depositor.key(), token::mint = mint.key())]
    pub depositor_token_account: Account<'info, TokenAccount>,
    #[account(mut, token::mint = mint,
        token::authority = vault_token_account,
        token::token_program = token_program,
        seeds = [TOKEN_VAULT_SEED, mint.key().as_ref()], bump)]
    pub vault_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(init_if_needed, space = 80 + 8 + 5 * 8, seeds=[INTERACTOR_SEED, depositor.key().as_ref(), mint.key().as_ref()], bump, payer = depositor)]
    pub user_interactions_counter: Account<'info, UserInteractions>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,

}
#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut, seeds = [VAULT_SEED, mint.key().as_ref()], bump)]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub withdrawer: Signer<'info>,
    #[account(mut, token::authority = withdrawer.key(), token::mint = mint.key())]
    pub withdrawer_token_account: Account<'info, TokenAccount>,
    #[account(mut, seeds = [TOKEN_VAULT_SEED, mint.key().as_ref()], bump)]
    pub vault_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        seeds = [INTERACTOR_SEED, withdrawer.key().as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub user_interactions_counter: Account<'info, UserInteractions>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,

}

#[account]
pub struct Vault {
    pub amount: u64,
    pub amount_staked: u64,
    pub start_pool: u64,
    pub base_rate: f32,
    pub base_hour: u32,
    pub total_stakers: u64,
    pub current_stakers: u64
}

#[account]
pub struct UserInteractions {
    total_deposits: [u64; 5],
    time_deposits: [u64; 5],
    stake_deposits: [u64; 5]
}

#[error_code]
pub enum ErrorCode {
    #[msg("Overflow error")]
    Overflow,
}