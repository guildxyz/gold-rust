use super::*;

pub fn filter_auction(admin_pubkey: Pubkey, auction_id: AuctionId) -> Instruction {
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &crate::ID);
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &crate::ID);

    let accounts = vec![
        AccountMeta::new_readonly(admin_pubkey, true),
        AccountMeta::new(auction_root_state_pubkey, false),
        AccountMeta::new_readonly(contract_bank_pubkey, false),
    ];

    let instruction = AuctionInstruction::FilterAuction { id: auction_id };

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}

pub fn unfilter_auction(admin_pubkey: Pubkey, auction_id: AuctionId) -> Instruction {
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &crate::ID);
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &crate::ID);

    let accounts = vec![
        AccountMeta::new_readonly(admin_pubkey, true),
        AccountMeta::new(auction_root_state_pubkey, false),
        AccountMeta::new_readonly(contract_bank_pubkey, false),
    ];

    let instruction = AuctionInstruction::UnFilterAuction { id: auction_id };

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}
