use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_spl::{token::{self, Mint, Token, TokenAccount, Transfer}};

use crate::ConfigAccount;

pub fn stake_tokens(ctx: Context<Stake>, amount: u64, lockup: u64) -> Result<()> {
    let staking_pool = &mut ctx.accounts.staking_pool;
    let user = &mut ctx.accounts.user;
    let user_stake = &mut ctx.accounts.user_stake;

    require!(
        lockup == 30 || lockup == 90 || lockup == 180,
        StakingError::WrongStakingPackage
    );
    
    require!(user_stake.amount == 0, StakingError::AlreadyStaked);
    require!(!user_stake.unstaked, StakingError::AlreadyUnStaked);
    let clock = Clock::get()?;
    // Transfer tokens from user to pool
    let cpi_accounts = Transfer {
        from: ctx.accounts.user_token_account.to_account_info(),
        to: ctx.accounts.pool_token_account.to_account_info(),
        authority: ctx.accounts.user_authority.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
    token::transfer(cpi_ctx, amount)?;

   
    // update user stake entry
    user_stake.stake_id = user.stake_count;
    user_stake.amount = amount;
    user_stake.staked_at = clock.unix_timestamp;
    user_stake.lockup = lockup;
    // update staking pool state
    staking_pool.total_staked += amount;

    // Update user staking info
    user.authority = ctx.accounts.user_authority.key();
    user.staking_pool = staking_pool.key();
    user.bump = ctx.bumps.user;
    if user.total_amount == 0 {
        user.first_staked_at = clock.unix_timestamp;
    }
    user.total_amount += amount;
    user.stake_count += 1;

    msg!("Staked {} tokens", amount);
    Ok(())
}

pub fn request_unstake_tokens(ctx: Context<UnstakeRequest>,_index:u64) -> Result<()> {
    let user_stake = &mut ctx.accounts.user_stake;
  
    require!(user_stake.amount > 0, StakingError::NoStakedTokens);
    require!(user_stake.unstake_requested_at == 0, StakingError::UnstakeAlreadyRequested);
    require!(!user_stake.unstaked, StakingError::AlreadyUnStaked);

    user_stake.unstake_requested_at = Clock::get()?.unix_timestamp;
    msg!(
        "Unstake requested for {} tokens at {}",
        user_stake.amount,
        user_stake.unstake_requested_at
    );
    Ok(())
}

pub fn unstake_tokens(ctx: Context<Unstake>,_index:u64) -> Result<()> {
    let user = &mut ctx.accounts.user;
    let user_stake = &mut ctx.accounts.user_stake;
    require!(!user_stake.unstaked, StakingError::AlreadyUnStaked);

    let staking_pool = &ctx.accounts.staking_pool;
    let clock = Clock::get()?;
    
    require!(user_stake.unstake_requested_at != 0, StakingError::RequestUnstakeFirst);
   
    // require!(clock.unix_timestamp >= user.unstake_requested_at + 172800 ,StakingError::WaitFor48Hours); // 48 hours in seconds
    require!(clock.unix_timestamp >= user_stake.unstake_requested_at + 180 ,StakingError::WaitFor48Hours); // 3 mint in seconds
  

    // Check if user has staked tokens
    require!(user_stake.amount > 0, StakingError::NoStakedTokens);

    let min_staking_duration: i64 = (user_stake.lockup * 24 * 60 * 60).try_into().unwrap();
    let staking_duration = clock.unix_timestamp.saturating_sub(user_stake.staked_at);

    let mut fee = 0;
    if staking_duration < min_staking_duration {
        fee = (user_stake.amount * 10) / 100;
    }

    let amount_to_return = user_stake.amount - fee;

    // Create PDA signer seeds
    let pool_seeds = &[
        b"staking_pool".as_ref(),
        staking_pool.token_mint.as_ref(),
        &[staking_pool.bump],
    ];

    let signer = &[&pool_seeds[..]];
    // Transfer staked tokens back to user

    let cpi_accounts = Transfer {
        from: ctx.accounts.pool_token_account.to_account_info(),
        to: ctx.accounts.user_token_account.to_account_info(),
        authority: staking_pool.to_account_info(),
    };

    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
    token::transfer(cpi_ctx, amount_to_return)?;

    // send penalty fee to staking reward account ata
    if fee != 0 {
       let cpi_accounts = Transfer {
            from: ctx.accounts.pool_token_account.to_account_info(),
            to: ctx.accounts.staking_reward_ata.to_account_info(),
            authority: staking_pool.to_account_info(),
        };

        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::transfer(cpi_ctx, fee)?;
    }


    // let amount = user.amount;
    
    user_stake.unstaked = true;
    user.total_amount -= user_stake.amount;
    let staking_pool = &mut ctx.accounts.staking_pool;
    staking_pool.total_staked -= user_stake.amount;
    
    user_stake.amount = 0;
    user_stake.staked_at = 0;
    user_stake.unstake_requested_at = 0;
    msg!("Unstaked {} tokens", amount_to_return);
    Ok(())
}



pub fn claim_reward(ctx: Context<ClaimReward>,_index:u64) -> Result<()> {
        let staking_pool = &mut ctx.accounts.staking_pool;

    let user = &mut ctx.accounts.user;
    let user_stake = &mut ctx.accounts.user_stake;
    
    require!(user_stake.amount > 0, StakingError::NoStakedTokens);
    let clock = Clock::get()?;
    let min_staking_duration = user_stake.lockup * 24 * 60 * 60; // days in seconds

    // Calculate staking duration
    let staking_duration: i64 = clock
        .unix_timestamp
        .saturating_sub(user_stake.staked_at)
        .try_into()
        .unwrap();
    require!(staking_pool.total_staked > 0, StakingError::NoStakedTokens);

    let periods = staking_duration as u64 / min_staking_duration;
    require!(periods > user_stake.current_period, StakingError::StakingPeriodNotMet);
    let elapsed_time;
    if user_stake.last_claimed_at == 0{
        elapsed_time = clock.unix_timestamp.saturating_sub(user_stake.staked_at);
    }else{
        elapsed_time = clock.unix_timestamp.saturating_sub(user_stake.last_claimed_at);
    }
   // Calculate elapsed time since last reward claim
    require!(elapsed_time > 0, StakingError::StakingPeriodNotMet);
    let reward = (user_stake.amount) * (staking_pool.rate_per_second) * (elapsed_time as u64) * periods;




    let reward_amount = reward
    .try_into()
    .map_err(|_| error!(StakingError::RewardOverflow))?;
    
    user_stake.current_period = periods;

    // Create PDA signer seeds
    let pool_seeds = &[
        b"staking_pool".as_ref(),
        ctx.accounts.staking_pool.token_mint.as_ref(),
        &[ctx.accounts.staking_pool.bump],
    ];
    let signer = &[&pool_seeds[..]];
    // Transfer reward tokens to user
    let cpi_accounts = Transfer {
        from: ctx.accounts.staking_reward_ata.to_account_info(),
        to: ctx.accounts.user_token_account.to_account_info(),
        authority: ctx.accounts.staking_pool.to_account_info(),
    };

    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
    token::transfer(cpi_ctx, reward_amount)?;

    ctx.accounts.staking_pool.reward_issued += reward_amount as i64;
    user.reward_issued += reward_amount as i64;
    user_stake.last_claimed_at = clock.unix_timestamp;  
    msg!("Claimed reward of {} tokens", reward_amount);
    Ok(())
}



#[derive(Accounts)]
pub struct StakeInitialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<StakingPool>(),
        seeds = [b"staking_pool".as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub staking_pool: Account<'info, StakingPool>,
    #[account(
        init,
        payer = authority,
        space = 8,
        seeds = [b"staking_reward".as_ref()],
        bump
    )]
    pub staking_reward: Account<'info, StakingRewards>,

    #[account(mut)]
    pub authority: Signer<'info>,

    // Modified to remove the constraint for the specific mint address
    pub token_mint: Account<'info, Mint>,
    pub pool_token_account: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(
        mut,
        seeds=[b"config".as_ref()],
        bump
    )]
    pub config_account: Account<'info, ConfigAccount>,
    #[account(
        mut,
        seeds = [b"staking_pool".as_ref(), staking_pool.token_mint.as_ref()],
        bump = staking_pool.bump,
    )]
    pub staking_pool: Account<'info, StakingPool>,

    #[account(
        init_if_needed,
        payer = user_authority,
        space = 8 + std::mem::size_of::<UserStakeInfo>(),
        seeds = [b"user".as_ref(), user_authority.key().as_ref()],
        bump
    )]
    pub user: Account<'info, UserStakeInfo>,
    #[account(
        init,
        payer = user_authority,
        space = 8 + std::mem::size_of::<UserStakesEntry>(),
        seeds = [b"user_stake".as_ref(), user_authority.key().as_ref(),user.stake_count.to_le_bytes().as_ref()],
        bump
    )]
    pub user_stake: Account<'info, UserStakesEntry>,

    #[account(mut)]
    pub user_authority: Signer<'info>,

    #[account(
        mut,
        constraint = user_token_account.mint == staking_pool.token_mint,
        constraint = user_token_account.owner == user_authority.key()
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut,
    constraint = pool_token_account.key() == staking_pool.pool_token_account
    )]
    pub pool_token_account: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(index:u64)]
