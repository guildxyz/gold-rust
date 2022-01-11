use crate::{pad_to_32_bytes, NET};
use agsol_gold_contract::pda::get_auction_root_state_seeds;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::state::AuctionRootState;
use agsol_gold_contract::ID as GOLD_ID;
use agsol_wasm_client::RpcClient;

pub async fn get_current_cycle(auction_id: String) -> Result<u64, anyhow::Error> {
    let mut client = RpcClient::new(NET);
    let auction_id = pad_to_32_bytes(&auction_id)?;
    let (root_state_pubkey, _) =
        Pubkey::find_program_address(&get_auction_root_state_seeds(&auction_id), &GOLD_ID);

    let root_state: AuctionRootState = client
        .get_and_deserialize_account_data(&root_state_pubkey)
        .await?;

    Ok(root_state.status.current_auction_cycle)
}

#[cfg(test)]
mod test {
    use super::get_current_cycle;
    #[tokio::test]
    async fn get_current_cycle_test() {
        let result = get_current_cycle("goldxyz-dao".to_string()).await;
        println!("{:?}", result);
    }
}
