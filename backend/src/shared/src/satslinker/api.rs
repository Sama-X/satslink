use candid::{CandidType, Nat, Principal};
use ic_e8s::c::E8s;
use ic_ledger_types::AccountIdentifier;

use serde::Deserialize;

use super::types::TCycles;

#[derive(CandidType, Deserialize)]
pub struct GetSatslinkersRequest {
    pub start: Option<Principal>,
    pub take: u32,
}

#[derive(CandidType, Deserialize)]
pub struct GetSatslinkersResponse {
    pub entries: Vec<(Principal, TCycles, E8s, bool)>,
}

#[derive(CandidType, Deserialize)]
pub struct GetTotalsResponse {
    pub total_icp_shares_supply: TCycles,
    pub total_pledge_shares_supply: TCycles,
    pub total_satslink_token_lottery: E8s,
    pub total_satslink_token_dev: E8s,
    pub total_satslink_token_minted: E8s,
    pub current_satslink_token_reward: E8s,
    pub pos_start_key: Option<Principal>,
    pub current_pos_round: u64,
    pub current_share_fee: TCycles,
    pub is_lottery_enabled: bool,

    pub total_satslinkers: u64,
    pub total_lottery_participants: u64,
    pub total_pos_participants: u64,
    pub total_pledge_participants: u64,
    pub total_vip_participants: u64,
    pub icp_to_cycles_exchange_rate: TCycles,

    pub your_share_tcycles: TCycles,
    pub your_unclaimed_reward_e8s: E8s,
    pub your_lottery_eligibility_status: bool,
}

#[derive(CandidType, Deserialize)]
pub enum RefundTokenKind {
    ICP(Vec<(AccountIdentifier, u64)>),
}

#[derive(CandidType, Deserialize)]
pub struct RefundLostTokensRequest {
    pub kind: RefundTokenKind,
}

#[derive(CandidType, Deserialize)]
pub struct RefundLostTokensResponse {
    pub results: Vec<Result<Nat, String>>,
}

#[derive(CandidType, Deserialize)]
pub struct ClaimRewardRequest {
    pub to: Principal,
}

#[derive(CandidType, Deserialize)]
pub struct ClaimRewardResponse {
    pub result: Result<Nat, String>,
}

#[derive(CandidType, Deserialize)]
pub struct StakeRequest {
    pub qty_e8s_u64: u64,
    pub enable_lottery: bool,
}

#[derive(CandidType, Deserialize)]
pub struct StakeResponse {}

#[derive(CandidType, Deserialize)]
pub struct PledgeRequest {
    pub qty_e8s_u64: u64,
}

#[derive(CandidType, Deserialize)]
pub struct PledgeResponse {}

#[derive(CandidType, Deserialize)]
pub struct RedeemRequest {
    pub to: Principal,
}

#[derive(CandidType, Deserialize)]
pub struct RedeemResponse {
    pub result: Result<Nat, String>,
}

#[derive(CandidType, Deserialize)]
pub struct WithdrawRequest {
    pub qty_e8s: E8s,
    pub to: Principal,
}

#[derive(CandidType, Deserialize)]
pub struct WithdrawResponse {}

#[derive(CandidType, Deserialize)]
pub struct MigrateMsqAccountRequest {
    pub to: Principal,
}

#[derive(CandidType, Deserialize)]
pub struct MigrateMsqAccountResponse {}
