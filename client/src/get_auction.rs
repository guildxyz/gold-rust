use crate::{pad_to_32_bytes, NET};
use agsol_common::MaxLenString;
use agsol_gold_contract::frontend::*;
use agsol_gold_contract::pda::*;
use agsol_gold_contract::solana_program::program_pack::Pack;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::state::{AuctionCycleState, AuctionRootState, TokenConfig};
use agsol_gold_contract::unpuff_metadata;
use agsol_gold_contract::ID as GOLD_ID;
use agsol_wasm_client::RpcClient;
use metaplex_token_metadata::state::Metadata;
use metaplex_token_metadata::ID as META_ID;
use spl_token::state::Mint;
use std::convert::TryFrom;

pub async fn get_auction(
    auction_id: String,
    cycle: Option<u64>,
) -> Result<FrontendAuction, anyhow::Error> {
    let mut client = RpcClient::new(NET);
    let auction_id = pad_to_32_bytes(&auction_id)?;

    // read root state
    let (root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &GOLD_ID);

    let root_state: AuctionRootState = client
        .get_and_deserialize_account_data(&root_state_pubkey)
        .await?;

    // cycle num for cycle state pda
    let cycle_num = if let Some(num) = cycle {
        num
    } else {
        root_state.status.current_auction_cycle
    };

    // read cycle state
    let (cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(&root_state_pubkey, &cycle_num.to_le_bytes()),
        &GOLD_ID,
    );
    let cycle_state: AuctionCycleState = client
        .get_and_deserialize_account_data(&cycle_state_pubkey)
        .await?;

    let token_config = match root_state.token_config {
        TokenConfig::Nft(ref data) => {
            let mint_pubkey = if cycle_num == root_state.status.current_auction_cycle {
                // get master mint
                let (master_mint_pubkey, _) =
                    Pubkey::find_program_address(&master_mint_seeds(&auction_id), &GOLD_ID);
                master_mint_pubkey
            } else {
                // get child mint
                let (child_mint_pubkey, _) = Pubkey::find_program_address(
                    &child_mint_seeds(&cycle_num.to_le_bytes(), &auction_id),
                    &GOLD_ID,
                );
                child_mint_pubkey
            };

            let (metadata_pubkey, _) =
                Pubkey::find_program_address(&metadata_seeds(&mint_pubkey), &META_ID);
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
    Ok(FrontendAuction {
        root_state,
        cycle_state,
        token_config,
    })
}

#[cfg(test)]
mod test {
    use super::get_auction;
    #[tokio::test]
    async fn query_auction() {
        let result = get_auction("goldxyz-dao".to_string(), Some(1)).await;
        println!("{:#?}", result);
    }
}
