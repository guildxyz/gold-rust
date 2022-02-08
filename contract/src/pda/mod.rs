pub mod factory;

use agsol_token_metadata::state::{EDITION, PREFIX};
use solana_program::pubkey::Pubkey;

pub fn contract_pda_seeds<'a>() -> [&'a [u8]; 1] {
    [b"gold_contract"]
}

pub fn auction_pool_seeds<'a>() -> [&'a [u8]; 1] {
    [b"gold_auction_pool"]
}

pub fn secondary_pool_seeds<'a>() -> [&'a [u8]; 1] {
    [b"gold_secondary_pool"]
}

pub fn auction_bank_seeds(auction_id: &[u8]) -> [&[u8]; 2] {
    [b"gold_auction_bank", auction_id]
}

pub fn auction_root_state_seeds(auction_id: &[u8]) -> [&[u8]; 2] {
    [b"gold_auction_root_state", auction_id]
}

pub fn auction_cycle_state_seeds<'a>(
    auction_root_state_pubkey: &'a Pubkey,
    cycle_number_bytes: &'a [u8],
) -> [&'a [u8]; 3] {
    [
        b"gold_auction_cycle_state",
        auction_root_state_pubkey.as_ref(),
        cycle_number_bytes,
    ]
}

pub fn contract_bank_seeds<'a>() -> [&'a [u8]; 1] {
    [b"gold_contract_bank"]
}

pub fn token_mint_seeds(auction_id: &[u8]) -> [&[u8]; 2] {
    [b"gold_token_mint", auction_id]
}
pub fn token_holding_seeds<'a>(mint: &'a Pubkey, user: &'a Pubkey) -> [&'a [u8]; 3] {
    [b"gold_token_holding", mint.as_ref(), user.as_ref()]
}

pub fn master_mint_seeds(auction_id: &[u8]) -> [&[u8]; 2] {
    [b"gold_master_mint", auction_id]
}

pub fn master_holding_seeds(auction_id: &[u8]) -> [&[u8]; 2] {
    [b"gold_master_holding", auction_id]
}

pub fn edition_seeds(mint_pubkey: &Pubkey) -> [&[u8]; 4] {
    [
        PREFIX.as_bytes(),
        agsol_token_metadata::ID.as_ref(),
        mint_pubkey.as_ref(),
        EDITION.as_bytes(),
    ]
}

pub fn user_asset_seeds<'a>(
    auction_id: &'a [u8],
    user_pubkey: &'a Pubkey,
    mint_pubkey: &'a Pubkey,
) -> [&'a [u8]; 4] {
    [
        b"gold_user_asset",
        auction_id,
        user_pubkey.as_ref(),
        mint_pubkey.as_ref(),
    ]
}

pub fn auction_mint_seeds(auction_id: &[u8]) -> [&[u8]; 2] {
    [b"gold_auction_mint", auction_id]
}

pub fn child_mint_seeds<'a>(edition: &'a [u8; 8], auction_id: &'a [u8]) -> [&'a [u8]; 3] {
    [b"gold_child_mint", auction_id, edition]
}

pub fn child_holding_seeds<'a>(edition: &'a [u8; 8], auction_id: &'a [u8]) -> [&'a [u8]; 3] {
    [b"gold_child_holding", auction_id, edition]
}

pub fn edition_marker_seeds<'a>(edition_str: &'a str, mint: &'a Pubkey) -> [&'a [u8]; 5] {
    [
        PREFIX.as_bytes(),
        agsol_token_metadata::ID.as_ref(),
        mint.as_ref(),
        EDITION.as_bytes(),
        edition_str.as_bytes(),
    ]
}

pub fn user_asset_pubkey(
    auction_id: &[u8],
    user_pubkey: &Pubkey,
    mint_pubkey: &Pubkey,
    program_id: &Pubkey,
) -> Pubkey {
    let (user_asset_pubkey, _bump_seed) = Pubkey::find_program_address(
        &user_asset_seeds(auction_id, user_pubkey, mint_pubkey),
        program_id,
    );
    user_asset_pubkey
}

pub fn metadata_seeds(mint_pubkey: &Pubkey) -> [&[u8]; 3] {
    [
        agsol_token_metadata::state::PREFIX.as_bytes(),
        agsol_token_metadata::ID.as_ref(),
        mint_pubkey.as_ref(),
    ]
}

pub enum EditionType {
    Master,
    Child(u64),
}

#[derive(Debug)]
pub struct EditionPda {
    pub edition: Pubkey,
    pub mint: Pubkey,
    pub holding: Pubkey,
    pub metadata: Pubkey,
}

impl EditionPda {
    pub fn new(edition_type: EditionType, auction_id: &[u8]) -> Self {
        let (mint, holding) = match edition_type {
            EditionType::Master => {
                let (mint, _) =
                    Pubkey::find_program_address(&master_mint_seeds(auction_id), &crate::ID);
                let (holding, _) =
                    Pubkey::find_program_address(&master_holding_seeds(auction_id), &crate::ID);
                (mint, holding)
            }
            EditionType::Child(next_edition) => {
                let (mint, _) = Pubkey::find_program_address(
                    &child_mint_seeds(&next_edition.to_le_bytes(), auction_id),
                    &crate::ID,
                );
                let (holding, _) = Pubkey::find_program_address(
                    &child_holding_seeds(&next_edition.to_le_bytes(), auction_id),
                    &crate::ID,
                );
                (mint, holding)
            }
        };
        let (metadata, _) =
            Pubkey::find_program_address(&metadata_seeds(&mint), &agsol_token_metadata::ID);
        let (edition, _) =
            Pubkey::find_program_address(&edition_seeds(&mint), &agsol_token_metadata::ID);
        Self {
            edition,
            mint,
            holding,
            metadata,
        }
    }
}
