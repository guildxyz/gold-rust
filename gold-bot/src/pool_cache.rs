use agsol_gold_contract::pda::{auction_cycle_state_seeds, auction_root_state_seeds};
use agsol_gold_contract::state::{AuctionCycleState, AuctionId, AuctionRootState};
use agsol_gold_contract::ID as GOLD_ID;

use solana_client::rpc_client::RpcClient;
use solana_sdk::borsh::try_from_slice_unchecked;
use solana_sdk::clock::UnixTimestamp;
use solana_sdk::pubkey::Pubkey;

use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;

// contains the cached data of an auction
pub struct PoolRecord {
    // the auction root state account's pubkey
    pub root_pubkey: Pubkey,
    // the auction root state
    pub root_state: AuctionRootState,
    // the auction cycle state
    pub cycle_state: AuctionCycleState,
    // the number of times an unexpected error occured on consecutive cycle closings
    pub error_streak: u8,
}

impl PoolRecord {
    // initializes a pool record by loading the root and cycle state of the auction
    pub fn new(connection: &RpcClient, auction_id: &AuctionId) -> Result<Self, anyhow::Error> {
        let (root_pubkey, _) =
            Pubkey::find_program_address(&auction_root_state_seeds(auction_id), &GOLD_ID);
        let root_state_data = connection.get_account_data(&root_pubkey)?;
        let root_state: AuctionRootState = try_from_slice_unchecked(&root_state_data)?;

        let current_cycle_bytes = root_state.status.current_auction_cycle.to_le_bytes();
        let (cycle_pubkey, _) = Pubkey::find_program_address(
            &auction_cycle_state_seeds(&root_pubkey, &current_cycle_bytes),
            &GOLD_ID,
        );
        let cycle_state_data = connection.get_account_data(&cycle_pubkey)?;
        let cycle_state: AuctionCycleState = try_from_slice_unchecked(&cycle_state_data)?;

        Ok(Self {
            root_pubkey,
            root_state,
            cycle_state,
            error_streak: 0,
        })
    }

    // updates the stored root state
    pub fn update_root_state(&mut self, connection: &RpcClient) -> Result<(), anyhow::Error> {
        let root_state_data = connection.get_account_data(&self.root_pubkey)?;
        self.root_state = try_from_slice_unchecked(&root_state_data)?;
        Ok(())
    }

    // updates the stored cycle state
    pub fn update_cycle_state(&mut self, connection: &RpcClient) -> Result<(), anyhow::Error> {
        let current_cycle_bytes = self.root_state.status.current_auction_cycle.to_le_bytes();
        let (cycle_pubkey, _) = Pubkey::find_program_address(
            &auction_cycle_state_seeds(&self.root_pubkey, &current_cycle_bytes),
            &GOLD_ID,
        );
        let cycle_state_data = connection.get_account_data(&cycle_pubkey)?;
        self.cycle_state = try_from_slice_unchecked(&cycle_state_data)?;
        Ok(())
    }

    // logs error appropriately
    //  - if unexpected error occurs, increments error_streak
    //
    // expected errors:
    //  - auction cycle was closed by other agent
    //  - bid triggered encore period which extended the cycle
    // both errors can be recognized by a difference in cycle end_times
    pub fn report_error(&mut self, connection: &RpcClient) -> Result<(), anyhow::Error> {
        let prev_end_time = self.cycle_state.end_time;
        self.update_root_state(connection)?;
        self.update_cycle_state(connection)?;

        if prev_end_time == self.cycle_state.end_time {
            self.error_streak += 1;
        }

        Ok(())
    }

    // resets error streak
    // should be used after successful cycle closing
    pub fn reset_error_streak(&mut self) {
        self.error_streak = 0;
    }

    // returns if the auction is likely broken
    // currently identified by facing 5+ consecutive errors on cycle closing
    pub fn is_faulty_auction(&self) -> bool {
        self.error_streak > 5
    }
}

type HashedPool = HashMap<AuctionId, PoolRecord>;
type HashedIdSet = HashSet<AuctionId>;

// manages auction states for caching
pub struct ManagedPool {
    // hashmap containing all auctions and their data
    pub hashed_pool: HashedPool,
    // hashset containing ids of inactive (frozen, filtered, finished) auctions
    pub inactive_auctions: HashedIdSet,
    // hashset containing ids of erroneous auctions
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

    // returns a mutable reference to a pool record if it is active
    //  - returns none if auction is not active (frozen, filtered, finished, erroneous)
    //  - returns none if auction cycle is not over yet
    pub fn get_or_insert_auction(
        &mut self,
        connection: &RpcClient,
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
            Vacant(entry) => entry.insert(PoolRecord::new(connection, &auction_id)?),
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
