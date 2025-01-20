use std::collections::BTreeSet;
use candid::{decode_one, encode_one, Principal};
use ic_cdk::api::time;
use ic_cdk::println;
use ic_e8s::c::{E8s, ECs};
use ic_stable_structures::{storable::Bound, Cell, StableBTreeMap, Storable};

use super::{
    api::{
        GetVIPuserResponse, 
        GetTotalsResponse
    },
    types::{
        SatslinkerStateInfo, 
        Memory, 
        TCycles, 
        Timestamp,
        Address,
        TCYCLE_POS_ROUND_BASE_FEE,
        PLEDGE_ROUND_DELAY_NS,
        VIP_ROUND_DELAY_NS,
    },
};

pub struct SatslinkerState {
    pub vip_shares: StableBTreeMap<Principal, (Address, Timestamp, E8s), Memory>,
    pub pledge_shares: StableBTreeMap<Principal, (E8s, Timestamp, E8s), Memory>, 
    pub info: Cell<SatslinkerStateInfo, Memory>,
    pub vip_participants: StableBTreeMap<Address, (Principal, Timestamp, E8s), Memory>,
}

impl SatslinkerState {
    pub fn init(&mut self, seed: Vec<u8>) {
        let mut info = self.get_info();
        info.init(seed);
        info.enable_satslink();
        self.set_info(info);
    }

    // TODO: delete this function, once the initialization is complete
    pub fn init_tmp_can_migrate(&mut self) {
        let mut info = self.get_info();
        // Only execute on the first call
        if info.tmp_can_vip_migrate.is_none() {
            let can_migrate_vip_set: BTreeSet<_> = self.vip_shares.iter().map(|(k, _)| k.clone()).collect();
            info.tmp_can_vip_migrate = Some(can_migrate_vip_set);
        }
        
        if info.tmp_can_pledge_migrate.is_none() {
            let can_migrate_pledge_set: BTreeSet<_> = self.pledge_shares.iter().map(|(k, _)| k.clone()).collect();
            info.tmp_can_pledge_migrate = Some(can_migrate_pledge_set);
        }

        self.set_info(info);
    }

    pub fn migrate_satslinker_account(&mut self, caller: &Principal, to: Principal,) -> Result<(), String> {
        let mut info = self.get_info();

        if !info.can_vip_migrate(caller) || !info.can_pledge_migrate(caller) {
            return Err(String::from("Access denied"));
        }

         // Handle VIP share migration
        if let Some((address_1, share_1, reward_1)) = self.vip_shares.remove(caller) {
            let (address, share, reward) = if let Some((address_2, share_2, reward_2)) = self.vip_shares.get(&to) {
                (address_2.clone(), share_1 + share_2, reward_1 + reward_2)
            } else {
                (address_1, share_1, reward_1)
            };

            self.vip_shares.insert(to, (address, share, reward));
            info.note_vip_migrated(caller);
        } 

        // Handle pledge share migration
        if let Some((share_1, tmps_1, reward_1)) = self.pledge_shares.remove(caller) {
            let (share_2, tmps_2, reward_2) = self.pledge_shares
                .get(&to)
                .map(|(s, t, r)| (s.clone(), t.clone(), r.clone()))
                .unwrap_or((E8s::zero(), 0u64, E8s::zero()));
            let (share, tmps, reward) = (share_1 + share_2, tmps_1 + tmps_2, reward_1 + reward_2);
            let fee = TCycles::from(TCYCLE_POS_ROUND_BASE_FEE)
                .to_dynamic()
                .to_decimals(8)
                .to_const::<8>();

            if share.clone() > fee {
                self.pledge_shares.insert(to, (share.clone(), tmps, reward));
                info.note_pledge_migrated(caller);     
            }   
        } 

        self.set_info(info);
        Ok(())

    }

    pub fn mint_vip_share(&mut self, tmps: Timestamp, to: Principal, address: Address) {
        // add new share to the account
        let cur_opt = self.vip_shares.get(&to);
        println!("mint vip share shares: {:?}", tmps);
        
        let (address, share, unclaimed_reward) = if let Some((cur_address, mut cur_share, cur_unclaimed_reward)) = cur_opt {
            cur_share += &tmps;
            (cur_address.clone(), cur_share, cur_unclaimed_reward.clone())
        } else {
            (address, tmps, E8s::zero())
        };

        self.vip_shares.insert(to, (address, share, unclaimed_reward.clone()));
        self.vip_participants.insert(address, (to, share, unclaimed_reward.clone()));
        println!("VIP shares: {:?} | unclarmed reward: {:?}", share, unclaimed_reward.clone());
    }

