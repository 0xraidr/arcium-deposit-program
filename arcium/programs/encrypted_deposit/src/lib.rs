use anchor_lang::prelude::*;
use arcium_anchor::prelude::*;
use arcium_client::idl::arcium::types::{CallbackAccount, CircuitSource, OffChainCircuitSource};
use arcium_macros::circuit_hash;

mod shard_cpi;

use shard_cpi::{
    invoke_record_encrypted_deposit, invoke_update_faction_sol, RecordEncryptedDepositCommitmentArgs,
    UpdateFactionSolFromArciumArgs, SHARD_PROGRAM_ID,
};

const COMP_DEF_OFFSET_DEPOSIT: u32 = comp_def_offset("deposit");

/// Number of factions in the game.
pub const FACTION_COUNT: usize = 6;

/// Max distinct faction legs per deposit (matches shard `MAX_FACTIONS_PER_USER_PER_ROUND`).
pub const MAX_DEPOSIT_LEGS: usize = 3;

/// Padding faction for unused encrypted leg slots.
pub const INACTIVE_FACTION: u8 = 255;

/// Shard program program ID — update this to match your deployed shard program.
pub const SHARD_PROGRAM_ID_STR: &str = "CjBEGXTxN4cMrvNJW6XkLM424pJ6g5aMNfWagDYwtPZy";

/// Max length for an off-chain circuit URL passed to [`init_deposit_round_comp_def`].
pub const MAX_CIRCUIT_URL_LEN: usize = 200;

declare_id!("D29HHVZiZWc1cbR8nDnjfTg8CHEJmLrau3NbcnteFzi");

#[arcium_program]
pub mod encrypted_deposit {
    use super::*;

    /// Initializes the computation definition for the deposit circuit.
    /// Must be called once before submitting any deposits.
    ///
    /// For devnet / large circuits: pass `use_offchain_circuit = true` and a public HTTPS URL to
    /// `deposit.arcis` (see `DEPOSIT_CIRCUIT_URL` in devnet tests). Localnet uses on-chain upload
    /// (`use_offchain_circuit = false`, empty `circuit_url`).
    pub fn init_deposit_round_comp_def(
        ctx: Context<InitDepositCompDef>,
        use_offchain_circuit: bool,
        circuit_url: String,
    ) -> Result<()> {
        let circuit_source = if use_offchain_circuit {
            require!(!circuit_url.is_empty(), ErrorCode::CircuitUrlRequired);
            require!(
                circuit_url.len() <= MAX_CIRCUIT_URL_LEN,
                ErrorCode::CircuitUrlTooLong
            );
            Some(CircuitSource::OffChain(OffChainCircuitSource {
                source: circuit_url,
                hash: circuit_hash!("deposit"),
            }))
        } else {
            None
        };
        init_computation_def(ctx.accounts, circuit_source)?;
        Ok(())
    }

