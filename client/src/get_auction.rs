use crate::{NET, RPC_CONFIG};
use anyhow::bail;
use agsol_common::MaxLenString;
use agsol_gold_contract::frontend::*;
use agsol_gold_contract::pda::*;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::state::{AuctionCycleState, AuctionRootState, TokenConfig};
use agsol_gold_contract::utils::{pad_to_32_bytes, unpuff_metadata};
use agsol_gold_contract::ID as GOLD_ID;
use agsol_token_metadata::state::Metadata;
use agsol_token_metadata::ID as META_ID;
use agsol_wasm_client::account::TokenAccount;
use agsol_wasm_client::RpcClient;
use std::convert::TryFrom;

pub async fn get_auction(auction_id: String) -> Result<FrontendAuction, anyhow::Error> {
    let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
    let auction_id = pad_to_32_bytes(&auction_id).map_err(anyhow::Error::msg)?;

    // read root state
    let (root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &GOLD_ID);

    let root_state: AuctionRootState = client
        .get_and_deserialize_account_data(&root_state_pubkey)
        .await?;

    let token_config = match root_state.token_config {
        TokenConfig::Nft(ref data) => {
            let (master_mint_pubkey, _) =
                Pubkey::find_program_address(&master_mint_seeds(&auction_id), &GOLD_ID);
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
    root_state_pubkey: &Pubkey,
    cycle_num: u64,
) -> Result<AuctionCycleState, anyhow::Error> {
    // read cycle state
    anyhow::ensure!(cycle_num != 0);
    let mut client = RpcClient::new(NET);
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
    #[tokio::test]
    async fn query_auction() {
        let auction_result = get_auction("goldxyz-dao".to_string()).await;
        println!("{:#?}", auction_result);
        if let Ok(auction) = auction_result {
            let cycle_result = get_auction_cycle_state(&auction.root_state_pubkey, 1).await;
            println!("{:#?}", cycle_result);
        }
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
