use crate::{InitializeAuctionArgs, pad_to_32_bytes};
use agsol_gold_contract::solana_program::clock::UnixTimestamp;
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
        mint: String,
        decimals: u8,
        per_cycle_amount: u64,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAuctionConfig {
    #[serde(flatten)]
    pub base: FrontendAuctionBaseConfig,
    pub description: String,
    pub socials: Vec<String>,
    #[serde(flatten)]
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
    pub config: FrontendAuctionConfig,
    pub available_treasury_amount: f32,
    pub current_cycle: u64,
    pub is_finished: bool,
    pub is_frozen: bool,
    pub is_filtered: bool,
    pub root_state_pubkey: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAuctionBaseConfig {
    pub id: String,
    pub name: String,
    pub owner_pubkey: String,
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
    pub bidder_pubkey: String,
    pub amount: f32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendCycle {
    pub bids: Vec<FrontendBid>,
    pub end_timestamp: UnixTimestamp,
}

impl FrontendAuctionConfig {
    pub fn into_initialize_auction_args(self) -> Result<InitializeAuctionArgs, String> {
        let auction_owner_pubkey = Pubkey::from_str(self.base.owner_pubkey)?;
        let creator = Creator {
            address: auction_owner_pubkey,
            verified: false,
            share: 100,
        };
        let auction_id = pad_to_32_bytes(self.base.id)?;
        let auction_name = pad_to_32_bytes(self.base.name)?;
        let auction_config = AuctionConfig {
            cycle_period: self.cycle_period,
            encore_period: self.encore_period,
            number_of_cycles: self.number_of_cycles,
            minimum_bid_amount: self.min_bid,
        };
        let socials = self.socials.into_iter().map(|link| MaxLenString::try_from(link)?).collect::<Vec<SocialsString>>();
        let auction_description = AuctionDescription {
            description: MaxLenString::try_from(self.description)?,
            socials: socials.try_into()?,
            goal_treasury_amount: (self.base.goal_treasury_amount * LAMPORTS) as u64,
        };
        let create_token_args = match self.asset {
            FrontendTokenConfig::Nft {name, symbol, uri, is_repeating } => {
                CreateTokenArgs {
                    metadata_args: CreateMetadataAccountArgs {
                        agsol_token_metadata::state::Data {
                            name,
                            symbol,
                            uri,
                            seller_fee_basis_points: 50,
                            creators: Some(vec![creator]),
                        },
                        is_mutable: true,
                    },
                    is_repeating,
                }
            },
            FrontendTokenConfig::Token { } => {
            }
        };
        let auction_start_timestamp = self.start_time;

        InitializeAuctionArgs {
            auction_owner_pubkey,
            auction_id,
            auction_name,
            auction_config,
            auction_description,
            create_token_args,
            auction_start_timestamp,
        }
    }
}
