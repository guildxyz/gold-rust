use super::*;

#[derive(BorshSchema, BorshSerialize, BorshDeserialize)]
pub struct ClaimFundsArgs {
    pub caller_pubkey: Pubkey,
    pub auction_owner_pubkey: Pubkey,
    #[alias([u8; 32])]
    pub auction_id: AuctionId,
    pub cycle_number: u64,
    pub amount: u64,
}

pub fn claim_funds(args: &ClaimFundsArgs) -> Instruction {
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&args.auction_id), &crate::ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&args.auction_id), &crate::ID);
    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(&auction_root_state_pubkey, &args.cycle_number.to_le_bytes()),
        &crate::ID,
    );

    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &crate::ID);

    let (protocol_fee_state_pubkey, _) =
        Pubkey::find_program_address(&protocol_fee_state_seeds(), &crate::ID);

    let accounts = vec![
        AccountMeta::new(args.caller_pubkey, true),
        AccountMeta::new(args.auction_owner_pubkey, false),
        AccountMeta::new(auction_bank_pubkey, false),
        AccountMeta::new(auction_root_state_pubkey, false),
        AccountMeta::new(auction_cycle_state_pubkey, false),
        AccountMeta::new(contract_bank_pubkey, false),
        AccountMeta::new_readonly(protocol_fee_state_pubkey, false),
    ];

    let instruction = AuctionInstruction::ClaimFunds {
        id: args.auction_id,
        amount: args.amount,
    };

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}
