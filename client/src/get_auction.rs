use crate::types::*;
use crate::utils::*;
use agsol_gold_contract::pda::*;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::state::*;
use agsol_gold_contract::utils::{unpad_id, unpuff_metadata};
use agsol_gold_contract::ID as GOLD_ID;
use agsol_token_metadata::state::Metadata;
use agsol_token_metadata::ID as META_ID;
use agsol_wasm_client::account::TokenAccount;
use agsol_wasm_client::RpcClient;
use anyhow::bail;
use futures::stream::{self, StreamExt};

struct RootState {
    state: AuctionRootState,
    pubkey: Pubkey,
}

pub async fn get_auctions(
    client: &mut RpcClient,
    secondary: bool,
) -> Result<Vec<FrontendAuctionBase>, anyhow::Error> {
    let seeds = if secondary {
        secondary_pool_seeds()
    } else {
        auction_pool_seeds()
    };
    let (auction_pool_pubkey, _) = Pubkey::find_program_address(&seeds, &GOLD_ID);
    let auction_pool: AuctionPool = client
        .get_and_deserialize_account_data(&auction_pool_pubkey)
        .await?;

    let base_stream = stream::iter(auction_pool.pool.into_iter());
    let base_vec = base_stream
        .filter_map(|id| async move {
            let mut client = RpcClient::new_with_config(crate::NET, crate::RPC_CONFIG);
            if let Ok(root_state) = get_root_state(&mut client, &id).await {
                if root_state.state.status.is_filtered {
                    None
                } else {
                    Some(get_auction_base(&id, &root_state.state))
                }
            } else {
                None
            }
        })
        .collect::<Vec<FrontendAuctionBase>>()
        .await;
    Ok(base_vec)
}

pub async fn get_auction(
    client: &mut RpcClient,
    auction_id: &AuctionId,
) -> Result<FrontendAuction, anyhow::Error> {
    let RootState {
        state: root_state,
        pubkey: root_state_pubkey,
    } = get_root_state(client, auction_id).await?;
    if root_state.status.is_filtered {
        bail!("this auction is filtered")
    }
    let token_config = match root_state.token_config {
        TokenConfig::Nft(ref data) => {
            let (master_mint_pubkey, _) =
                Pubkey::find_program_address(&master_mint_seeds(auction_id), &GOLD_ID);
            let (metadata_pubkey, _) =
                Pubkey::find_program_address(&metadata_seeds(&master_mint_pubkey), &META_ID);
            let mut metadata: Metadata = client
                .get_and_deserialize_account_data(&metadata_pubkey)
                .await?;

            unpuff_metadata(&mut metadata.data);
            strip_uri(&mut metadata.data.uri);

            FrontendTokenConfig::Nft {
                name: metadata.data.name,
                symbol: metadata.data.symbol,
                uri: metadata.data.uri,
                is_repeating: data.is_repeating,
            }
        }
        TokenConfig::Token(ref data) => {
            // get mint metadata and decimals
            let mint_data = client
                .get_and_deserialize_parsed_account_data::<TokenAccount>(&data.mint)
                .await;

            match mint_data {
                Ok(TokenAccount::Mint(mint)) => FrontendTokenConfig::Token {
                    mint: Some(data.mint.to_string()),
                    decimals: mint.decimals,
                    per_cycle_amount: data.per_cycle_amount,
                },
                Ok(_) => bail!("not a mint account"),
                Err(e) => bail!("{}", e),
            }
        }
    };

    let base = get_auction_base(auction_id, &root_state);

    let socials: Vec<SocialsString> = root_state.description.socials.into();
    let config = FrontendAuctionConfigExtra {
        description: root_state.description.description.into(),
        socials: socials
            .into_iter()
            .map(|x| x.into())
            .collect::<Vec<String>>(),
        asset: token_config,
        encore_period: Some(root_state.auction_config.encore_period),
        cycle_period: root_state.auction_config.cycle_period,
        number_of_cycles: root_state
            .auction_config
            .number_of_cycles
            .unwrap_or_default(),
        start_time: Some(root_state.start_time),
        min_bid: Some(to_sol(root_state.auction_config.minimum_bid_amount)),
    };

    Ok(FrontendAuction {
        base,
        config,
        available_treasury_amount: to_sol(root_state.available_funds),
        current_cycle: root_state.status.current_auction_cycle,
        is_finished: root_state.status.is_finished,
        is_frozen: root_state.status.is_frozen,
        is_filtered: root_state.status.is_filtered,
        root_state_pubkey: root_state_pubkey.to_string(),
    })
}

pub async fn get_auction_cycle_state(
    client: &mut RpcClient,
    root_state_pubkey: &Pubkey,
    cycle_num: u64,
) -> Result<FrontendCycle, anyhow::Error> {
    anyhow::ensure!(cycle_num > 0);
    let (cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(root_state_pubkey, &cycle_num.to_le_bytes()),
        &GOLD_ID,
    );
    let cycle_state: AuctionCycleState = client
        .get_and_deserialize_account_data(&cycle_state_pubkey)
        .await?;

    let bid_history: Vec<BidData> = cycle_state.bid_history.into();
    let bids = bid_history
        .into_iter()
        .map(|bid| FrontendBid {
            bidder_pubkey: bid.bidder_pubkey.to_string(),
            amount: to_sol(bid.bid_amount),
        })
        .collect::<Vec<FrontendBid>>();
    Ok(FrontendCycle {
        bids,
        end_timestamp: cycle_state.end_time,
    })
}

fn get_auction_base(auction_id: &AuctionId, root_state: &AuctionRootState) -> FrontendAuctionBase {
    let base_config = FrontendAuctionBaseConfig {
        id: unpad_id(auction_id),
        name: unpad_id(&root_state.auction_name),
        owner_pubkey: root_state.auction_owner.to_string(),
        goal_treasury_amount: to_sol(
            root_state
                .description
                .goal_treasury_amount
                .unwrap_or_default(),
        ),
    };

    FrontendAuctionBase {
        config: base_config,
        all_time_treasury_amount: to_sol(root_state.all_time_treasury),
        is_verified: root_state.status.is_verified,
    }
}

async fn get_root_state(
    client: &mut RpcClient,
    id: &AuctionId,
) -> Result<RootState, anyhow::Error> {
    let (pubkey, _) = Pubkey::find_program_address(&auction_root_state_seeds(id), &GOLD_ID);

    let state: AuctionRootState = client.get_and_deserialize_account_data(&pubkey).await?;

    Ok(RootState { state, pubkey })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{pad_to_32_bytes, NET, RPC_CONFIG, TEST_AUCTION_ID};
    use std::str::FromStr;

    #[tokio::test]
    async fn query_auction() {
        // unwraps ensure that the accounts are properly deserialized
        let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
        let auction_id = pad_to_32_bytes(TEST_AUCTION_ID).unwrap();
        let auction = get_auction(&mut client, &auction_id).await.unwrap();
        let owner_pubkey = Pubkey::from_str(&auction.root_state_pubkey).unwrap();
        get_auction_cycle_state(&mut client, &owner_pubkey, 1)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn auction_base_array() {
        let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
        let x = get_auctions(&mut client, true).await.unwrap();
        println!("{:#?}", x);
        get_auctions(&mut client, false).await.unwrap();
    }
}