    pub fn claim_vip_reward(&mut self, caller: Principal) -> Option<E8s> {
        let current_time = time() / VIP_ROUND_DELAY_NS; // Get current time

        if let Some((address, share, unclaimed_reward)) = self.vip_shares.get(&caller) {
            let mut info = self.get_info();
            // Check if VIP time has expired
            if current_time >= share {
                // Only remove if user is in participant list
                if self.vip_participants.contains_key(&address) {
                    self.vip_participants.remove(&address);
                }
                self.vip_shares.remove(&caller); 
                // If there are unclaimed rewards
                if unclaimed_reward > E8s::zero() {
                    info.total_token_minted += &unclaimed_reward;
                    self.set_info(info);           
                    return Some(unclaimed_reward);
                } 
            } else {
                // If VIP time hasn't expired, return original unclaimed rewards
                info.total_token_minted += &unclaimed_reward;
                self.set_info(info);

                // Reset unclaimed rewards
                self.vip_shares.insert(caller, (address, share, E8s::zero()));
                self.vip_participants.insert(address, (caller, share, E8s::zero()));
                return Some(unclaimed_reward);
            }
        }

        None
    }

    pub fn revert_claim_vip_reward(&mut self, caller: Principal, unclaimed_reward: E8s) {
        let mut info = self.get_info();
        info.total_token_minted -= &unclaimed_reward;
        self.set_info(info);

        if let Some((address, share, reward)) = self.vip_shares.get(&caller) {
            let new_rewards = reward + unclaimed_reward;
            self.vip_shares.insert(caller, (address, share, new_rewards.clone()));
            // Only insert if user is not in participant list
            if !self.vip_participants.contains_key(&address) {
                self.vip_participants.insert(address, (caller, share, new_rewards.clone()));
            }
        } 
    }

    pub fn mint_pledge_share(&mut self, qty: E8s, pledge_time: Timestamp, to: Principal) {
        // Add new SATSLINK share to the account
        let cur_opt = self.pledge_shares.get(&to);
        let mut info = self.get_info();
        let satslink_amount = qty.clone();
    
        // Update the user's SATSLINK share and unclaimed reward
        let (satslink_share, pledge_satslink_time, unclaimed_reward) = 
            if let Some((mut cur_satslink_share ,_, cur_unclaimed_reward)) = cur_opt
            {
                cur_satslink_share += qty; // Increase the user's SATSLINK share
                (cur_satslink_share, pledge_time.clone(), cur_unclaimed_reward)

            } else {
                (qty.clone(), pledge_time.clone(), E8s::zero())
            };


        let fee = TCycles::from(TCYCLE_POS_ROUND_BASE_FEE)
            .to_dynamic()
            .to_decimals(8)
            .to_const::<8>();

        if satslink_share >= fee {
            // Update the user's SATSLINK shares
            self.pledge_shares.insert(to, (satslink_share, pledge_satslink_time, unclaimed_reward));
            // Update total pledged SATSLINK amount in SatslinkerStateInfo
            info.total_pledge_token_supply += satslink_amount;// 调整总质押SATSLINK
        }
        self.set_info(info); 

    }

    pub fn claim_pledge_reward(&mut self, caller: Principal) -> Option<E8s> {
        let fee = TCycles::from(TCYCLE_POS_ROUND_BASE_FEE)
            .to_dynamic()
            .to_decimals(8)
            .to_const::<8>();
        // Get user's SATSLINK shares and unclaimed rewards
        if let Some((satslink_share, pledge_satslink_time, unclaimed_reward)) = self.pledge_shares.get(&caller) {

            if satslink_share < fee {
                let mut info = self.get_info();
                info.total_pledge_token_supply -= &satslink_share;
                self.set_info(info);
                self.pledge_shares.remove(&caller);
            } else {
                self.pledge_shares.insert(caller, (satslink_share, pledge_satslink_time, E8s::zero()));
            }

            // Check if user has unclaimed rewards
            if unclaimed_reward > E8s::zero() {
                let mut info = self.get_info();
                info.total_token_minted += &unclaimed_reward;
                self.set_info(info);
                // Return user's unclaimed rewards
                return Some(unclaimed_reward);
            } 
        } 
        None
    }

