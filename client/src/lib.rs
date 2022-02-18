// This is necessary because clippy throws 'unneeded unit expression' error
// on the wasm_bindgen expressions
#![allow(clippy::unused_unit)]

mod get_auction;
mod get_current_cycle;
mod get_top_bidder;
mod is_id_unique;
mod utils;

use agsol_gold_contract::instruction::factory::*;
use agsol_gold_contract::pda::{
    auction_pool_seeds, auction_root_state_seeds, secondary_pool_seeds,
};
use agsol_gold_contract::solana_program;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::utils::pad_to_32_bytes;
use agsol_gold_contract::ID as GOLD_ID;
use agsol_wasm_client::rpc_config::{CommitmentLevel, Encoding, RpcConfig};
use agsol_wasm_client::{wasm_instruction, Net, RpcClient};
use borsh::BorshSerialize;
use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;

#[cfg(not(feature = "mainnet"))]
const NET: Net = Net::Devnet;
#[cfg(feature = "mainnet")]
const NET: Net = Net::Mainnet;

const RPC_CONFIG: RpcConfig = RpcConfig {
    encoding: Some(Encoding::JsonParsed),
    commitment: Some(CommitmentLevel::Processed),
};

#[cfg(test)]
const TEST_AUCTION_ID: &str = "teletubbies";

wasm_instruction!(initialize_auction);
wasm_instruction!(delete_auction);
wasm_instruction!(place_bid);
wasm_instruction!(claim_funds);

#[wasm_bindgen(js_name = "getAuctionWasm")]
pub async fn get_auction_wasm(auction_id: String) -> Result<Uint8Array, JsValue> {
    let id = pad_to_32_bytes(&auction_id).map_err(|e| JsValue::from(e.to_string()))?;
    let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
    let auction = get_auction::get_auction(&mut client, &id)
        .await
        .map_err(|e| JsValue::from(e.to_string()))?;

    Ok(Uint8Array::from(auction.try_to_vec().unwrap().as_slice()))
}

#[wasm_bindgen(js_name = "getAuctionCycleStateWasm")]
pub async fn get_auction_cycle_state_wasm(
    root_state_pubkey: Pubkey,
    cycle_num: u64,
) -> Result<Uint8Array, JsValue> {
    let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
    let auction_cycle_state =
        get_auction::get_auction_cycle_state(&mut client, &root_state_pubkey, cycle_num)
            .await
            .map_err(|e| JsValue::from(e.to_string()))?;

    Ok(Uint8Array::from(
        auction_cycle_state.try_to_vec().unwrap().as_slice(),
    ))
}

#[wasm_bindgen(js_name = "getTopBidderPubkeyWasm")]
pub async fn get_top_bidder_pubkey_wasm(auction_id: String) -> Result<Pubkey, JsValue> {
    let id = pad_to_32_bytes(&auction_id).map_err(|e| JsValue::from(e.to_string()))?;
    let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
    get_top_bidder::get_top_bidder(&mut client, &id)
        .await
        .map_err(|e| JsValue::from(e.to_string()))
}

#[wasm_bindgen(js_name = "getCurrentCycleWasm")]
pub async fn get_current_cycle_wasm(auction_id: String) -> Result<u64, JsValue> {
    let id = pad_to_32_bytes(&auction_id).map_err(|e| JsValue::from(e.to_string()))?;
    let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
    get_current_cycle::get_current_cycle(&mut client, &id)
        .await
        .map_err(|e| JsValue::from(e.to_string()))
}

#[wasm_bindgen(js_name = "getAuctionPoolPubkeyWasm")]
pub fn auction_pool_pubkey_wasm(secondary: bool) -> Pubkey {
    let seeds = if secondary {
        secondary_pool_seeds()
    } else {
        auction_pool_seeds()
    };
    let (auction_pool_pubkey, _) = Pubkey::find_program_address(&seeds, &GOLD_ID);
    auction_pool_pubkey
}

#[wasm_bindgen(js_name = "getAuctionRootStatePubkeyWasm")]
pub fn auction_root_state_pubkey_wasm(auction_id: &[u8]) -> Pubkey {
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(auction_id), &GOLD_ID);
    auction_root_state_pubkey
}

#[wasm_bindgen(js_name = "isIdUniqueWasm")]
pub async fn is_id_unique_wasm(auction_id: String) -> Result<bool, JsValue> {
    let mut client = RpcClient::new_with_config(NET, RPC_CONFIG);
    let auction_id = pad_to_32_bytes(&auction_id).map_err(|e| JsValue::from(e.to_string()))?;
    is_id_unique::is_id_unique(&mut client, &auction_id)
        .await
        .map_err(|e| JsValue::from(e.to_string()))
}

#[wasm_bindgen(js_name = "getNetWasm")]
pub fn get_net_wasm() -> String {
    NET.to_url().to_owned()
}
