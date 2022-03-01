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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FrontendDeleteAuctionArgs {
    pub auction_owner_pubkey: String,
    pub top_bidder_pubkey: Option<String>,
    pub auction_id: String,
    pub cycle_number: u64,
}

impl TryFrom<FrontendDeleteAuctionArgs> for DeleteAuctionArgs {
    type Error = String;
    fn try_from(args: FrontendDeleteAuctionArgs) -> Result<Self, Self::Error> {
        let top_bidder_pubkey = if let Some(pubkey_string) = args.top_bidder_pubkey {
            Some(Pubkey::from_str(&pubkey_string).map_err(|e| e.to_string())?)
        } else {
            None
        };
        Ok(Self {
            auction_owner_pubkey: Pubkey::from_str(&args.auction_owner_pubkey)
                .map_err(|e| e.to_string())?,
            top_bidder_pubkey,
            auction_id: pad_to_32_bytes(&args.auction_id)?,
            current_auction_cycle: args.cycle_number,
            num_of_cycles_to_delete: crate::RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL,
        })
    }
}

pub fn delete_auction(args: &DeleteAuctionArgs) -> Instruction {
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &crate::ID);

    let (auction_pool_pubkey, _) = Pubkey::find_program_address(&auction_pool_seeds(), &crate::ID);
    let (secondary_pool_pubkey, _) =
        Pubkey::find_program_address(&secondary_pool_seeds(), &crate::ID);

    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&args.auction_id), &crate::ID);

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&args.auction_id), &crate::ID);

    let (protocol_fee_state_pubkey, _) =
        Pubkey::find_program_address(&protocol_fee_state_seeds(), &crate::ID);

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
        AccountMeta::new(secondary_pool_pubkey, false),
        AccountMeta::new_readonly(protocol_fee_state_pubkey, false),
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

pub fn delete_all(mut args: DeleteAuctionArgs) -> Vec<Instruction> {
    let mut delete_rounds = args.current_auction_cycle / args.num_of_cycles_to_delete;
    if args.current_auction_cycle % args.num_of_cycles_to_delete != 0 {
        delete_rounds += 1;
    }

    let mut instructions = Vec::<Instruction>::with_capacity(delete_rounds as usize);
    for round in 1..=delete_rounds {
        instructions.push(delete_auction(&args));
        if round < delete_rounds {
            args.current_auction_cycle -= args.num_of_cycles_to_delete;
        }
    }
    instructions
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn delete_all_no_rem() {
        let args = DeleteAuctionArgs {
            auction_owner_pubkey: Pubkey::from_str("7Z8ftDAzMvoyXnGEJye8DurzgQQXLAbYCaeeesM7UKHa")
                .unwrap(),
            top_bidder_pubkey: Some(
                Pubkey::from_str("7Z8ftDAzMvoyXnGEJye8DurzgQQXLAbYCaeeesM7UKN1").unwrap(),
            ),
            auction_id: [120; 32],
            current_auction_cycle: 80,
            num_of_cycles_to_delete: 40,
        };

        let instructions = delete_all(args);
        assert_eq!(instructions.len(), 2);
    }

    #[test]
    fn delete_all_with_rem() {
        let args = DeleteAuctionArgs {
            auction_owner_pubkey: Pubkey::from_str("7Z8ftDAzMvoyXnGEJye8DurzgQQXLAbYCaeeesM7UKHa")
                .unwrap(),
            top_bidder_pubkey: Some(
                Pubkey::from_str("7Z8ftDAzMvoyXnGEJye8DurzgQQXLAbYCaeeesM7UKN1").unwrap(),
            ),
            auction_id: [120; 32],
            current_auction_cycle: 33,
            num_of_cycles_to_delete: 20,
        };
        let instructions = delete_all(args);
        assert_eq!(instructions.len(), 2);

        let args = DeleteAuctionArgs {
            auction_owner_pubkey: Pubkey::from_str("7Z8ftDAzMvoyXnGEJye8DurzgQQXLAbYCaeeesM7UKHa")
                .unwrap(),
            top_bidder_pubkey: Some(
                Pubkey::from_str("7Z8ftDAzMvoyXnGEJye8DurzgQQXLAbYCaeeesM7UKN1").unwrap(),
            ),
            auction_id: [120; 32],
            current_auction_cycle: 8,
            num_of_cycles_to_delete: 20,
        };
        let instructions = delete_all(args);
        assert_eq!(instructions.len(), 1);
    }
}
