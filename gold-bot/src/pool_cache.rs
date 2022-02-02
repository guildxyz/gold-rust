use agsol_gold_contract::pda::{auction_cycle_state_seeds, auction_root_state_seeds};
use agsol_gold_contract::state::{AuctionCycleState, AuctionId, AuctionRootState};
use agsol_gold_contract::ID as GOLD_ID;
use agsol_wasm_client::RpcClient;
use solana_sdk::borsh::try_from_slice_unchecked;
use solana_sdk::clock::UnixTimestamp;
use solana_sdk::pubkey::Pubkey;

use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;

/// Contains the cached data of an auction
pub struct PoolRecord {
    /// The auction root state account's pubkey
    pub root_pubkey: Pubkey,
    /// The auction root state
    pub root_state: AuctionRootState,
    /// The auction cycle state
    pub cycle_state: AuctionCycleState,
    /// The number of times an unexpected error occured on consecutive cycle closings
    pub error_streak: u8,
}

impl PoolRecord {
    /// Initializes a pool record by loading the root and cycle state of the auction
    pub async fn new(client: &mut RpcClient, auction_id: &AuctionId) -> Result<Self, anyhow::Error> {
        let (root_pubkey, _) =
            Pubkey::find_program_address(&auction_root_state_seeds(auction_id), &GOLD_ID);
        let root_state_data = client.get_account_data(&root_pubkey).await?;
        let root_state: AuctionRootState = try_from_slice_unchecked(&root_state_data)?;

        let current_cycle_bytes = root_state.status.current_auction_cycle.to_le_bytes();
        let (cycle_pubkey, _) = Pubkey::find_program_address(
            &auction_cycle_state_seeds(&root_pubkey, &current_cycle_bytes),
            &GOLD_ID,
        );
        let cycle_state_data = client.get_account_data(&cycle_pubkey).await?;
        let cycle_state: AuctionCycleState = try_from_slice_unchecked(&cycle_state_data)?;

        Ok(Self {
            root_pubkey,
            root_state,
            cycle_state,
            error_streak: 0,
        })
    }

    /// Updates the stored root state
    pub async fn update_root_state(&mut self, client: &mut RpcClient) -> Result<(), anyhow::Error> {
        let root_state_data = client.get_account_data(&self.root_pubkey).await?;
        self.root_state = try_from_slice_unchecked(&root_state_data)?;
        Ok(())
    }

    /// Updates the stored cycle state
    pub async fn update_cycle_state(&mut self, client: &mut RpcClient) -> Result<(), anyhow::Error> {
        let current_cycle_bytes = self.root_state.status.current_auction_cycle.to_le_bytes();
        let (cycle_pubkey, _) = Pubkey::find_program_address(
            &auction_cycle_state_seeds(&self.root_pubkey, &current_cycle_bytes),
            &GOLD_ID,
        );
        let cycle_state_data = client.get_account_data(&cycle_pubkey).await?;
        self.cycle_state = try_from_slice_unchecked(&cycle_state_data)?;
        Ok(())
    }

    /// Logs error appropriately, if unexpected error occurs then increments error_streak
    ///
    /// Expected errors:
    ///
    ///  - Auction cycle was closed by other agent
    ///
    ///  - Bid triggered encore period which extended the cycle
    ///
    /// Both errors can be recognized by a difference in cycle end_times
    pub async fn report_error(&mut self, client: &mut RpcClient) -> Result<(), anyhow::Error> {
        let prev_end_time = self.cycle_state.end_time;
        self.update_root_state(client).await?;
        self.update_cycle_state(client).await?;

        if prev_end_time == self.cycle_state.end_time {
            self.error_streak += 1;
        }

        Ok(())
    }

    /// Resets error streak.
    /// Should be used after successful cycle closing.
    pub fn reset_error_streak(&mut self) {
        self.error_streak = 0;
    }

    /// Returns if the auction is likely broken.
    /// Currently identified by receiving 5+ consecutive errors on cycle closing
    pub fn is_faulty_auction(&self) -> bool {
        self.error_streak > 5
    }
}

type HashedPool = HashMap<AuctionId, PoolRecord>;
type HashedIdSet = HashSet<AuctionId>;

/// Manages auction states for caching
pub struct ManagedPool {
    /// Hashmap containing all auctions and their data
    pub hashed_pool: HashedPool,
    /// Hashset containing ids of inactive (frozen, filtered, finished) auctions
    pub inactive_auctions: HashedIdSet,
    /// Hashset containing ids of erroneous auctions
    pub error_auctions: HashedIdSet,
}

impl ManagedPool {
    pub fn new() -> Self {
        Self {
            hashed_pool: HashedPool::new(),
            inactive_auctions: HashedIdSet::new(),
            error_auctions: HashedIdSet::new(),
        }
    }

    /// Returns a mutable reference to a pool record if it is active
    ///
    ///  - Returns none if auction is not active (frozen, filtered, finished, erroneous)
    ///
    ///  - Returns none if auction cycle is not over yet
    pub async fn get_or_insert_auction(
        &mut self,
        connection: &mut RpcClient,
        auction_id: AuctionId,
        block_time: UnixTimestamp,
    ) -> Result<Option<&mut PoolRecord>, anyhow::Error> {
        // if previously identified as inactive or uncallable, return none
        if self.inactive_auctions.get(&auction_id).is_some()
            || self.error_auctions.get(&auction_id).is_some()
        {
            return Ok(None);
        }

        // fetch or insert pool record
        let pool_record = match self.hashed_pool.entry(auction_id) {
            Vacant(entry) => entry.insert(PoolRecord::new(connection, &auction_id).await?),
            Occupied(entry) => entry.into_mut(),
        };

        // if frozen or inactive or filtered, register it and return none
        if pool_record.root_state.status.is_frozen
            || pool_record.root_state.status.is_finished
            || pool_record.root_state.status.is_filtered
        {
            self.inactive_auctions.insert(auction_id);
            return Ok(None);
        }

        // if cycle not over yet, return none
        if block_time < pool_record.cycle_state.end_time {
            return Ok(None);
        }
        Ok(Some(pool_record))
    }
}
