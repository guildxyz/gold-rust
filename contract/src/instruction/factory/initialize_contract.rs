use super::*;

pub struct InitializeContractArgs {
    pub contract_admin: Pubkey,
    pub withdraw_authority: Pubkey,
}

pub fn initialize_contract(args: &InitializeContractArgs) -> Instruction {
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&get_contract_bank_seeds(), &crate::ID);

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&get_auction_pool_seeds(), &crate::ID);

    let accounts = vec![
        AccountMeta::new(args.contract_admin, true),
        AccountMeta::new(contract_bank_pubkey, false),
        AccountMeta::new(auction_pool_pubkey, false),
        AccountMeta::new_readonly(SYS_ID, false),
    ];

    let instruction = AuctionInstruction::InitializeContract {
        withdraw_authority: args.withdraw_authority,
    };

    // unwrap is fine because instruction is serializable
    let data = instruction.try_to_vec().unwrap();
    Instruction {
        program_id: crate::ID,
        accounts,
        data,
    }
}
