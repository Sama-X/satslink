# transfer
dfx canister call nns-ledger icrc1_transfer '(record { from = record { owner = principal "bn3iu-mpsrd-gt3co-obdrr-ze753-7ybad-nl2ak-x7tey-atlbe-qeoz5-mqe"; subaccount = null }; to = record { owner = principal "bkyz2-fmaaa-aaaaa-qaaaq-cai"; subaccount = null }; amount = 100_000_000_000; memo = null })'
dfx canister call nns-ledger icrc1_transfer '(record { from = record { owner = principal "bn3iu-mpsrd-gt3co-obdrr-ze753-7ybad-nl2ak-x7tey-atlbe-qeoz5-mqe"; subaccount = null }; to = record { owner = principal "273et-sy3ra-prqcg-kfz77-u47ys-wmgnd-zxckh-tbb7b-tiij2-6mpgo-vae"; subaccount = null }; amount = 5500_000_000_000; memo = null })'
dfx canister call nns-ledger icrc1_transfer '(record { from = record { owner = principal "bn3iu-mpsrd-gt3co-obdrr-ze753-7ybad-nl2ak-x7tey-atlbe-qeoz5-mqe"; subaccount = null }; to = record { owner = principal "c4dgf-2pclu-fs3cq-pi5mb-7ugp4-7qak2-4znh2-n5eda-bobkt-vcgkl-pqe"; subaccount = null }; amount = 5500_000_000_000; memo = null })'
dfx canister call nns-ledger icrc1_transfer '(record { from = record { owner = principal "bn3iu-mpsrd-gt3co-obdrr-ze753-7ybad-nl2ak-x7tey-atlbe-qeoz5-mqe"; subaccount = null }; to = record { owner = principal "payim-rg5bb-65oni-ygdtf-phiss-kf3ym-hlftd-p5i3k-bxtkq-hxgji-aae"; subaccount = null }; amount = 5500_000_000_000; memo = null })'


# balance
dfx canister call nns-ledger icrc1_balance_of '(record { owner = principal "bn3iu-mpsrd-gt3co-obdrr-ze753-7ybad-nl2ak-x7tey-atlbe-qeoz5-mqe"; subaccount = null })'
dfx canister call nns-ledger icrc1_balance_of '(record { owner = principal "273et-sy3ra-prqcg-kfz77-u47ys-wmgnd-zxckh-tbb7b-tiij2-6mpgo-vae"; subaccount = null })'
dfx canister call nns-ledger icrc1_balance_of '(record { owner = principal "bkyz2-fmaaa-aaaaa-qaaaq-cai"; subaccount = null })'
dfx canister call nns-ledger icrc1_balance_of '(record { owner = principal "c4dgf-2pclu-fs3cq-pi5mb-7ugp4-7qak2-4znh2-n5eda-bobkt-vcgkl-pqe"; subaccount = null })'
dfx canister call nns-ledger icrc1_balance_of '(record { owner = principal "payim-rg5bb-65oni-ygdtf-phiss-kf3ym-hlftd-p5i3k-bxtkq-hxgji-aae"; subaccount = null })'