    pub fn revert_claim_pledge_reward(&mut self, caller: Principal, unclaimed_reward: E8s) {
        let mut info = self.get_info();
        info.total_token_minted -= &unclaimed_reward;

        if let Some((satslink_share, pledge_time, reward)) = self.pledge_shares.get(&caller) {
            self.pledge_shares.insert(caller, (satslink_share, pledge_time, reward + unclaimed_reward));
        } else {
            self.pledge_shares.insert(caller, (E8s::zero(), 0u64, unclaimed_reward));
            // If no staking record found, panic or execute other logic
            panic!("No staking record found for caller: {:?}", caller);
        }
        self.set_info(info);
    }

    // dostribute rewards for vip users
    pub fn distribute_vip_pos_rewards(&mut self) -> bool {
        if self.vip_shares.is_empty() {
            return true;
        }
    
        let info = self.get_info();
        let mut cur_reward = info.current_token_reward.clone();

        cur_reward *= ECs::<8>::from(500u64);
        cur_reward /= ECs::<8>::from(1000u64);  // 50% = 500/1000
        println!("Current VIP reward: {:?}", cur_reward);
    
        let current_time = ic_cdk::api::time() / VIP_ROUND_DELAY_NS;
    
        let mut accounts_to_remove = Vec::new();
        let mut accounts_to_update = Vec::new();
    
        // **Step 1: Iterate through vip_shares and categorize**
        for (account, (address, vip_time, unclaimed_reward)) in self.vip_shares.iter() {
            if current_time >= vip_time && unclaimed_reward == ECs::<8>::zero() {
                println!("Expired VIP user: {:?}", account);
                accounts_to_remove.push(account.clone());
            } else {
                accounts_to_update.push((account.clone(), address.clone(), vip_time.clone(), unclaimed_reward.clone()));
            }
        }
    
        // **Step 2: Remove expired accounts**
        for account in accounts_to_remove {
            self.vip_shares.remove(&account);
        }
    
        // Check number of valid accounts
        let valid_account_count = accounts_to_update.len();
        if valid_account_count == 0 {
            return true;
        }
    
        // **Step 3: Calculate rewards for each account**
        let valid_account_count = valid_account_count as u128;
        let reward_per_account = cur_reward.val / valid_account_count;
        let new_reward = ECs::<8>::new(reward_per_account);

        // **Step 4: Update rewards for valid accounts**
        for (account, address, vip_time, unclaimed_reward) in accounts_to_update {
            // Update account reward value
            let updated_reward = unclaimed_reward + new_reward.clone();
            self.vip_shares.insert(account.clone(), (address.clone(), vip_time.clone(), updated_reward.clone()));
            println!("Account: {:?}, Expiration: {:?}, New reward: {:?}", account, vip_time, updated_reward);
        }
    
        self.set_info(info);
        true
    } 

    // Return true if the staking round has completed
    pub fn distribute_pledge_rewards(&mut self) -> bool {
        let mut info = self.get_info();
        let mut cur_reward = info.current_token_reward.clone();
        cur_reward *= ECs::<8>::from(375u64);
        cur_reward /= ECs::<8>::from(1000u64);  // 37.5% = 375/1000
        ic_cdk::println!("Current pledge reward: {:?}", cur_reward);

        if self.pledge_shares.len() == 0 {
            return true;
        }

        let current_time = ic_cdk::api::time() / VIP_ROUND_DELAY_NS;
        let mut accounts_to_update = Vec::new();
        let mut valid_shares_total = E8s::zero();

        // First pass: Collect expired accounts and calculate total valid pledge
        let mut expired_accounts = Vec::new();
        for (account, (share, pledge_time, _)) in self.pledge_shares.iter() {
            let pledge_expiration_time = pledge_time + PLEDGE_ROUND_DELAY_NS;
            if current_time >= pledge_expiration_time {
                expired_accounts.push(account.clone());
            } else {
                valid_shares_total += &share;
            }
        }

        // Second pass: Process rewards for non-expired accounts
        for (account, (share, pledge_time, unclaimed_reward)) in self.pledge_shares.iter() {
            if !expired_accounts.contains(&account) {
                let new_reward = if valid_shares_total > E8s::zero() {
                    &cur_reward * &share / &valid_shares_total
                } else {
                    E8s::zero()
                };
                ic_cdk::println!("Account {:?} received pledge reward: {:?}", account, new_reward);
                let updated_reward = unclaimed_reward + &new_reward;
                accounts_to_update.push((
                    account.clone(),
                    (share.clone(), pledge_time, updated_reward)
                ));
            }
        }

        // Update rewards for non-expired accounts
        for (account, entry) in accounts_to_update {
            self.pledge_shares.insert(account, entry);
        }

        // Remove expired accounts and update total pledge amount
        for account in expired_accounts {
            if let Some((share, _, _)) = self.pledge_shares.remove(&account) {
                info.total_pledge_token_supply -= &share;
                ic_cdk::println!("Removed expired pledge account {:?}, pledge amount: {:?}", account, share);
            }
        }

        // Update state information
        self.set_info(info);
        true
    }