pub struct Unstake<'info> {
    #[account(
        mut,
        seeds=[b"config".as_ref()],
        bump
    )]
    pub config_account: Account<'info, ConfigAccount>,
    #[account(
        mut,
        seeds = [b"staking_pool".as_ref(), staking_pool.token_mint.as_ref()],
        bump = staking_pool.bump,
    )]
    pub staking_pool: Account<'info, StakingPool>,

    #[account(
        mut,
        seeds = [b"user".as_ref(),  user_authority.key().as_ref()],
        bump = user.bump,
        constraint = user.authority == user_authority.key(),
        constraint = user.staking_pool == staking_pool.key()
    )]
    pub user: Account<'info, UserStakeInfo>,
  #[account(
        seeds = [b"user_stake".as_ref(), user_authority.key().as_ref(),index.to_le_bytes().as_ref()],
        bump
    )]
    pub user_stake: Account<'info, UserStakesEntry>,
    #[account(mut)]
    pub user_authority: Signer<'info>,
    #[account(
        mut,
        constraint = staking_reward_ata.owner == staking_pool.staking_reward_account
    )]
    pub staking_reward_ata: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = user_token_account.mint == staking_pool.token_mint,
        constraint = user_token_account.owner == user_authority.key()
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = pool_token_account.key() == staking_pool.pool_token_account
    )]
    pub pool_token_account: Account<'info, TokenAccount>,

 

    pub token_program: Program<'info, Token>,
}



