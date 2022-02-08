use super::*;

#[derive(BorshSchema, BorshSerialize, BorshDeserialize)]
pub struct DeleteAuctionArgs {
    pub auction_owner_pubkey: Pubkey,
    pub top_bidder_pubkey: Option<Pubkey>,
    #[alias([u8; 32])]
    pub auction_id: AuctionId,
    pub current_auction_cycle: u64,
    pub num_of_cycles_to_delete: u64,
}

pub fn delete_auction(args: &DeleteAuctionArgs) -> Instruction {
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &crate::ID);

    let (auction_pool_pubkey, _) = Pubkey::find_program_address(&auction_pool_seeds(), &crate::ID);

    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&args.auction_id), &crate::ID);

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&args.auction_id), &crate::ID);

    let top_bidder = if let Some(bidder) = args.top_bidder_pubkey {
        bidder
    } else {
        Pubkey::default()
    };

    let mut accounts = vec![
        AccountMeta::new(args.auction_owner_pubkey, true),
        AccountMeta::new(top_bidder, false),
        AccountMeta::new(auction_root_state_pubkey, false),
        AccountMeta::new(auction_bank_pubkey, false),
        AccountMeta::new(contract_bank_pubkey, false),
        AccountMeta::new(auction_pool_pubkey, false),
    ];

    let cycles_to_include = std::cmp::min(args.current_auction_cycle, args.num_of_cycles_to_delete);
    for i in 0..cycles_to_include {
        let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
            &auction_cycle_state_seeds(
                &auction_root_state_pubkey,
                &(args.current_auction_cycle - i).to_le_bytes(),
            ),
            &crate::ID,
        );
        accounts.push(AccountMeta::new(auction_cycle_state_pubkey, false));
    }

    let instruction = AuctionInstruction::DeleteAuction {
        id: args.auction_id,
        num_of_cycles_to_delete: args.num_of_cycles_to_delete,
    };

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}
