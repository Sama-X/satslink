use std::collections::BTreeSet;

use candid::{CandidType, Principal};
use ic_e8s::c::{E8s, ECs};
use ic_stable_structures::{memory_manager::VirtualMemory, DefaultMemoryImpl};
use serde::Deserialize;
use sha2::Digest;
use num_bigint::BigUint;

// use crate
use crate::{cmc::XdrData, ONE_MINUTE_NS};
pub type Address = [u8; 20];
pub type Timestamp = u64;
pub type TCycles = ECs<12>;
pub type Memory = VirtualMemory<DefaultMemoryImpl>;

pub const TCYCLE_POS_ROUND_BASE_FEE: u64 = 25_000_000_000_u64;
pub const POS_ROUND_START_REWARD_E8S: u64 = 1024_0000_0000_u64;
pub const POS_ROUND_END_REWARD_E8S: u64 = 0_0014_0000_u64;
pub const POS_ROUNDS_PER_HALVING: u64 = 5040;
pub const POS_ACCOUNTS_PER_BATCH: u64 = 300;
pub const UPDATE_SEED_DOMAIN: &[u8] = b"stl-satslink-update-seed";

pub const SATSLINKER_REDISTRIBUTION_SUBACCOUNT: [u8; 32] = [0u8; 32];
pub const SATSLINKER_LOTTERY_SUBACCOUNT: [u8; 32] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,];
pub const SATSLINKER_DEV_FEE_SUBACCOUNT: [u8; 32] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2,];
pub const SATSLINKER_SWAPPOOL_SUBACCOUNT: [u8; 32] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3,];

pub const REDISTRIBUTION_LOTTERY_SHARE_E8S: u64 = 1000_0000;    // 10%
pub const REDISTRIBUTION_SWAPPOOL_SHARE_E8S: u64 = 6000_0000;  // 60%
pub const REDISTRIBUTION_DEV_SHARE_E8S: u64 = 3000_0000;      // 30%

// pub const VIP_ROUND_DELAY_NS: u64 = 1_000_000_000; // VIP rounds in minutes
// pub const POS_ROUND_DELAY_NS: u64 = ONE_MINUTE_NS * 2;                  // Generate 1 block every 2 minutes
// pub const ICPSWAP_PRICE_UPDATE_INTERVAL_NS: u64 = ONE_MINUTE_NS * 10;   // Update ICP/Cycles exchange rate every 10 minutes
// pub const ICP_REDISTRIBUTION_INTERVAL_NS: u64 = ONE_HOUR_NS * 3;        // Redistribute ICP every 3 hours
// pub const PLEDGE_ROUND_DELAY_NS: u64 = ONE_MONTH_NS;                    // 1 month pledge cycle

// Test time constants
pub const VIP_ROUND_DELAY_NS: u64 = 1_000_000_000;                        // VIP rounds in minutes
pub const POS_ROUND_DELAY_NS: u64 = ONE_MINUTE_NS / 10;                   // Generate 1 block every 6 seconds
pub const ICPSWAP_PRICE_UPDATE_INTERVAL_NS: u64 = ONE_MINUTE_NS * 1;      // Update ICP/Cycles exchange rate every 1 minute
pub const ICP_REDISTRIBUTION_INTERVAL_NS: u64 = ONE_MINUTE_NS * 1;        // Redistribute ICP every 1 minute
pub const PLEDGE_ROUND_DELAY_NS: u64 = ONE_MINUTE_NS * 1000;               // 1000 minutes pledge cycle


#[derive(CandidType, Deserialize, Clone, Default, Debug)]
pub struct SatslinkerStateInfo {
    pub total_pledge_token_supply: E8s, // Total SATSLINK tokens pledged by all users
    pub total_token_lottery: E8s,
    pub total_token_dev: E8s,
    pub total_token_minted: E8s,
    pub current_token_reward: E8s,
    pub current_pos_round: u64,
    pub pos_round_delay_ns: u64,

    pub seed: Vec<u8>,
    pub satslink_enabled: Option<bool>,
    pub tmp_can_vip_migrate: Option<BTreeSet<Principal>>,
    pub tmp_can_pledge_migrate: Option<BTreeSet<Principal>>,
    pub icp_to_cycles_exchange_rate: Option<TCycles>,
}

