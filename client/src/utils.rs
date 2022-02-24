use crate::Scalar;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_wasm_client::RpcClient;

const LAMPORTS: Scalar = 1e9;

pub async fn account_exists(
    client: &mut RpcClient,
    pubkey: &Pubkey,
) -> Result<bool, anyhow::Error> {
    let balance = client.get_balance(pubkey).await?;
    Ok(balance != 0)
}

pub fn to_sol(amount: u64) -> Scalar {
    amount as Scalar / LAMPORTS
}

pub fn to_lamports(amount: Scalar) -> u64 {
    (amount * LAMPORTS) as u64
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
