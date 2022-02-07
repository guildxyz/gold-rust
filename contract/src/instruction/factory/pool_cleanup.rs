use super::*;

pub fn pool_cleanup(payer_pubkey: &Pubkey, accounts_to_clean: Vec<Pubkey>) -> Instruction {
    let (auction_pool_pubkey, _) = Pubkey::find_program_address(&auction_pool_seeds(), &crate::ID);
    let (secondary_pool_pubkey, _) =
        Pubkey::find_program_address(&secondary_pool_seeds(), &crate::ID);

    let mut accounts = vec![
        AccountMeta::new_readonly(*payer_pubkey, true),
        AccountMeta::new(auction_pool_pubkey, false),
        AccountMeta::new(secondary_pool_pubkey, false),
    ];

    for account in accounts_to_clean.into_iter() {
        accounts.push(AccountMeta::new(account, false));
    }

    let instruction = AuctionInstruction::PoolCleanup;

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}
