use crate::{NET, RPC_CONFIG};
use agsol_gold_contract::pda::{auction_cycle_state_seeds, auction_root_state_seeds};
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::state::{AuctionCycleState, AuctionRootState};
use agsol_gold_contract::utils::pad_to_32_bytes;
use agsol_gold_contract::ID as GOLD_ID;
use agsol_wasm_client::RpcClient;

pub async fn get_top_bidder(auction_id: String) -> Result<Pubkey, anyhow::Error> {
    let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
    let auction_id = pad_to_32_bytes(&auction_id).map_err(anyhow::Error::msg)?;
    let (root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &GOLD_ID);

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

    if let Some(bid) = cycle_state.bid_history.get_last_element() {
        Ok(bid.bidder_pubkey)
    } else {
        Ok(Pubkey::default())
    }
}

#[cfg(test)]
mod test {
    use super::get_top_bidder;
    #[tokio::test]
    async fn top_bidder_test() {
        let result = get_top_bidder("goldxyz-dao".to_string()).await;
        println!("{:?}", result);
    }
}
