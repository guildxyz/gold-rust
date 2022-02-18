use crate::utils::account_exists;
use agsol_gold_contract::pda::master_mint_seeds;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::state::AuctionId;
use agsol_gold_contract::ID as GOLD_ID;
use agsol_wasm_client::RpcClient;

pub async fn is_id_unique(
    client: &mut RpcClient,
    auction_id: &AuctionId,
) -> Result<bool, anyhow::Error> {
    // read master mint state
    let (master_mint_pubkey, _) =
        Pubkey::find_program_address(&master_mint_seeds(auction_id), &GOLD_ID);

    // if id exists, then it's not unique
    account_exists(client, &master_mint_pubkey)
        .await
        .map(|exists| !exists)
}

#[cfg(test)]
mod test {
    use super::{is_id_unique, RpcClient};
    use crate::{pad_to_32_bytes, NET, RPC_CONFIG, TEST_AUCTION_ID};

    #[tokio::test]
    async fn check_master_exists() {
        let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
        // this id will probably never be used
        let id = pad_to_32_bytes("__rtghsfwerlakdf*~!").unwrap();
        let unique = is_id_unique(&mut client, &id).await.unwrap();
        assert!(unique);
        let id = pad_to_32_bytes(TEST_AUCTION_ID).unwrap();
        let unique = is_id_unique(&mut client, &id).await.unwrap();
        assert!(!unique);
    }
}
