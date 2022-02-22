use crate::state::AuctionRootState;
use agsol_borsh_schema::BorshSchema;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

#[derive(BorshSchema, BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum FrontendTokenConfig {
    Nft {
        name: String,
        symbol: String,
        uri: String,
        is_repeating: bool,
    },
    Token {
        mint: Pubkey,
        decimals: u8,
        per_cycle_amount: u64,
    },
}

#[derive(BorshSchema, BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct FrontendAuction {
    pub root_state_pubkey: Pubkey,
    pub root_state: AuctionRootState,
    pub token_config: FrontendTokenConfig,
}

#[derive(BorshSchema, BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct FrontendAuctionBase {
    pub id: String,
    pub name: String,
    pub owner: String,
    pub goal_treasury_amount: String,
    pub all_time_treasury_amount: String,
    pub is_verified: bool,
}

#[derive(BorshSchema, BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct FrontendAuctionBaseArray {
    array: Vec<FrontendAuctionBase>,
}
