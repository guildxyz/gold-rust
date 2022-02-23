use crate::solana_program::clock::UnixTimestamp;
use crate::Pubkey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAuctionConfig {
    pub description: String,
    pub socials: Vec<String>,
    pub asset: FrontendTokenConfig,
    pub encore_period: Option<UnixTimestamp>,
    pub cycle_period: UnixTimestamp,
    pub number_of_cycles: u64,
    pub start_time: Option<UnixTimestamp>,
    pub min_bid: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAuction {
    #[serde(flatten)]
    pub base: FrontendAuctionBase,
    #[serde(flatten)]
    pub config: FrontendAuctionConfig,
    pub available_treasury_amount: f32,
    pub current_cycle: u64,
    pub is_finished: bool,
    pub is_frozen: bool,
    pub is_filtered: bool,
    pub root_state_pubkey: Pubkey,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAuctionBaseConfig {
    pub id: String,
    pub name: String,
    pub owner_pubkey: Pubkey,
    pub goal_treasury_amount: f32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAuctionBase {
    #[serde(flatten)]
    pub config: FrontendAuctionBaseConfig,
    pub all_time_treasury_amount: f32,
    pub is_verified: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendBid {
    pub bidder_pubkey: Pubkey,
    pub amount: f32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendCycle {
    pub bids: Vec<FrontendBid>,
    pub end_timestamp: UnixTimestamp,
}
