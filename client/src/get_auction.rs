use agsol_gold_contract::frontend::*;
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

const LAMPORTS: f32 = 1e9;

async fn get_auction_base(
    client: &mut RpcClient,
    id: &AuctionId,
    root_state_pubkey: &Pubkey,
) -> Result<FrontendAuctionBase, anyhow::Error> {
    let root_state: AuctionRootState = client
        .get_and_deserialize_account_data(root_state_pubkey)
        .await?;

    if root_state.status.is_filtered {
        bail!("this auction is filtered")
    }

    let goal_treasury_amount = if let Some(amount) = root_state.description.goal_treasury_amount {
        (amount as f32 / LAMPORTS).to_string()
    } else {
        "0".to_owned()
    };
    let all_time_treasury_amount = (root_state.all_time_treasury as f32 / LAMPORTS).to_string();
    Ok(FrontendAuctionBase {
        id: unpad_id(id),
        name: unpad_id(&root_state.auction_name),
        owner: root_state.auction_owner.to_string(),
        goal_treasury_amount,
        all_time_treasury_amount,
        is_verified: root_state.status.is_verified,
    })
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
    let mut auction_base_vec = Vec::with_capacity(auction_pool.pool.len());
    for id in &auction_pool.pool {
        let (root_state_pubkey, _) =
            Pubkey::find_program_address(&auction_root_state_seeds(id), &GOLD_ID);
        if let Ok(auction_base) = get_auction_base(client, id, &root_state_pubkey).await {
            auction_base_vec.push(auction_base);
        } // else we are skipping stuff
    }
    Ok(auction_base_vec)
}

pub async fn get_auction(
    client: &mut RpcClient,
    auction_id: &AuctionId,
) -> Result<FrontendAuction, anyhow::Error> {
    let (root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(auction_id), &GOLD_ID);

    let root_state: AuctionRootState = client
        .get_and_deserialize_account_data(&root_state_pubkey)
        .await?;
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
                    mint: data.mint,
                    decimals: mint.decimals,
                    per_cycle_amount: data.per_cycle_amount,
                },
                Ok(_) => bail!("not a mint account"),
                Err(e) => bail!("{}", e),
            }
        }
    };

    Ok(FrontendAuction {
        root_state_pubkey,
        root_state,
        token_config,
    })
}

pub async fn get_auction_cycle_state(
    client: &mut RpcClient,
    root_state_pubkey: &Pubkey,
    cycle_num: u64,
) -> Result<AuctionCycleState, anyhow::Error> {
    anyhow::ensure!(cycle_num != 0);
    let (cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(root_state_pubkey, &cycle_num.to_le_bytes()),
        &GOLD_ID,
    );
    client
        .get_and_deserialize_account_data(&cycle_state_pubkey)
        .await
}

fn strip_uri(uri: &mut String) {
    if let Some(index) = uri.rfind('/') {
        uri.drain(index..);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{pad_to_32_bytes, NET, RPC_CONFIG, TEST_AUCTION_ID};

    #[tokio::test]
    async fn query_auction() {
        // unwraps ensure that the accounts are properly deserialized
        let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
        let auction_id = pad_to_32_bytes(TEST_AUCTION_ID).unwrap();
        let auction = get_auction(&mut client, &auction_id).await.unwrap();
        get_auction_cycle_state(&mut client, &auction.root_state_pubkey, 1)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn auction_base_array() {
        let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
        get_auctions(&mut client, true).await.unwrap();
        get_auctions(&mut client, false).await.unwrap();
    }

    #[test]
    fn strip_uri_test() {
        let mut uri = "https://hello/this-is-a-dir/file.json".to_string();
        strip_uri(&mut uri);
        assert_eq!(uri, "https://hello/this-is-a-dir");
        let mut uri = "https://hello/this-is-a-dir/0/file.json".to_string();
        strip_uri(&mut uri);
        assert_eq!(uri, "https://hello/this-is-a-dir/0");
        strip_uri(&mut uri);
        assert_eq!(uri, "https://hello/this-is-a-dir");
    }
}