    /// Submits an encrypted multi-leg deposit (up to [`MAX_DEPOSIT_LEGS`] factions) for one MPC job.
    ///
    /// SOL is routed to the shard vault and ciphertext is stored on [`UserState`] before MPC runs.
    /// Unused leg slots must encrypt `(INACTIVE_FACTION, 0)`. `leg_count_ciphertext` encrypts the
    /// number of active legs (1–3); remaining slots must be inactive padding.
    pub fn deposit(
        ctx: Context<Deposit>,
        computation_offset: u64,
        round_id: u64,
        net_lamports: u64,
        pending_gross_lamports: u64,
        pub_key: [u8; 32],
        nonce: u128,
        leg_count_ciphertext: [u8; 32],
        leg0_faction_ciphertext: [u8; 32],
        leg0_amount_ciphertext: [u8; 32],
        leg1_faction_ciphertext: [u8; 32],
        leg1_amount_ciphertext: [u8; 32],
        leg2_faction_ciphertext: [u8; 32],
        leg2_amount_ciphertext: [u8; 32],
        commitment_hash: [u8; 32],
    ) -> Result<()> {
        require!(net_lamports > 0, ErrorCode::ZeroDepositAmount);
        require_keys_eq!(ctx.accounts.shard_program.key(), SHARD_PROGRAM_ID);

        let record_args = RecordEncryptedDepositCommitmentArgs {
            computation_offset,
            net_lamports,
            pending_gross_lamports,
            encryption_nonce: nonce,
            encryption_pubkey: pub_key,
            leg_count_ciphertext,
            leg0_faction_ciphertext,
            leg0_amount_ciphertext,
            leg1_faction_ciphertext,
            leg1_amount_ciphertext,
            leg2_faction_ciphertext,
            leg2_amount_ciphertext,
            commitment_hash,
        };

        invoke_record_encrypted_deposit(
            &ctx.accounts.shard_program.to_account_info(),
            &ctx.accounts.payer.to_account_info(),
            &ctx.accounts.user_state.to_account_info(),
            &ctx.accounts.global_config.to_account_info(),
            &ctx.accounts.mint_emission.to_account_info(),
            &ctx.accounts.active_round_state.to_account_info(),
            &ctx.accounts.sol_deposit_vault.to_account_info(),
            &ctx.accounts.instructions_sysvar.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &ctx.accounts.deposit_receipt.to_account_info(),
            ctx.accounts.prior_round_state.as_ref().map(|a| a.to_account_info()).as_ref(),
            ctx.accounts.protocol_fee_vault.as_ref().map(|a| a.to_account_info()).as_ref(),
            ctx.accounts.withdrawal_fee_pool.as_ref().map(|a| a.to_account_info()).as_ref(),
            &record_args,
        )?;

        let args = ArgBuilder::new()
            .x25519_pubkey(pub_key)
            .plaintext_u128(nonce)
            .encrypted_u8(leg_count_ciphertext)
            .encrypted_u8(leg0_faction_ciphertext)
            .encrypted_u64(leg0_amount_ciphertext)
            .encrypted_u8(leg1_faction_ciphertext)
            .encrypted_u64(leg1_amount_ciphertext)
            .encrypted_u8(leg2_faction_ciphertext)
            .encrypted_u64(leg2_amount_ciphertext)
            .build();

        ctx.accounts.sign_pda_account.bump = ctx.bumps.sign_pda_account;

        queue_computation(
            ctx.accounts,
            computation_offset,
            args,
            vec![DepositCallback::callback_ix(
                computation_offset,
                &ctx.accounts.mxe_account,
                &[
                    CallbackAccount {
                        pubkey: ctx.accounts.sign_pda_account.key(),
                        is_writable: false,
                    },
                    CallbackAccount {
                        pubkey: ctx.accounts.active_round_state.key(),
                        is_writable: true,
                    },
                    CallbackAccount {
                        pubkey: ctx.accounts.shard_program.key(),
                        is_writable: false,
                    },
                    CallbackAccount {
                        pubkey: ctx.accounts.global_config.key(),
                        is_writable: false,
                    },
                    CallbackAccount {
                        pubkey: ctx.accounts.deposit_receipt.key(),
                        is_writable: true,
                    },
                ],
            )?],
            1,
            0,
        )?;

        let _ = round_id;
        Ok(())
    }

    /// Callback for the deposit MPC computation.
    #[arcium_callback(encrypted_ix = "deposit")]
    pub fn deposit_callback(
        ctx: Context<DepositCallback>,
        output: SignedComputationOutputs<DepositOutput>,
    ) -> Result<()> {
        let revealed_legs = match output.verify_output(
            &ctx.accounts.cluster_account,
            &ctx.accounts.computation_account,
        ) {
            Ok(DepositOutput { field_0 }) => field_0,
            Err(e) => {
                msg!("verify_output failed: {}", e);
                return Err(ErrorCode::AbortedComputation.into());
            }
        };

        let (leg_count, faction_indices, amounts) = parse_and_validate_legs(&revealed_legs)?;

        require_keys_eq!(ctx.accounts.shard_program.key(), SHARD_PROGRAM_ID);

        let round_id = {
            let round_data = ctx.accounts.round_state.try_borrow_data()?;
            require!(round_data.len() >= 10, ErrorCode::InvalidShardRoundState);
            u64::from_le_bytes(round_data[2..10].try_into().unwrap())
        };

        let update_args = UpdateFactionSolFromArciumArgs {
            round_id,
            leg_count,
            faction_indices,
            amounts,
        };

        invoke_update_faction_sol(
            &ctx.accounts.shard_program.to_account_info(),
            &ctx.accounts.sign_pda_account.to_account_info(),
            ctx.accounts.sign_pda_account.bump,
            &ctx.accounts.round_state.to_account_info(),
            &ctx.accounts.global_config.to_account_info(),
            &ctx.accounts.deposit_receipt.to_account_info(),
            &update_args,
        )?;

        Ok(())
    }
}

