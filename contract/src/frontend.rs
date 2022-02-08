use crate::state::AuctionRootState;
use agsol_borsh_schema::BorshSchema;
use agsol_common::{MaxLenString, MaxSerializedLen};
use agsol_token_metadata::state::{MAX_NAME_LENGTH, MAX_SYMBOL_LENGTH, MAX_URI_LENGTH};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

#[derive(BorshSchema, MaxSerializedLen, BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum FrontendTokenConfig {
    Nft {
        #[alias(String)]
        name: MaxLenString<MAX_NAME_LENGTH>,
        #[alias(String)]
        symbol: MaxLenString<MAX_SYMBOL_LENGTH>,
        #[alias(String)]
        uri: MaxLenString<MAX_URI_LENGTH>,
        is_repeating: bool,
    },
    Token {
        mint: Pubkey,
        decimals: u8,
        per_cycle_amount: u64,
    },
}

#[derive(BorshSchema, MaxSerializedLen, BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct FrontendAuction {
    pub root_state_pubkey: Pubkey,
    pub root_state: AuctionRootState,
    pub token_config: FrontendTokenConfig,
}
