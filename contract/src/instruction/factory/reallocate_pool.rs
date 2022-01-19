use super::*;

pub fn reallocate_pool(contract_admin_pubkey: &Pubkey, new_max_auction_num: u32) -> Instruction {
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &crate::ID);
    let (auction_pool_pubkey, _) = Pubkey::find_program_address(&auction_pool_seeds(), &crate::ID);

    let accounts = vec![
        AccountMeta::new(*contract_admin_pubkey, true),
        AccountMeta::new_readonly(contract_bank_pubkey, false),
        AccountMeta::new(auction_pool_pubkey, false),
    ];

    let instruction = AuctionInstruction::ReallocatePool {
        new_max_auction_num,
    };

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}
