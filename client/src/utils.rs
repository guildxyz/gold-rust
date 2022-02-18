use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_wasm_client::RpcClient;

pub async fn account_exists(
    client: &mut RpcClient,
    pubkey: &Pubkey,
) -> Result<bool, anyhow::Error> {
    let balance = client.get_balance(pubkey).await?;
    Ok(balance != 0)
}
