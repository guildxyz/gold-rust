mod admin_withdraw;
mod claim_funds;
mod claim_rewards;
mod close_auction_cycle;
mod delete_auction;
mod filter_auction;
mod initialize_auction;
mod initialize_contract;
mod modify_auction;
mod place_bid;
mod reallocate_pool;
mod set_protocol_fee;
mod verify_auction;

pub use admin_withdraw::*;
pub use claim_funds::*;
pub use claim_rewards::*;
pub use close_auction_cycle::*;
pub use delete_auction::*;
pub use filter_auction::*;
pub use initialize_auction::*;
pub use initialize_contract::*;
pub use modify_auction::*;
pub use place_bid::*;
pub use reallocate_pool::*;
pub use set_protocol_fee::*;
pub use verify_auction::*;

use super::AuctionInstruction;
use crate::frontend::{to_lamports, Scalar};
use crate::pda::*;
use crate::state::*;
use crate::utils::pad_to_32_bytes;
use agsol_borsh_schema::BorshSchema;
use agsol_token_metadata::instruction::CreateMetadataAccountArgs;
use agsol_token_metadata::state::EDITION_MARKER_BIT_SIZE;
use agsol_token_metadata::ID as META_ID;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::Deserialize;
use solana_program::clock::UnixTimestamp;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::pubkey::Pubkey;
use solana_program::system_program::ID as SYS_ID;
use solana_program::sysvar::rent::ID as RENT_ID;
use spl_token::ID as TOKEN_ID;

use std::str::FromStr;

#[derive(BorshSchema, BorshDeserialize, BorshSerialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TokenType {
    Nft,
    Token,
}
