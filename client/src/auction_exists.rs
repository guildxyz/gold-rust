use agsol_gold_contract::pda::master_mint_seeds;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::state::AuctionId;
use agsol_gold_contract::ID as GOLD_ID;
use agsol_wasm_client::RpcClient;

pub async fn account_exists(
    client: &mut RpcClient,
    pubkey: &Pubkey,
) -> Result<bool, anyhow::Error> {
    let balance = client.get_balance(pubkey).await?;
    Ok(balance != 0)
}

pub async fn auction_exists(
    client: &mut RpcClient,
    auction_id: &AuctionId,
) -> Result<bool, anyhow::Error> {
    // read master mint state
    let (master_mint_pubkey, _) =
        Pubkey::find_program_address(&master_mint_seeds(auction_id), &GOLD_ID);

    account_exists(client, &master_mint_pubkey).await
}

#[cfg(test)]
mod test {
    use super::{auction_exists, RpcClient};
    use crate::{pad_to_32_bytes, NET, RPC_CONFIG, TEST_AUCTION_ID};

    #[tokio::test]
    async fn check_master_exists() {
        let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
        // this id will probably never be used
        let id = pad_to_32_bytes("__rtghsfwerlakdf*~!").unwrap();
        let exists = auction_exists(&mut client, &id).await.unwrap();
        assert!(!exists);
        let id = pad_to_32_bytes(TEST_AUCTION_ID).unwrap();
        let exists = auction_exists(&mut client, &id).await.unwrap();
        assert!(exists);
    }
}
