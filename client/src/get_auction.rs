use agsol_common::MaxLenString;
use agsol_gold_contract::frontend::*;
use agsol_gold_contract::pda::*;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::state::{AuctionCycleState, AuctionId, AuctionRootState, TokenConfig};
use agsol_gold_contract::utils::unpuff_metadata;
use agsol_gold_contract::ID as GOLD_ID;
use agsol_token_metadata::state::Metadata;
use agsol_token_metadata::ID as META_ID;
use agsol_wasm_client::account::TokenAccount;
use agsol_wasm_client::RpcClient;
use anyhow::bail;
use std::convert::TryFrom;

pub async fn get_auction(
    client: &mut RpcClient,
    auction_id: &AuctionId,
) -> Result<FrontendAuction, anyhow::Error> {
    // read root state
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
                name: MaxLenString::try_from(metadata.data.name).unwrap(),
                symbol: MaxLenString::try_from(metadata.data.symbol).unwrap(),
                uri: MaxLenString::try_from(metadata.data.uri).unwrap(),
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
    use super::{get_auction, get_auction_cycle_state, strip_uri, RpcClient};
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
