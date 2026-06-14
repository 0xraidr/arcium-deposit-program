use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
    program::invoke_signed,
};
use borsh::{BorshDeserialize, BorshSerialize};

pub const SHARD_PROGRAM_ID: Pubkey = pubkey!("CjBEGXTxN4cMrvNJW6XkLM424pJ6g5aMNfWagDYwtPZy");
pub const SHARD_IX_RECORD_ENCRYPTED_DEPOSIT: u8 = 20;
pub const SHARD_IX_UPDATE_FACTION_SOL: u8 = 21;

pub const MAX_DEPOSIT_LEGS: usize = 3;

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct RecordEncryptedDepositCommitmentArgs {
    pub computation_offset: u64,
    pub net_lamports: u64,
    pub pending_gross_lamports: u64,
    pub encryption_nonce: u128,
    pub encryption_pubkey: [u8; 32],
    pub leg_count_ciphertext: [u8; 32],
    pub leg0_faction_ciphertext: [u8; 32],
    pub leg0_amount_ciphertext: [u8; 32],
    pub leg1_faction_ciphertext: [u8; 32],
    pub leg1_amount_ciphertext: [u8; 32],
    pub leg2_faction_ciphertext: [u8; 32],
    pub leg2_amount_ciphertext: [u8; 32],
    pub commitment_hash: [u8; 32],
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug)]
pub struct UpdateFactionSolFromArciumArgs {
    pub round_id: u64,
    pub leg_count: u8,
    pub faction_indices: [u8; MAX_DEPOSIT_LEGS],
    pub amounts: [u64; MAX_DEPOSIT_LEGS],
}

pub fn encode_record_encrypted_deposit_ix(args: &RecordEncryptedDepositCommitmentArgs) -> Vec<u8> {
    let mut data = vec![SHARD_IX_RECORD_ENCRYPTED_DEPOSIT];
    data.extend_from_slice(&borsh::to_vec(args).unwrap());
    data
}

pub fn encode_update_faction_sol_ix(args: &UpdateFactionSolFromArciumArgs) -> Vec<u8> {
    let mut data = vec![SHARD_IX_UPDATE_FACTION_SOL];
    data.extend_from_slice(&borsh::to_vec(args).unwrap());
    data
}

#[allow(clippy::too_many_arguments)]
pub fn invoke_record_encrypted_deposit<'info>(
    shard_program: &AccountInfo<'info>,
    user: &AccountInfo<'info>,
    user_state: &AccountInfo<'info>,
    global_config: &AccountInfo<'info>,
    mint_emission: &AccountInfo<'info>,
    active_round_state: &AccountInfo<'info>,
    sol_deposit_vault: &AccountInfo<'info>,
    instructions_sysvar: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    deposit_receipt: &AccountInfo<'info>,
    prior_round_state: Option<&AccountInfo<'info>>,
    protocol_fee_vault: Option<&AccountInfo<'info>>,
    withdrawal_fee_pool: Option<&AccountInfo<'info>>,
    args: &RecordEncryptedDepositCommitmentArgs,
) -> Result<()> {
    let mut account_metas = vec![
        AccountMeta::new(*user.key, true),
        AccountMeta::new(*user_state.key, false),
        AccountMeta::new_readonly(*global_config.key, false),
        AccountMeta::new_readonly(*mint_emission.key, false),
        AccountMeta::new(*active_round_state.key, false),
        AccountMeta::new(*sol_deposit_vault.key, false),
        AccountMeta::new_readonly(*instructions_sysvar.key, false),
        AccountMeta::new_readonly(*system_program.key, false),
        AccountMeta::new(*deposit_receipt.key, false),
    ];
    let mut account_infos = vec![
        user.clone(),
        user_state.clone(),
        global_config.clone(),
        mint_emission.clone(),
        active_round_state.clone(),
        sol_deposit_vault.clone(),
        instructions_sysvar.clone(),
        system_program.clone(),
        deposit_receipt.clone(),
    ];

    if let Some(prior) = prior_round_state {
        account_metas.push(AccountMeta::new(*prior.key, false));
        account_infos.push(prior.clone());
    }
    if let Some(fee_vault) = protocol_fee_vault {
        account_metas.push(AccountMeta::new(*fee_vault.key, false));
        account_infos.push(fee_vault.clone());
    }
    if let Some(pool) = withdrawal_fee_pool {
        account_metas.push(AccountMeta::new(*pool.key, false));
        account_infos.push(pool.clone());
    }

    let ix = Instruction {
        program_id: *shard_program.key,
        accounts: account_metas,
        data: encode_record_encrypted_deposit_ix(args),
    };
    invoke(&ix, &account_infos)?;
    Ok(())
}

pub fn invoke_update_faction_sol<'info>(
    shard_program: &AccountInfo<'info>,
    sign_pda: &AccountInfo<'info>,
    sign_pda_bump: u8,
    round_state: &AccountInfo<'info>,
    global_config: &AccountInfo<'info>,
    deposit_receipt: &AccountInfo<'info>,
    args: &UpdateFactionSolFromArciumArgs,
) -> Result<()> {
    let ix = Instruction {
        program_id: *shard_program.key,
        accounts: vec![
            AccountMeta::new_readonly(*sign_pda.key, true),
            AccountMeta::new(*round_state.key, false),
            AccountMeta::new_readonly(*global_config.key, false),
            AccountMeta::new(*deposit_receipt.key, false),
        ],
        data: encode_update_faction_sol_ix(args),
    };
    invoke_signed(
        &ix,
        &[
            sign_pda.clone(),
            round_state.clone(),
            global_config.clone(),
            deposit_receipt.clone(),
        ],
        &[&[b"ArciumSignerAccount", &[sign_pda_bump]]],
    )?;
    Ok(())
}
