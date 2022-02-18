use agsol_gold_contract::pda::{auction_cycle_state_seeds, auction_root_state_seeds};
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::state::{AuctionCycleState, AuctionId, AuctionRootState};
use agsol_gold_contract::ID as GOLD_ID;
use agsol_wasm_client::RpcClient;

pub async fn get_top_bidder(
    client: &mut RpcClient,
    auction_id: &AuctionId,
) -> Result<Pubkey, anyhow::Error> {
    let (root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(auction_id), &GOLD_ID);

    let root_state: AuctionRootState = client
        .get_and_deserialize_account_data(&root_state_pubkey)
        .await?;

    let current_cycle = root_state.status.current_auction_cycle;

    let (cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(&root_state_pubkey, &current_cycle.to_le_bytes()),
        &GOLD_ID,
    );

    let cycle_state: AuctionCycleState = client
        .get_and_deserialize_account_data(&cycle_state_pubkey)
        .await?;

    if let Some(bid_data) = cycle_state.bid_history.get_last_element() {
        Ok(bid_data.bidder_pubkey)
    } else {
        Ok(Pubkey::default())
    }
}

#[cfg(test)]
mod test {
    use super::{get_top_bidder, RpcClient};
    use crate::{pad_to_32_bytes, NET, RPC_CONFIG, TEST_AUCTION_ID};
    #[tokio::test]
    async fn top_bidder_test() {
        let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
        let auction_id = pad_to_32_bytes(TEST_AUCTION_ID).unwrap();
        let result = get_top_bidder(&mut client, &auction_id).await;
        assert!(result.is_ok());
    }
}
