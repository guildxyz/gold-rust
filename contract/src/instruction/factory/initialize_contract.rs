use super::*;

pub struct InitializeContractArgs {
    pub contract_admin: Pubkey,
    pub withdraw_authority: Pubkey,
    pub initial_auction_pool_len: u32,
}

pub fn initialize_contract(args: &InitializeContractArgs) -> Instruction {
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &crate::ID);

    let (auction_pool_pubkey, _) = Pubkey::find_program_address(&auction_pool_seeds(), &crate::ID);
    let (secondary_pool_pubkey, _) = Pubkey::find_program_address(&secondary_pool_seeds(), &crate::ID);

    let accounts = vec![
        AccountMeta::new(args.contract_admin, true),
        AccountMeta::new(contract_bank_pubkey, false),
        AccountMeta::new(auction_pool_pubkey, false),
        AccountMeta::new(secondary_pool_pubkey, false),
        AccountMeta::new_readonly(SYS_ID, false),
    ];

    let instruction = AuctionInstruction::InitializeContract {
        withdraw_authority: args.withdraw_authority,
        initial_auction_pool_len: args.initial_auction_pool_len,
    };

    // unwrap is fine because instruction is serializable
    let data = instruction.try_to_vec().unwrap();
    Instruction {
        program_id: crate::ID,
        accounts,
        data,
    }
}
