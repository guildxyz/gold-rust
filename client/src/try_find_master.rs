use crate::{NET, RPC_CONFIG};
use agsol_gold_contract::pda::master_mint_seeds;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::utils::pad_to_32_bytes;
use agsol_gold_contract::ID as GOLD_ID;
use agsol_wasm_client::RpcClient;
use agsol_wasm_client::token_account::EncodedMint;

pub async fn try_find_master(auction_id: String) -> Result<(), anyhow::Error> {
    let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
    let auction_id = pad_to_32_bytes(&auction_id).map_err(anyhow::Error::msg)?;

    // read master mint state
    let (master_mint_pubkey, _) =
        Pubkey::find_program_address(&master_mint_seeds(&auction_id), &GOLD_ID);

    let acc = client.get_and_deserialize_parsed_account_data::<EncodedMint>(&master_mint_pubkey).await?;
    println!("{:?}", acc);
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    #[tokio::test]
    async fn check_master_exists() {
        // this id will probably never be used
        //let find_master = try_find_master("__rtghsfwerlakdf*~!".to_string()).await;
        //assert!(find_master.is_err());
        let find_master = try_find_master("teletubbies".to_string()).await;
        println!("{:?}", find_master);
        assert!(find_master.is_ok());
    }
}