/// Parse MPC output and enforce padding, factions, and duplicate rules.
fn parse_and_validate_legs(
    revealed: &[DepositOutputStruct0; MAX_DEPOSIT_LEGS],
) -> Result<(u8, [u8; MAX_DEPOSIT_LEGS], [u64; MAX_DEPOSIT_LEGS])> {
    let mut faction_indices = [INACTIVE_FACTION; MAX_DEPOSIT_LEGS];
    let mut amounts = [0u64; MAX_DEPOSIT_LEGS];
    let mut seen = [false; FACTION_COUNT];
    let mut active_count = 0usize;

    for leg in revealed.iter() {
        let faction = leg.field_0;
        let amount = leg.field_1;

        if amount == 0 {
            if faction != INACTIVE_FACTION {
                return Err(ErrorCode::InvalidPaddingLeg.into());
            }
            continue;
        }

        if faction as usize >= FACTION_COUNT {
            return Err(ErrorCode::InvalidFactionIndex.into());
        }
        if seen[faction as usize] {
            return Err(ErrorCode::DuplicateFactionLeg.into());
        }
        if active_count >= MAX_DEPOSIT_LEGS {
            return Err(ErrorCode::TooManyLegs.into());
        }

        seen[faction as usize] = true;
        faction_indices[active_count] = faction;
        amounts[active_count] = amount;
        active_count += 1;
    }

    if active_count == 0 {
        return Err(ErrorCode::EmptyDepositLegs.into());
    }

    Ok((active_count as u8, faction_indices, amounts))
}

fn shard_arcium_deposit_receipt_pda(
    shard_program: &Pubkey,
    round_id: u64,
    computation_offset: u64,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            b"arcium_deposit",
            &round_id.to_le_bytes(),
            &computation_offset.to_le_bytes(),
        ],
        shard_program,
    )
    .0
}

fn shard_user_state_pda(user: &Pubkey, shard_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"user_state", user.as_ref()], shard_program).0
}

fn shard_global_config_pda(shard_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"global_config"], shard_program).0
}

fn shard_mint_emission_pda(shard_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"mint_emission"], shard_program).0
}

fn shard_round_pda(shard_program: &Pubkey, round_id: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[b"round", &round_id.to_le_bytes()],
        shard_program,
    )
    .0
}

fn shard_sol_deposit_vault_pda(shard_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"sol_deposit_vault"], shard_program).0
}

#[init_computation_definition_accounts("deposit", payer)]
#[derive(Accounts)]
pub struct InitDepositCompDef<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        address = derive_mxe_pda!()
    )]
    pub mxe_account: Box<Account<'info, MXEAccount>>,
    #[account(mut)]
    /// CHECK: comp_def_account, checked by arcium program.
    pub comp_def_account: UncheckedAccount<'info>,
    #[account(mut, address = derive_mxe_lut_pda!(mxe_account.lut_offset_slot))]
    /// CHECK: address_lookup_table, checked by arcium program.
    pub address_lookup_table: UncheckedAccount<'info>,
    #[account(address = LUT_PROGRAM_ID)]
    /// CHECK: lut_program is the Address Lookup Table program.
    pub lut_program: UncheckedAccount<'info>,
    pub arcium_program: Program<'info, Arcium>,
    pub system_program: Program<'info, System>,
}

