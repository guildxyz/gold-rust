mod types;
pub use types::*;

pub type Scalar = f64;
pub const SELLER_FEE_BASIS_POINTS: u16 = 50;
const LAMPORTS: Scalar = 1e9;

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
