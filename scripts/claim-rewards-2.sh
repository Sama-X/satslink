
sleep 2

dfx canister call satslinker get_satslinkers '(
    blob "\a4\2d\35\cc\66\34\c0\53\29\25\a3\b8\44\bc\45\4e\44\38\f4\4e"

)'

sleep 2

dfx canister call satslinker claim_vip_reward '(record { to = principal "273et-sy3ra-prqcg-kfz77-u47ys-wmgnd-zxckh-tbb7b-tiij2-6mpgo-vae"})'

sleep 2
# balance 
dfx canister call satslink_token icrc1_balance_of '(record { owner = principal "273et-sy3ra-prqcg-kfz77-u47ys-wmgnd-zxckh-tbb7b-tiij2-6mpgo-vae"; subaccount = null })'


sleep 10
# stake 操作
dfx canister call satslinker pledge '(
    record { 
        qty_e8s_u64 = 800_000_000
    }
)'