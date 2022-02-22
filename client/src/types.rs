use crate::Pubkey;
use crate::solana_program::clock::UnixTimestamp;
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
    description: String,
    socials: Vec<String>,
    asset: FrontendTokenConfig,
    encore_period: Option<UnixTimestamp>,
    cycle_period: UnixTimestamp,
    number_of_cycles: u64,
    start_time: Option<UnixTimestamp>,
    min_bid: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAuction {
    pub base: FrontendAuctionBase,
    pub config: FrontendAuctionConfig,
    pub available_treasury_amount: f32,
    pub current_cycle: f32,
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
    pub all_time_treasury_amount: String,
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
