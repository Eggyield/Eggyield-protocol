<p align="center">
  <img src="https://i.imgur.com/ueU5wyM.gif" alt="Eggyield Banner">
</p>

<p align="center">
  <a href="https://eggyield.fun/" target="_blank">
    <img src="https://img.shields.io/badge/Website-eggyield.fun-blue?style=for-the-badge" alt="Website">
  </a>
  <a href="https://discord.gg/eggyield" target="_blank">
    <img src="https://img.shields.io/badge/Discord-Join%20Server-7289DA?style=for-the-badge&logo=discord" alt="Discord">
  </a>
  <a href="https://x.com/eggyield" target="_blank">
    <img src="https://img.shields.io/badge/Twitter-Follow-1DA1F2?style=for-the-badge&logo=twitter" alt="Twitter">
  </a>
</p>

# Eggyield

A Solana-based staking protocol for token yield generation with time-based rewards.

## Technical Overview

Eggyield is a token staking protocol built on Solana that allows users to deposit tokens ("eggs") into vaults and earn rewards based on time staked. The protocol implements a dynamic reward system influenced by time held, base rates, and pool utilization.

### Core Features

- **Time-based Yield Generation**: Rewards scale with staking duration
- **Configurable Base Rates**: Customizable reward rates per vault
- **Variable Time Thresholds**: Configurable minimum holding periods for rewards
- **Multiple Deposit Support**: Each user can maintain up to 5 separate deposits
- **Reward-Only Withdrawals**: Option to withdraw only earned rewards while keeping principal staked

## Smart Contract Architecture

The protocol consists of three primary components:

1. **Vault Accounts**: Store staking parameters and token balances
2. **Token Accounts**: Handle SPL token interactions
3. **User Interaction Trackers**: Maintain user deposit history and staking metrics

### Account Structures

#### Vault

The central storage for vault configuration and token accounting:

```rust
pub struct Vault {
    pub amount: u64,              // Total amount of tokens in the vault
    pub amount_staked: u64,       // Amount currently staked by users
    pub start_pool: u64,          // Initial pool size for reward calculations
    pub base_rate: f32,           // Base reward rate multiplier
    pub base_hour: u32,           // Minimum staking hours for reward eligibility
    pub total_stakers: u64,       // All-time stakers count
    pub current_stakers: u64      // Active stakers count
}
```

#### User Interactions

Tracks individual user deposit information:

```rust
pub struct UserInteractions {
    total_deposits: [u64; 5],     // Array of user deposit amounts
    time_deposits: [u64; 5],      // Timestamps of initial deposits
    stake_deposits: [u64; 5]      // Timestamps of stake activation
}
```

### Core Functions

#### Create Egg Vault

Initializes a new vault with specified parameters:

```rust
pub fn create_egg_vault(
    ctx: Context<CreateVault>,
    amount: u64,           // Initial token amount
    base_rate: f32,        // Reward rate multiplier
    base_hour: u32         // Minimum hours before rewards
) -> Result<()>
```

#### Deposit Eggs

Allows users to stake tokens in a vault:

```rust
pub fn deposit_eggs(
    ctx: Context<Deposit>, 
    amount: u64,           // Amount to deposit
    index: usize           // Deposit slot (0-4)
) -> Result<()>
```

#### Withdraw Eggs

Enables withdrawal of staked tokens with accrued rewards:

```rust
pub fn withdraw_eggs(
    ctx: Context<Withdraw>, 
    index: usize,          // Deposit slot to withdraw from
    reward_only: bool      // Whether to withdraw only rewards
) -> Result<()>
```

## Reward Calculation

Rewards are calculated based on several factors:

1. **Time Elapsed**: Hours since deposit (capped at 24 hours maximum)
2. **Base Rate**: Configurable multiplier set during vault creation
3. **Pool Utilization**: Ratio of current pool size to initial pool size

The formula follows:

```
hours_elapsed = min(time_staked_in_hours, MAX_HELD_HOURS)
adjustable_hours = floor(hours_elapsed / base_hour) * base_hour
multiplier = (adjustable_hours / base_hour) * base_rate * (current_pool / start_pool)
reward = deposit_amount * multiplier
```

## Security Considerations

The contract implements several security measures:

- **Overflow Protection**: All arithmetic operations use checked math
- **Index Validation**: Array accesses are bounds-checked
- **PDA Security**: Program-derived addresses for vault and token accounts
- **Seed Separation**: Distinct seeds for different account types

## Usage Flow

1. An admin creates a vault with initial parameters
2. Users deposit tokens into the vault, specifying a deposit slot
3. Deposits accrue rewards based on time staked
4. Users can withdraw their principal + rewards or only rewards

## Technical Dependencies

- `anchor_lang`: Core Anchor framework
- `anchor_spl`: SPL token program interfaces
- `solana_program`: Native Solana program utilities
- `borsh`: Binary serialization/deserialization

## Error Handling

The contract defines custom error types to handle various failure conditions, particularly focused on arithmetic overflow protection to ensure safe operation even with large token amounts or extended staking periods.

## Limitations

- Maximum of 5 deposits per user per vault
- 24-hour cap on reward calculation
- Rewards are based on discrete time intervals rather than continuous accrual