    pub fn get_info(&self) -> SatslinkerStateInfo {
        self.info.get().clone()
    }

    pub fn set_info(&mut self, info: SatslinkerStateInfo) {
        self.info.set(info).expect("Unable to store info");
    }

    pub fn get_satslinkers(&self, address: Address) -> GetVIPuserResponse {
        let mut entry = Vec::new();

        for (participant_address, (principal, share, rewards)) in self.vip_participants.iter() {
            if participant_address == address {
                // If matching address found, add to entries
                entry.push((participant_address.clone(),principal.clone(), share.clone(), rewards.clone(), true));
                println!("satslinker vip shares:{:?} | {:?} | {:?} | {:?} ", principal, address, share, rewards);
            }
        }
    
        GetVIPuserResponse { entry }
    }

    pub fn get_totals(&self, caller: &Principal) -> GetTotalsResponse {
        let info = self.get_info();
        let fee = SatslinkerStateInfo::get_current_fee();
        let is_satslink_enabled = info.is_satslink_enabled();

        let (address, share_1, unclaimed_reward_1) = self.vip_shares
            .get(caller)
            .map(|(a, s, r)| (a.clone(), s.clone(), r.clone()))
            .unwrap_or_default();
        let vip_status = self.vip_participants.contains_key(&address);
        let (share_2, _, unclaimed_reward_2) = self.pledge_shares
            .get(&caller)
            .map(|(s, t, r)| (s.clone(), t, r.clone()))
            .unwrap_or((ECs::zero(), 0u64, ECs::zero()));
        let pledge_status = self.pledge_shares.contains_key(caller);
        let icp_to_cycles_exchange_rate = info.get_icp_to_cycles_exchange_rate();

        GetTotalsResponse {
            total_pledge_token_supply: info.total_pledge_token_supply,
            total_token_lottery: info.total_token_lottery,
            total_token_dev: info.total_token_dev,
            total_token_minted: info.total_token_minted,

            current_pos_round: info.current_pos_round,
            pos_round_delay_ns: info.pos_round_delay_ns,

            current_token_reward: info.current_token_reward,
            current_share_fee: fee,
            is_satslink_enabled,

            total_pledge_participants: self.pledge_shares.len(),
            total_vip_participants: self.vip_participants.len(),

            icp_to_cycles_exchange_rate: icp_to_cycles_exchange_rate,

            your_vip_shares: share_1,
            your_vip_unclaimed_reward_e8s: unclaimed_reward_1,
            your_vip_eligibility_status: vip_status,
            your_pledge_shares: share_2,
            your_pledge_unclaimed_reward_e8s: unclaimed_reward_2,
            your_pledge_eligibility_status: pledge_status,
        }
    }
}

impl Storable for SatslinkerStateInfo {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        match encode_one(self) {
            Ok(bytes) => std::borrow::Cow::Owned(bytes),
            Err(e) => {
                ic_cdk::trap(&format!(
                    "Failed to encode SatslinkerStateInfo: {}. State: {:?}", 
                    e, 
                    self
                ));
            }
        }
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        match decode_one(&bytes) {
            Ok(state) => state,
            Err(e) => {
                ic_cdk::println!(
                    "Failed to decode existing state: {}. Creating new state.", 
                    e
                );
                Self::default()
            }
        }
    }

    const BOUND: ic_stable_structures::storable::Bound = Bound::Unbounded;
}