#[derive(Accounts)]
#[instruction(index:u64)]
pub struct UnstakeRequest<'info> {
        #[account(
            mut,
            seeds=[b"config".as_ref()],
            bump
        )]
    pub config_account: Account<'info, ConfigAccount>,
    #[account(
        mut,
        seeds = [b"staking_pool".as_ref(), staking_pool.token_mint.as_ref()],
        bump = staking_pool.bump,
    )]
    pub staking_pool: Account<'info, StakingPool>,
  #[account(
    mut,
        seeds = [b"user_stake".as_ref(), user_authority.key().as_ref(),index.to_le_bytes().as_ref()],
        bump
    )]
    pub user_stake: Account<'info, UserStakesEntry>,
    #[account(
        mut,
        seeds = [b"user".as_ref(), user_authority.key().as_ref()],
        bump = user.bump,
        constraint = user.authority == user_authority.key(),
        constraint = user.staking_pool == staking_pool.key()
    )]
    pub user: Account<'info, UserStakeInfo>,

    #[account(mut)]
    pub user_authority: Signer<'info>,
}


#[derive(Accounts)]
#[instruction(index:u64)]
pub struct ClaimReward<'info> {
          #[account(
            mut,
            seeds=[b"config".as_ref()],
            bump
        )]
    pub config_account: Account<'info, ConfigAccount>,
    #[account(
        seeds = [b"staking_pool".as_ref(), staking_pool.token_mint.as_ref()],
        bump = staking_pool.bump,
    )]
    pub staking_pool: Account<'info, StakingPool>,

    #[account(
        mut,
        seeds = [b"user".as_ref(), user_authority.key().as_ref()],
        bump = user.bump,
        constraint = user.authority == user_authority.key(),
        constraint = user.staking_pool == staking_pool.key()
    )]
    pub user: Account<'info, UserStakeInfo>,
  #[account(
        seeds = [b"user_stake".as_ref(), user_authority.key().as_ref(),index.to_le_bytes().as_ref()],
        bump
    )]
    pub user_stake: Account<'info, UserStakesEntry>,
    #[account(mut)]
    pub user_authority: Signer<'info>,

    #[account(
        mut,
        constraint = user_token_account.mint == staking_pool.token_mint,
        constraint = user_token_account.owner == user_authority.key()
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = staking_reward_ata.owner == staking_pool.staking_reward_account
    )]
    pub staking_reward_ata: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[account]
pub struct StakingPool {
    pub authority: Pubkey,
    pub token_mint: Pubkey,
    pub pool_token_account: Pubkey,
    pub staking_reward_account: Pubkey,
    pub total_staked: u64,
    pub reward_issued: i64,
    pub bump: u8,
    pub rate_per_second: u64, // reward rate per second
}

#[account]
pub struct StakingRewards {
}

#[account]
pub struct UserStakeInfo {
    pub authority: Pubkey,
    pub staking_pool: Pubkey,
    pub first_staked_at: i64,

    pub total_amount: u64,
    pub reward_issued: i64,
    pub bump: u8,

    pub stake_count:u64
}


#[account]
pub struct UserStakesEntry {
    pub stake_id:u64,
    pub amount: u64,
    pub staked_at: i64,
    pub lockup: u64,
    pub unstake_requested_at: i64,
    pub last_claimed_at: i64,
    pub current_period:u64,
    pub unstaked: bool,
}


#[error_code]
pub enum StakingError {
    #[msg("User has no staked tokens")]
    NoStakedTokens,
    #[msg("Staking period has not been met yet")]
    StakingPeriodNotMet,
    #[msg("Wrong Staking Package")]
    WrongStakingPackage,
    #[msg("Reward has already been claimed")]
    RewardAlreadyClaimed,
    #[msg("Already Staked")]
    AlreadyStaked,
    #[msg("Already un Staked")]
    AlreadyUnStaked,
    #[msg("Wait For 48 Hours")]
    WaitFor48Hours,
    #[msg("Request Unstake First")]
    RequestUnstakeFirst,
    #[msg("Unstake Already Requested")]
    UnstakeAlreadyRequested,
    #[msg("Reward Overflow")]
    RewardOverflow,
    #[msg("Invalid Stake Id")]
    InvalidStakeId,
}

