use crate::LAMPORTS;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_wasm_client::RpcClient;

pub async fn account_exists(
    client: &mut RpcClient,
    pubkey: &Pubkey,
) -> Result<bool, anyhow::Error> {
    let balance = client.get_balance(pubkey).await?;
    Ok(balance != 0)
}

pub fn to_ui_amount(amount: u64) -> f32 {
    amount as f32 / LAMPORTS
}

pub fn strip_uri(uri: &mut String) {
    if let Some(index) = uri.rfind('/') {
        uri.drain(index..);
    }
}

#[test]
fn strip_uri_test() {
    let mut uri = "https://hello/this-is-a-dir/file.json".to_string();
    strip_uri(&mut uri);
    assert_eq!(uri, "https://hello/this-is-a-dir");
    let mut uri = "https://hello/this-is-a-dir/0/file.json".to_string();
    strip_uri(&mut uri);
    assert_eq!(uri, "https://hello/this-is-a-dir/0");
    strip_uri(&mut uri);
    assert_eq!(uri, "https://hello/this-is-a-dir");
}