impl SatslinkerStateInfo {
    pub fn init(&mut self, seed: Vec<u8>) {
        self.seed = seed;
        self.current_token_reward = E8s::from(POS_ROUND_START_REWARD_E8S);
        self.pos_round_delay_ns = POS_ROUND_DELAY_NS;
    }

    pub fn get_icp_to_cycles_exchange_rate(&self) -> TCycles {
        self.icp_to_cycles_exchange_rate
            .clone()
            // shouldn't ever be the case, since we're fetching the rate each 10 minutes, but defaults to 8T per ICP
            .unwrap_or(TCycles::from(8_0000_0000_0000u64))
    }

    pub fn update_icp_to_cycles_exchange_rate(&mut self, new_rate: XdrData) {
        let rate_e4s = ECs::<4>::from(new_rate.xdr_permyriad_per_icp);
        let rate_tcycles = rate_e4s.to_dynamic().to_decimals(12).to_const::<12>();

        self.icp_to_cycles_exchange_rate = Some(rate_tcycles);
    }

    pub fn is_satslink_enabled(&self) -> bool {
        self.satslink_enabled.unwrap_or_default()
    }

    pub fn enable_satslink(&mut self) {
        self.satslink_enabled = Some(true);
    }

    pub fn disable_satslink(&mut self) {
        self.satslink_enabled = Some(false);
    }

    pub fn complete_round(&mut self) {
        self.current_pos_round += 1;
        self.update_seed();

        // each 5040 blocks we half the reward, until it reaches 0.0014 SATSLINK per block
        if (self.current_pos_round % POS_ROUNDS_PER_HALVING) == 0 {
            let end_reward = E8s::from(POS_ROUND_END_REWARD_E8S);

            if self.current_token_reward > end_reward {
                let new_reward = (self.current_token_reward.val.clone() * BigUint::from(3u64)) / BigUint::from(4u64); // 0.75
                self.current_token_reward = ECs::<8>::new(new_reward); // 更新当前奖励

                if self.current_token_reward < end_reward {
                    self.current_token_reward = end_reward;
                }
            }
        }
    }

    pub fn current_winning_idx(&self, total_options: u64) -> u64 {
        let mut rng_buf = [0u8; 8];
        rng_buf.copy_from_slice(&self.seed[0..8]);

        u64::from_le_bytes(rng_buf) % total_options
    }

    pub fn update_seed(&mut self) {
        let mut hasher = sha2::Sha256::default();
        hasher.update(UPDATE_SEED_DOMAIN);
        hasher.update(&self.seed);

        self.seed = hasher.finalize().to_vec();
    }

    // pub fn note_pledged_satslink(&mut self, qty: E8s) {
    //     self.total_pledge_token_supply += qty;
    // }

    // pub fn note_minted_reward(&mut self, qty: E8s) {
    //     self.total_token_minted += qty;
    // }

    // pub fn note_satslink_token_lottery(&mut self, qty: E8s) {
    //     self.total_token_lottery += qty;
    // }

    // pub fn get_satslink_token_lottery(&mut self) ->E8s{
    //     return self.total_token_lottery.clone();
    // }

    // pub fn note_satslink_token_dev(&mut self, qty: E8s) {
    //     self.total_token_dev += qty;
    // }

    // pub fn get_satslink_token_dev(&mut self) ->E8s {
    //     return self.total_token_dev.clone();
    // }

    pub fn can_vip_migrate(&self, caller: &Principal) -> bool {
        self.tmp_can_vip_migrate
            .as_ref()
            .map(|it| it.contains(caller))
            .unwrap_or_default()
    }

    pub fn note_vip_migrated(&mut self, caller: &Principal) {
        if let Some(can_migrate) = &mut self.tmp_can_vip_migrate {
            can_migrate.remove(caller);
        }
    }

    pub fn can_pledge_migrate(&self, caller: &Principal) -> bool {
        self.tmp_can_pledge_migrate
            .as_ref()
            .map(|it| it.contains(caller))
            .unwrap_or_default()
    }

    pub fn note_pledge_migrated(&mut self, caller: &Principal) {
        if let Some(can_migrate) = &mut self.tmp_can_pledge_migrate {
            can_migrate.remove(caller);
        }
    }

    pub fn get_current_fee() -> TCycles {
        TCycles::from(TCYCLE_POS_ROUND_BASE_FEE)
    }
}
