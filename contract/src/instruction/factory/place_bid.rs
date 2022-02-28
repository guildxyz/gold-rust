use super::*;

#[derive(BorshSchema, BorshSerialize, BorshDeserialize)]
pub struct PlaceBidArgs {
    pub bidder_pubkey: Pubkey,
    #[alias([u8; 32])]
    pub auction_id: AuctionId,
    pub cycle_number: u64,
    pub top_bidder_pubkey: Option<Pubkey>,
    pub amount: u64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FrontendPlaceBidArgs {
    pub bidder_pubkey: String,
    pub auction_id: String,
    pub cycle_number: u64,
    pub amount: f32,
    pub top_bidder_pubkey: Option<String>,
}

impl TryFrom<FrontendPlaceBidArgs> for PlaceBidArgs {
    type Error = String;
    fn try_from(args: FrontendPlaceBidArgs) -> Result<Self, Self::Error> {
        todo!()
    }
}

pub fn place_bid(args: &PlaceBidArgs) -> Instruction {
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&args.auction_id), &crate::ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&args.auction_id), &crate::ID);
    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(&auction_root_state_pubkey, &args.cycle_number.to_le_bytes()),
        &crate::ID,
    );
    let (auction_pool_pubkey, _) = Pubkey::find_program_address(&auction_pool_seeds(), &crate::ID);
    let (secondary_pool_pubkey, _) =
        Pubkey::find_program_address(&secondary_pool_seeds(), &crate::ID);

    let top_bidder = if let Some(bidder) = args.top_bidder_pubkey {
        bidder
    } else {
        Pubkey::default()
    };

    let accounts = vec![
        AccountMeta::new(args.bidder_pubkey, true),
        AccountMeta::new(auction_bank_pubkey, false),
        AccountMeta::new(auction_root_state_pubkey, false),
        AccountMeta::new(auction_cycle_state_pubkey, false),
        AccountMeta::new(top_bidder, false),
        AccountMeta::new(auction_pool_pubkey, false),
        AccountMeta::new(secondary_pool_pubkey, false),
        AccountMeta::new_readonly(SYS_ID, false),
    ];

    let instruction = AuctionInstruction::Bid {
        id: args.auction_id,
        amount: args.amount,
    };
    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}
