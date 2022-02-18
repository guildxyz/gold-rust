use super::*;

#[derive(BorshSchema, BorshSerialize, BorshDeserialize)]
pub struct ClaimRewardsArgs {
    pub payer_pubkey: Pubkey,
    pub top_bidder_pubkey: Pubkey,
    #[alias([u8; 32])]
    pub auction_id: AuctionId,
    pub cycle_number: u64,
    pub token_type: TokenType,
    pub existing_token_mint: Option<Pubkey>,
}

pub fn claim_rewards(args: &ClaimRewardsArgs) -> Instruction {
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&args.auction_id), &crate::ID);
    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(&auction_root_state_pubkey, &args.cycle_number.to_le_bytes()),
        &crate::ID,
    );

    let (contract_pda, _) = Pubkey::find_program_address(&contract_pda_seeds(), &crate::ID);

    let mut accounts = vec![
        AccountMeta::new(args.payer_pubkey, true),
        AccountMeta::new_readonly(args.top_bidder_pubkey, false),
        AccountMeta::new_readonly(auction_root_state_pubkey, false),
        AccountMeta::new(auction_cycle_state_pubkey, false),
        AccountMeta::new_readonly(contract_pda, false),
        AccountMeta::new_readonly(RENT_ID, false),
        AccountMeta::new_readonly(SYS_ID, false),
        AccountMeta::new_readonly(TOKEN_ID, false),
    ];

    let mut token_accounts = match args.token_type {
        TokenType::Nft => {
            let master_pdas = EditionPda::new(EditionType::Master, &args.auction_id);
            let child_pdas =
                EditionPda::new(EditionType::Child(args.cycle_number), &args.auction_id);

            let edition_div = args
                .cycle_number
                .checked_div(EDITION_MARKER_BIT_SIZE)
                .unwrap();
            let edition_string = edition_div.to_string();
            let (child_edition_marker_pubkey, _) = Pubkey::find_program_address(
                &edition_marker_seeds(&edition_string, &master_pdas.mint),
                &agsol_token_metadata::ID,
            );

            vec![
                AccountMeta::new_readonly(META_ID, false),
                AccountMeta::new(child_pdas.edition, false),
                AccountMeta::new(child_edition_marker_pubkey, false),
                AccountMeta::new(child_pdas.metadata, false),
                AccountMeta::new(child_pdas.mint, false),
                AccountMeta::new(child_pdas.holding, false),
                AccountMeta::new(master_pdas.edition, false),
                AccountMeta::new(master_pdas.metadata, false),
                AccountMeta::new_readonly(master_pdas.mint, false),
                AccountMeta::new_readonly(master_pdas.holding, false),
            ]
        }
        TokenType::Token => {
            let mint_pubkey = args.existing_token_mint.unwrap_or_else(|| {
                Pubkey::find_program_address(&token_mint_seeds(&args.auction_id), &crate::ID).0
            });
            let (token_holding_pubkey, _) = Pubkey::find_program_address(
                &token_holding_seeds(&mint_pubkey, &args.top_bidder_pubkey),
                &crate::ID,
            );
            vec![
                AccountMeta::new(mint_pubkey, false),
                AccountMeta::new(token_holding_pubkey, false),
            ]
        }
    };

    accounts.append(&mut token_accounts);

    let instruction = AuctionInstruction::ClaimRewards {
        id: args.auction_id,
        cycle_number: args.cycle_number,
    };

    Instruction {
        program_id: crate::ID,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    }
}
