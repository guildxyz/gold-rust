use super::*;

pub struct VerifyAuctionArgs {
    pub contract_admin_pubkey: Pubkey,
    pub auction_id: AuctionId,
}

pub fn verify_auction(args: &VerifyAuctionArgs) -> Instruction {
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &crate::ID);

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&args.auction_id), &crate::ID);

    let accounts = vec![
        AccountMeta::new(args.contract_admin_pubkey, true),
        AccountMeta::new(contract_bank_pubkey, false),
        AccountMeta::new(auction_root_state_pubkey, false),
    ];

    let instruction = AuctionInstruction::VerifyAuction {
        id: args.auction_id,
    };

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}