#[queue_computation_accounts("deposit", payer)]
#[derive(Accounts)]
#[instruction(computation_offset: u64, round_id: u64, net_lamports: u64, pending_gross_lamports: u64, commitment_hash: [u8; 32])]
pub struct Deposit<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init_if_needed,
        space = 9,
        payer = payer,
        seeds = [&SIGN_PDA_SEED],
        bump,
        address = derive_sign_pda!(),
    )]
    pub sign_pda_account: Account<'info, ArciumSignerAccount>,

    #[account(
        address = derive_mxe_pda!()
    )]
    pub mxe_account: Box<Account<'info, MXEAccount>>,

    #[account(
        mut,
        address = derive_mempool_pda!(mxe_account)
    )]
    /// CHECK: mempool_account, checked by the arcium program
    pub mempool_account: UncheckedAccount<'info>,

    #[account(
        mut,
        address = derive_execpool_pda!(mxe_account)
    )]
    /// CHECK: executing_pool, checked by the arcium program
    pub executing_pool: UncheckedAccount<'info>,

    #[account(
        mut,
        address = derive_comp_pda!(computation_offset, mxe_account)
    )]
    /// CHECK: computation_account, checked by the arcium program.
    pub computation_account: UncheckedAccount<'info>,

    #[account(
        address = derive_comp_def_pda!(COMP_DEF_OFFSET_DEPOSIT)
    )]
    pub comp_def_account: Box<Account<'info, ComputationDefinitionAccount>>,

    #[account(
        mut,
        address = derive_cluster_pda!(mxe_account)
    )]
    pub cluster_account: Box<Account<'info, Cluster>>,

    #[account(
        mut,
        address = ARCIUM_FEE_POOL_ACCOUNT_ADDRESS,
    )]
    pub pool_account: Account<'info, FeePool>,

    #[account(
        mut,
        address = ARCIUM_CLOCK_ACCOUNT_ADDRESS,
    )]
    pub clock_account: Account<'info, ClockAccount>,

    /// CHECK: shard program id
    #[account(address = SHARD_PROGRAM_ID)]
    pub shard_program: UncheckedAccount<'info>,

    /// CHECK: shard user_state PDA validated against payer + shard_program
    #[account(
        mut,
        constraint = user_state.key() == shard_user_state_pda(payer.key, &shard_program.key()) @ ErrorCode::InvalidShardUserState
    )]
    pub user_state: UncheckedAccount<'info>,

    /// CHECK: shard global_config PDA validated against shard_program
    #[account(
        constraint = global_config.key() == shard_global_config_pda(&shard_program.key()) @ ErrorCode::InvalidShardGlobalConfig
    )]
    pub global_config: UncheckedAccount<'info>,

    /// CHECK: shard mint_emission PDA validated against shard_program
    #[account(
        constraint = mint_emission.key() == shard_mint_emission_pda(&shard_program.key()) @ ErrorCode::InvalidShardMintEmission
    )]
    pub mint_emission: UncheckedAccount<'info>,

    /// CHECK: shard active round PDA validated against round_id + shard_program
    #[account(
        mut,
        constraint = active_round_state.key() == shard_round_pda(&shard_program.key(), round_id) @ ErrorCode::InvalidShardRoundState
    )]
    pub active_round_state: UncheckedAccount<'info>,

    /// CHECK: shard sol_deposit_vault PDA validated against shard_program
    #[account(
        mut,
        constraint = sol_deposit_vault.key() == shard_sol_deposit_vault_pda(&shard_program.key()) @ ErrorCode::InvalidShardSolVault
    )]
    pub sol_deposit_vault: UncheckedAccount<'info>,

    /// CHECK: shard deposit receipt PDA validated against round_id + computation_offset
    #[account(
        mut,
        constraint = deposit_receipt.key() == shard_arcium_deposit_receipt_pda(&shard_program.key(), round_id, computation_offset)
            @ ErrorCode::InvalidShardDepositReceipt
    )]
    pub deposit_receipt: UncheckedAccount<'info>,

    #[account(address = ::arcium_anchor::solana_instructions_sysvar::ID)]
    /// CHECK: instructions sysvar
    pub instructions_sysvar: UncheckedAccount<'info>,

    /// CHECK: optional prior round when switching rounds
    #[account(mut)]
    pub prior_round_state: Option<UncheckedAccount<'info>>,

    /// CHECK: optional protocol fee vault when protocol_fee_bps > 0
    #[account(mut)]
    pub protocol_fee_vault: Option<UncheckedAccount<'info>>,

    /// CHECK: optional withdrawal fee pool
    #[account(mut)]
    pub withdrawal_fee_pool: Option<UncheckedAccount<'info>>,

    pub system_program: Program<'info, System>,
    pub arcium_program: Program<'info, Arcium>,
}

