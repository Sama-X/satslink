use candid::{CandidType, Principal};
use env::{
    CAN_SATSLINKER_CANISTER_ID, 
    CAN_SATSLINK_TOKEN_CANISTER_ID, 
    CAN_IC_HOST, 
    CAN_II_CANISTER_ID, 
    CAN_MODE,
    CAN_ROOT_KEY,
};
use lazy_static::lazy_static;
use serde::Deserialize;

pub mod satslinker;
pub mod cmc;
//pub mod decideid;
mod env;
pub mod icrc1;

pub const ICP_FEE: u64 = 10_000u64;
pub const CYCLES_SATSLINKER_FEE: u128 = 10_000_000_000_u128;
pub const MIN_ICP_STAKE_E8S_U64: u64 = 50_000_000;
pub const MIN_STL_LOTTERY_E8S_U64: u64 = 100_000_000;

pub const ONE_MINUTE_NS: u64 = 1_000_000_000 * 60;
pub const ONE_HOUR_NS: u64 = ONE_MINUTE_NS * 60;
pub const ONE_DAY_NS: u64 = ONE_HOUR_NS * 24;
pub const ONE_WEEK_NS: u64 = ONE_DAY_NS * 7;
pub const ONE_MONTH_NS: u64 = ONE_WEEK_NS * 30;

lazy_static! {
    pub static ref ENV_VARS: EnvVarsState = EnvVarsState::new();
}

#[derive(CandidType, Deserialize, Clone)]
pub enum CanisterMode {
    Dev,
    IC,
}

#[derive(CandidType, Deserialize, Clone)]
pub struct EnvVarsState {
    pub satslinker_canister_id: Principal,
    pub satslink_token_canister_id: Principal,
    pub ii_canister_id: Principal,
    pub ii_origin: String,
    pub ic_root_key_der: Vec<u8>,
    pub icp_token_canister_id: Principal,
    pub cycles_minting_canister_id: Principal,
    pub mode: CanisterMode,
}

impl EnvVarsState {
    pub fn new() -> Self {
        let ii_origin = if CAN_MODE == "ic" {
            String::from("https://identity.ic0.app/")
        } else {
            String::from(CAN_IC_HOST).replace("http://", &format!("http://{}.", CAN_II_CANISTER_ID))
        };

        Self {
            satslinker_canister_id: Principal::from_text(CAN_SATSLINKER_CANISTER_ID).unwrap(),
            satslink_token_canister_id: Principal::from_text(CAN_SATSLINK_TOKEN_CANISTER_ID).unwrap(),
            ii_canister_id: Principal::from_text(CAN_II_CANISTER_ID).unwrap(),

            ii_origin,

            ic_root_key_der: CAN_ROOT_KEY
                .trim_start_matches("[")
                .trim_end_matches("]")
                .split(",")
                .map(|chunk| chunk.trim().parse().expect("Unable to parse ic root key"))
                .collect(),
            
            icp_token_canister_id: Principal::from_text("ryjl3-tyaaa-aaaaa-aaaba-cai").unwrap(),
            cycles_minting_canister_id: Principal::from_text("rkp4c-7iaaa-aaaaa-aaaca-cai").unwrap(),

            mode: if CAN_MODE == "ic" {
                CanisterMode::IC
            } else {
                CanisterMode::Dev
            },
        }
    }
}
