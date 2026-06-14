use arcis::*;

#[encrypted]
mod circuits {
    use arcis::*;

    /// Max distinct faction legs per deposit (matches shard `MAX_FACTIONS_PER_USER_PER_ROUND`).
    pub const MAX_DEPOSIT_LEGS: usize = 3;

    /// Padding faction for unused leg slots (must be >= FACTION_COUNT).
    pub const INACTIVE_FACTION: u8 = 255;

    /// One faction + amount pair in a multi-leg deposit.
    pub struct DepositLeg {
        pub faction_index: u8,
        pub amount: u64,
    }

    /// Encrypted multi-leg deposit (up to [`MAX_DEPOSIT_LEGS`] active legs).
    pub struct DepositInput {
        /// Number of active legs (1–3). Remaining slots must be inactive padding.
        pub leg_count: u8,
        pub legs: [DepositLeg; MAX_DEPOSIT_LEGS],
    }

    /// Revealed leg after MPC (inactive slots use [`INACTIVE_FACTION`] and amount 0).
    pub struct RevealedDepositLeg {
        pub faction_index: u8,
        pub amount: u64,
    }

    /// Processes up to three encrypted deposit legs in one MPC computation.
    ///
    /// Returns all three slots; inactive slots are `(INACTIVE_FACTION, 0)`.
    /// The on-chain callback validates leg count, factions, and duplicates.
    #[instruction]
    pub fn deposit(input_ctxt: Enc<Shared, DepositInput>) -> [RevealedDepositLeg; MAX_DEPOSIT_LEGS] {
        let input = input_ctxt.to_arcis();
        let mut out = [
            RevealedDepositLeg {
                faction_index: INACTIVE_FACTION,
                amount: 0,
            },
            RevealedDepositLeg {
                faction_index: INACTIVE_FACTION,
                amount: 0,
            },
            RevealedDepositLeg {
                faction_index: INACTIVE_FACTION,
                amount: 0,
            },
        ];

        for i in 0..MAX_DEPOSIT_LEGS {
            let leg = &input.legs[i];
            out[i] = RevealedDepositLeg {
                faction_index: leg.faction_index.reveal(),
                amount: leg.amount.reveal(),
            };
        }

        out
    }
}
