use crate::utils::to_lamports;
use crate::{pad_to_32_bytes, InitializeAuctionArgs};
use agsol_gold_contract::solana_program::clock::UnixTimestamp;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::state::*;
use agsol_gold_contract::UNIVERSAL_BID_FLOOR;
use agsol_token_metadata::instruction::CreateMetadataAccountArgs;
use agsol_token_metadata::state::{Creator, Data as NftMetadata};
use serde::{Deserialize, Serialize};

use std::str::FromStr;

pub type Scalar = f64;
pub const SELLER_FEE_BASIS_POINTS: u16 = 50;

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum FrontendTokenConfig {
    #[serde(rename_all = "camelCase")]
    Nft {
        name: String,
        symbol: String,
        uri: String,
        is_repeating: bool,
    },
    #[serde(rename_all = "camelCase")]
    Token {
        mint: Option<String>,
        decimals: u8,
        per_cycle_amount: u64,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAuctionConfig {
    #[serde(flatten)]
    pub base: FrontendAuctionBaseConfig,
    #[serde(flatten)]
    pub extra: FrontendAuctionConfigExtra,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAuctionConfigExtra {
    pub description: String,
    pub socials: Vec<String>,
    pub asset: FrontendTokenConfig,
    pub encore_period: Option<UnixTimestamp>,
    pub cycle_period: UnixTimestamp,
    pub number_of_cycles: u64,
    pub start_time: Option<UnixTimestamp>,
    pub min_bid: Option<Scalar>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAuctionBase {
    #[serde(flatten)]
    pub config: FrontendAuctionBaseConfig,
    pub all_time_treasury_amount: Scalar,
    pub is_verified: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAuctionBaseConfig {
    pub id: String,
    pub name: String,
    pub owner_pubkey: String,
    pub goal_treasury_amount: Option<Scalar>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAuction {
    #[serde(flatten)]
    pub base: FrontendAuctionBase,
    #[serde(flatten)]
    pub config: FrontendAuctionConfigExtra,
    pub available_treasury_amount: Scalar,
    pub current_cycle: u64,
    pub is_finished: bool,
    pub is_frozen: bool,
    pub is_filtered: bool,
    pub root_state_pubkey: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendBid {
    pub bidder_pubkey: String,
    pub amount: Scalar,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendCycle {
    pub bids: Vec<FrontendBid>,
    pub end_timestamp: UnixTimestamp,
}

impl FrontendAuctionConfig {
    pub fn into_initialize_auction_args(self) -> Result<InitializeAuctionArgs, String> {
        let auction_owner_pubkey =
            Pubkey::from_str(&self.base.owner_pubkey).map_err(|e| e.to_string())?;
        let creator = Creator {
            address: auction_owner_pubkey,
            verified: false,
            share: 100,
        };
        let auction_id = pad_to_32_bytes(&self.base.id)?;
        let auction_name = pad_to_32_bytes(&self.base.name)?;
        let auction_config = AuctionConfig {
            cycle_period: self.extra.cycle_period,
            encore_period: self.extra.encore_period.unwrap_or_default(),
            number_of_cycles: Some(self.extra.number_of_cycles),
            minimum_bid_amount: self
                .extra
                .min_bid
                .map(to_lamports)
                .unwrap_or_else(|| UNIVERSAL_BID_FLOOR),
        };
        let mut socials = Vec::<SocialsString>::with_capacity(self.extra.socials.len());
        for link in self.extra.socials.into_iter() {
            socials.push(SocialsString::try_from(link)?);
        }
        let auction_description = AuctionDescription {
            description: DescriptionString::try_from(self.extra.description)?,
            socials: socials.try_into()?,
            goal_treasury_amount: self.base.goal_treasury_amount.map(to_lamports),
        };
        let create_token_args = match self.extra.asset {
            FrontendTokenConfig::Nft {
                name,
                symbol,
                uri,
                is_repeating,
            } => CreateTokenArgs::Nft {
                metadata_args: CreateMetadataAccountArgs {
                    data: NftMetadata {
                        name,
                        symbol,
                        uri,
                        seller_fee_basis_points: SELLER_FEE_BASIS_POINTS,
                        creators: Some(vec![creator]),
                    },
                    is_mutable: true,
                },
                is_repeating,
            },
            FrontendTokenConfig::Token {
                mint,
                decimals,
                per_cycle_amount,
            } => {
                let existing_mint = if let Some(mint) = mint {
                    Some(Pubkey::from_str(&mint).map_err(|e| e.to_string())?)
                } else {
                    None
                };
                CreateTokenArgs::Token {
                    existing_mint,
                    decimals,
                    per_cycle_amount,
                }
            }
        };
        let auction_start_timestamp = self.extra.start_time;

        Ok(InitializeAuctionArgs {
            auction_owner_pubkey,
            auction_id,
            auction_name,
            auction_config,
            auction_description,
            create_token_args,
            auction_start_timestamp,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn token_deserialization() {
        let example_json = r#"
        {
            "id": "john-doe",
            "name": "John Doe",
            "description": "xd",
            "socials": [
                "aaa.aaa",
                "bbb.bbb"
            ],
            "goalTreasuryAmount": null,
            "ownerPubkey": "95b225CEtMmkRYUpg626DNqen55FgwEGbH5NKVXHUejT",
            "asset": {
                "type": "Token",
                "mint": null,
                "decimals": 5,
                "perCycleAmount": 100000
            },
            "encorePeriod": 0,
            "cyclePeriod": 3600,
            "numberOfCycles": 20,
            "startTime": null,
            "minBid": 0.07
        }"#;

        let deserialized: FrontendAuctionConfig = serde_json::from_str(example_json).unwrap();
        assert_eq!(deserialized.base.id, "john-doe");
        match deserialized.extra.asset {
            FrontendTokenConfig::Token { mint, decimals, per_cycle_amount } => {
                assert!(mint.is_none());
                assert_eq!(decimals, 5);
                assert_eq!(per_cycle_amount, 100_000);
            }
            _ => panic!("should be Token")
        }
        assert!(deserialized.extra.start_time.is_none());
        assert_eq!(deserialized.extra.min_bid, Some(0.07));
    }

    #[test]
    fn nft_deserialization() {
        let example_json = r#"
        {
            "id": "john-doe",
            "name": "John Doe",
            "description": "xd",
            "socials": [
                "aaa.aaa",
                "bbb.bbb"
            ],
            "goalTreasuryAmount": null,
            "ownerPubkey": "95b225CEtMmkRYUpg626DNqen55FgwEGbH5NKVXHUejT",
            "asset": {
                "type": "Nft",
                "name": "aaa",
                "symbol": "AAA",
                "uri": "ipfs://nice/aaa",
                "isRepeating": false
            },
            "encorePeriod": 0,
            "cyclePeriod": 3600,
            "numberOfCycles": 20,
            "startTime": null,
            "minBid": 0.07
        }"#;

        let deserialized: FrontendAuctionConfig = serde_json::from_str(example_json).unwrap();
        assert_eq!(deserialized.base.id, "john-doe");
        match deserialized.extra.asset {
            FrontendTokenConfig::Nft { name, symbol, uri, is_repeating } => {
                assert_eq!(name, "aaa");
                assert_eq!(symbol, "AAA");
                assert_eq!(uri, "ipfs://nice/aaa");
                assert!(!is_repeating);
            }
            _ => panic!("should be NFT")
        }
        assert!(deserialized.extra.start_time.is_none());
        assert_eq!(deserialized.extra.min_bid, Some(0.07));
    }

    #[test]
    fn init_auction_conversion() {
        let owner = "4K3NiGuqYGqKQoUk6LrRQNPXrkp5i9qNG7KpyTvACemX".to_owned();
        let base_config = FrontendAuctionBaseConfig {
            id: "hello".to_owned(),
            name: "HEllO".to_owned(),
            owner_pubkey: owner.clone(),
            goal_treasury_amount: Some(150.0),
        };
        let asset = FrontendTokenConfig::Nft {
            name: "MyNft".to_owned(),
            symbol: "MNFT".to_owned(),
            uri: "ipfs://hello.asd".to_owned(),
            is_repeating: true,
        };
        let extra_config = FrontendAuctionConfigExtra {
            description: "lollerkopter".to_owned(),
            socials: vec![
                "hehe.com".to_owned(),
                "hehe.dc".to_owned(),
                "hehe.tg".to_owned(),
            ],
            asset,
            encore_period: Some(180),
            cycle_period: 3600,
            number_of_cycles: 10,
            start_time: None,
            min_bid: Some(0.5),
        };
        let frontend_auction_config = FrontendAuctionConfig {
            base: base_config,
            extra: extra_config,
        };
    
        let init_args = frontend_auction_config
            .into_initialize_auction_args()
            .unwrap();
    
        assert_eq!(&init_args.auction_owner_pubkey.to_string(), &owner);
        assert_eq!(
            agsol_gold_contract::utils::unpad_id(&init_args.auction_id),
            "hello"
        );
        assert_eq!(
            init_args.auction_description.description.contents(),
            "lollerkopter"
        );
        assert_eq!(
            init_args.auction_description.socials.contents()[0].contents(),
            "hehe.com"
        );
        assert_eq!(
            init_args.auction_description.socials.contents()[1].contents(),
            "hehe.dc"
        );
        assert_eq!(
            init_args.auction_description.socials.contents()[2].contents(),
            "hehe.tg"
        );
        assert_eq!(
            init_args.auction_description.goal_treasury_amount,
            Some(150_000_000_000)
        );
        assert_eq!(init_args.auction_config.minimum_bid_amount, 500_000_000);
        match init_args.create_token_args {
            CreateTokenArgs::Nft {
                metadata_args,
                is_repeating,
            } => {
                let creators = metadata_args.data.creators.unwrap();
                assert!(is_repeating);
                assert_eq!(metadata_args.data.name, "MyNft");
                assert_eq!(metadata_args.data.symbol, "MNFT");
                assert_eq!(metadata_args.data.uri, "ipfs://hello.asd");
                assert!(metadata_args.is_mutable);
                assert_eq!(
                    metadata_args.data.seller_fee_basis_points,
                    SELLER_FEE_BASIS_POINTS
                );
                assert_eq!(&creators[0].address.to_string(), &owner);
            }
            _ => panic!("should be NFT"),
        }
    }
}
