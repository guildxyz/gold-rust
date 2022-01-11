mod get_auction;
mod get_current_cycle;
mod get_top_bidder;
mod get_treasury;

use agsol_gold_contract::instruction::factory::*;
use agsol_gold_contract::pda::get_auction_pool_seeds;
use agsol_gold_contract::solana_program;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::ID as GOLD_ID;
use agsol_wasm_client::{wasm_instruction, Net};
use anyhow::anyhow;
use borsh::BorshSerialize;
use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;

// TODO client net from env-var
const NET: Net = Net::Devnet;

wasm_instruction!(initialize_auction);
wasm_instruction!(freeze_auction);
wasm_instruction!(place_bid);
wasm_instruction!(claim_funds);
wasm_instruction!(delete_auction);
wasm_instruction!(initialize_contract);

#[wasm_bindgen(js_name = "getAuctionWasm")]
pub async fn get_auction_wasm(
    auction_id: String,
    cycle: Option<u64>,
) -> Result<Uint8Array, JsValue> {
    let frontend_auction = get_auction::get_auction(auction_id, cycle)
        .await
        .map_err(|e| JsValue::from(e.to_string()))?;

    Ok(Uint8Array::from(
        frontend_auction.try_to_vec().unwrap().as_slice(),
    ))
}

#[wasm_bindgen(js_name = "getTopBidderWasm")]
pub async fn get_top_bidder_wasm(auction_id: String) -> Result<Pubkey, JsValue> {
    get_top_bidder::get_top_bidder(auction_id)
        .await
        .map_err(|e| JsValue::from(e.to_string()))
}

#[wasm_bindgen(js_name = "getTreasuryWasm")]
pub async fn get_treasury_wasm(auction_id: String) -> Result<u64, JsValue> {
    get_treasury::get_treasury(auction_id)
        .await
        .map_err(|e| JsValue::from(e.to_string()))
}

#[wasm_bindgen(js_name = "getCurrentCycleWasm")]
pub async fn get_current_cycle_wasm(auction_id: String) -> Result<u64, JsValue> {
    get_current_cycle::get_current_cycle(auction_id)
        .await
        .map_err(|e| JsValue::from(e.to_string()))
}

#[wasm_bindgen(js_name = "getAuctionPoolPubkeyWasm")]
pub fn wasm_auction_pool_pubkey() -> Vec<u8> {
    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&get_auction_pool_seeds(), &GOLD_ID);
    auction_pool_pubkey.try_to_vec().unwrap()
}

// NOTE special characters are chopped off to fit an u8, so it won't be
// correct, however, we may assume in this case that the input is valid.
// Else, we will throw an error when the auction with this id is not found
fn pad_to_32_bytes(input: &str) -> Result<[u8; 32], anyhow::Error> {
    if input.len() > 32 {
        return Err(anyhow!("input is longer than 32 bytes"));
    }
    let mut array = [0_u8; 32];
    for (i, c) in input.chars().enumerate() {
        array[i] = c as u8;
    }
    Ok(array)
}

#[test]
fn str_padding() {
    assert_eq!(
        pad_to_32_bytes("this is definitely longer than 32 bytes")
            .err()
            .unwrap()
            .to_string(),
        "input is longer than 32 bytes"
    );
    assert_eq!(
        pad_to_32_bytes("this-is-fine").unwrap(),
        [
            0x74, 0x68, 0x69, 0x73, 0x2d, 0x69, 0x73, 0x2d, 0x66, 0x69, 0x6e, 0x65, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]
    );
    assert_eq!(
        pad_to_32_bytes("hélló").unwrap(),
        [
            0x68, 0xe9, 0x6c, 0x6c, 0xf3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
        ]
    );
}
