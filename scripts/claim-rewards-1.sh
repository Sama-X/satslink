# stake 操作
dfx canister call satslinker get_satslinkers '(
    blob "\d8\da\6b\f2\69\64\af\9f\7e\f5\7c\c2\52\51\65\f2\3e\3b\32\a6"
)'

sleep 2

dfx canister call satslinker claim_vip_reward '(record { to = principal "bn3iu-mpsrd-gt3co-obdrr-ze753-7ybad-nl2ak-x7tey-atlbe-qeoz5-mqe"})'


sleep 2
# balance 
dfx canister call satslink_token icrc1_balance_of '(record { owner = principal "bn3iu-mpsrd-gt3co-obdrr-ze753-7ybad-nl2ak-x7tey-atlbe-qeoz5-mqe"; subaccount = null })'


sleep 10
# stake 操作
dfx canister call satslinker pledge '(
    record { 
        qty_e8s_u64 = 500_000_000
    }
)'