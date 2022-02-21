use super::*;

#[derive(BorshSchema, BorshSerialize, BorshDeserialize, Debug)]
pub struct ModifyAuctionArgs {
    pub auction_owner_pubkey: Pubkey,
    #[alias([u8; 32])]
    pub auction_id: AuctionId,
    pub modify_data: ModifyAuctionData,
}

pub fn modify_auction(args: &ModifyAuctionArgs) -> Instruction {
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&args.auction_id), &crate::ID);

    let accounts = vec![
        AccountMeta::new(args.auction_owner_pubkey, true),
        AccountMeta::new(auction_root_state_pubkey, false),
    ];

    let instruction = AuctionInstruction::ModifyAuction {
        id: args.auction_id,
        modify_data: args.modify_data.clone(),
    };

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}
