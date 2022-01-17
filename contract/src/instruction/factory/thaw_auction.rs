use super::*;

#[derive(BorshSchema, BorshSerialize, BorshDeserialize)]
pub struct ThawAuctionArgs {
    pub contract_admin_pubkey: Pubkey,
    #[alias([u8; 32])]
    pub auction_id: AuctionId,
}

pub fn thaw_auction(args: &ThawAuctionArgs) -> Instruction {
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&args.auction_id), &crate::ID);
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &crate::ID);

    let accounts = vec![
        AccountMeta::new_readonly(args.contract_admin_pubkey, true),
        AccountMeta::new(auction_root_state_pubkey, false),
        AccountMeta::new(contract_bank_pubkey, false),
    ];

    let instruction = AuctionInstruction::Thaw {
        id: args.auction_id,
    };

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}
