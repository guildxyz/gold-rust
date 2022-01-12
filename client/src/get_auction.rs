use crate::{pad_to_32_bytes, NET};
use agsol_common::MaxLenString;
use agsol_gold_contract::frontend::*;
use agsol_gold_contract::pda::*;
use agsol_gold_contract::solana_program::program_pack::Pack;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::state::{AuctionCycleState, AuctionPool, AuctionRootState, TokenConfig};
use agsol_gold_contract::unpuff_metadata;
use agsol_gold_contract::ID as GOLD_ID;
use agsol_wasm_client::RpcClient;
use anyhow::anyhow;
use metaplex_token_metadata::state::Metadata;
use metaplex_token_metadata::ID as META_ID;
use spl_token::state::Mint;
use std::convert::TryFrom;

pub async fn get_auction(auction_id: String) -> Result<FrontendAuction, anyhow::Error> {
    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&get_auction_pool_seeds(), &GOLD_ID);
    let mut client = RpcClient::new(NET);
    let auction_pool: AuctionPool = client
        .get_and_deserialize_account_data(&auction_pool_pubkey)
        .await?;

    let auction_id = pad_to_32_bytes(&auction_id)?;

    // read root state
    let root_state_pubkey = if let Some(pubkey) = auction_pool.pool.get(&auction_id) {
        pubkey
    } else {
        return Err(anyhow!("no auction found with this id"));
    };

    let root_state: AuctionRootState = client
        .get_and_deserialize_account_data(root_state_pubkey)
        .await?;

    let token_config = match root_state.token_config {
        TokenConfig::Nft(ref data) => {
            let (master_mint_pubkey, _) =
                Pubkey::find_program_address(&get_master_mint_seeds(&auction_id), &GOLD_ID);
            let (metadata_pubkey, _) =
                Pubkey::find_program_address(&get_metadata_seeds(&master_mint_pubkey), &META_ID);
            let mut metadata: Metadata = client
                .get_and_deserialize_account_data(&metadata_pubkey)
                .await?;

            unpuff_metadata(&mut metadata.data);

            FrontendTokenConfig::Nft {
                name: MaxLenString::try_from(metadata.data.name).unwrap(),
                symbol: MaxLenString::try_from(metadata.data.symbol).unwrap(),
                uri: MaxLenString::try_from(metadata.data.uri).unwrap(),
                is_repeating: data.is_repeating,
            }
        }
        TokenConfig::Token(ref data) => {
            // get mint metadata
            let mint_data = client.get_account_data(&data.mint).await?;
            // get decimals
            let mint = Mint::unpack_from_slice(&mint_data)?;

            FrontendTokenConfig::Token {
                mint: data.mint,
                decimals: mint.decimals,
                per_cycle_amount: data.per_cycle_amount,
            }
        }
    };

    let cycle_state =
        get_auction_cycle_state(root_state_pubkey, root_state.status.current_auction_cycle)
            .await?;

    let mut available_funds = get_treasury_without_rent(&mut client, &auction_id).await?;

    if let Some(bid_data) = cycle_state.bid_history.get_last_element() {
        available_funds = available_funds.saturating_sub(bid_data.bid_amount);
    }

    Ok(FrontendAuction {
        root_state_pubkey: *root_state_pubkey,
        root_state,
        token_config,
        available_funds,
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
        &get_auction_cycle_state_seeds(root_state_pubkey, &cycle_num.to_le_bytes()),
        &GOLD_ID,
    );
    client
        .get_and_deserialize_account_data(&cycle_state_pubkey)
        .await
}

pub async fn get_treasury_without_rent(
    client: &mut RpcClient,
    auction_id: &[u8],
) -> Result<u64, anyhow::Error> {
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&get_auction_bank_seeds(auction_id), &GOLD_ID);

    let bank_account = client.get_account(&auction_bank_pubkey).await?;
    let rent = client
        .get_minimum_balance_for_rent_exemption(bank_account.data.len())
        .await?;
    Ok(bank_account.lamports.saturating_sub(rent))
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
}