#[callback_accounts("deposit")]
#[derive(Accounts)]
pub struct DepositCallback<'info> {
    pub arcium_program: Program<'info, Arcium>,

    #[account(
        address = derive_comp_def_pda!(COMP_DEF_OFFSET_DEPOSIT)
    )]
    pub comp_def_account: Account<'info, ComputationDefinitionAccount>,

    #[account(
        address = derive_mxe_pda!()
    )]
    pub mxe_account: Account<'info, MXEAccount>,

    /// CHECK: computation_account, checked by arcium program via constraints in the callback context.
    pub computation_account: UncheckedAccount<'info>,

    #[account(
        address = derive_cluster_pda!(mxe_account)
    )]
    pub cluster_account: Account<'info, Cluster>,

    #[account(address = ::arcium_anchor::solana_instructions_sysvar::ID)]
    /// CHECK: instructions_sysvar, checked by the account constraint
    pub instructions_sysvar: UncheckedAccount<'info>,

    #[account(
        seeds = [&SIGN_PDA_SEED],
        bump,
        address = derive_sign_pda!(),
    )]
    pub sign_pda_account: Account<'info, ArciumSignerAccount>,

    /// CHECK: shard round state passed through callback accounts
    #[account(mut)]
    pub round_state: UncheckedAccount<'info>,

    /// CHECK: shard program id
    #[account(address = SHARD_PROGRAM_ID)]
    pub shard_program: UncheckedAccount<'info>,

    /// CHECK: shard global config for pause guard on faction updates
    pub global_config: UncheckedAccount<'info>,

    /// CHECK: shard deposit receipt — caps callback net SOL; no wallet linkage
    #[account(mut)]
    pub deposit_receipt: UncheckedAccount<'info>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("The computation was aborted")]
    AbortedComputation,
    #[msg("Invalid faction index — must be 0–5")]
    InvalidFactionIndex,
    #[msg("Deposit must include at least one leg")]
    EmptyDepositLegs,
    #[msg("Too many active deposit legs")]
    TooManyLegs,
    #[msg("Duplicate faction in deposit legs")]
    DuplicateFactionLeg,
    #[msg("Inactive leg slot must use INACTIVE_FACTION and zero amount")]
    InvalidPaddingLeg,
    #[msg("Net deposit amount must be greater than zero")]
    ZeroDepositAmount,
    #[msg("Invalid shard user_state PDA")]
    InvalidShardUserState,
    #[msg("Invalid shard global_config PDA")]
    InvalidShardGlobalConfig,
    #[msg("Invalid shard mint_emission PDA")]
    InvalidShardMintEmission,
    #[msg("Invalid shard round_state PDA")]
    InvalidShardRoundState,
    #[msg("Invalid shard sol_deposit_vault PDA")]
    InvalidShardSolVault,
    #[msg("Invalid shard arcium deposit receipt PDA")]
    InvalidShardDepositReceipt,
    #[msg("Circuit URL required when use_offchain_circuit is true")]
    CircuitUrlRequired,
    #[msg("Circuit URL exceeds MAX_CIRCUIT_URL_LEN")]
    CircuitUrlTooLong,
}
