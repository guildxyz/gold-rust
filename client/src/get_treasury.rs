use crate::{pad_to_32_bytes, NET};
use agsol_gold_contract::pda::get_auction_bank_seeds;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::ID as GOLD_ID;
use agsol_wasm_client::RpcClient;

pub async fn get_treasury(auction_id: String) -> Result<u64, anyhow::Error> {
    let mut client = RpcClient::new(NET);
    let auction_id = pad_to_32_bytes(&auction_id)?;
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&get_auction_bank_seeds(&auction_id), &GOLD_ID);

    let bank_account = client.get_account(&auction_bank_pubkey).await?;
    let rent = client
        .get_minimum_balance_for_rent_exemption(bank_account.data.len())
        .await?;
    Ok(bank_account.lamports.saturating_sub(rent))
}

#[cfg(test)]
mod test {
    use super::get_treasury;
    #[tokio::test]
    async fn get_treasury_test() {
        let result = get_treasury("goldxyz-dao".to_string()).await;
        println!("{:?}", result);
    }
}
