
# Satslink

**Overview**

Satslink is an innovative privacy communication application designed for Web3 users. It integrates the functionalities of a traditional VPN and leverages Sama Network's full-stack data encryption communication capabilities to provide users with end-to-end comprehensive data encryption protection.

**Objectives**

Satslink aims to:

  - **Enhance Web3 User Privacy:** Replace centralized VPNs and address the data leakage and auditing risks inherent in traditional VPNs.
  - **Ensure Data Security:** Prevent user data from being hijacked, monitored, and analyzed by centralized entities.
  - **Build a Secure Network Environment:** Utilize SAMA Network's consensus and business sharding technology to enhance network security and privacy.
  - **Simplify Web3 Payments:** Based on MSQ, enable direct purchase of ICP and ICRC-1 and payment of VPN service fees.

📐 **Project Architecture and Function**

Satslink is built upon the following core technologies:

  - **Sama Network:** Provides underlying full-stack data encryption communication capabilities to ensure data transmission security.
  - **MSQ (Message Stream Queue):** Used for user identity integration and payment systems, enabling user authorization and on-chain transactions.
  - **Internet Computer (ICP):** Serves as the underlying blockchain platform, providing decentralized infrastructure.
  - **ICRC-1:** Used for cryptocurrency payments and transactions.
  - **IC Exchange Rate Canister:** Ensures the dynamic and fair pricing of Satslink services.
  - **Android (Client):** Provides a user-friendly mobile application.

**(Architecture Diagram)**

graph LR
    A[User (Android)] --> B(Satslink Client)
    A --> C{Sama Network}
    B --> D[Internet Computer (ICP)]
    B --> E(MSQ) --> D[Internet Computer (ICP)]
    B --> A[User (Android)]
    C --> A[User (Android)]

**Web3 Privacy Communication Application, Your Data, Under Your Control.**

[![Twitter](about:sanitized)](https://x.com/gknmoon)
[![Discord](about:sanitized)](https://discord.com/channels/1062661363756966020/1159441094526902324)

**Provide Metadata**

  - **Description:** Satslink is a privacy communication application that provides end-to-end encrypted VPN services for Web3 users, ensuring data security and privacy. Built on the Sama Network, it is dedicated to replacing centralized VPNs and reducing data leakage and auditing risks.
  - **Tags:** `internet-computer`, `privacy`, `VPN`, `Web3`, `encryption`, `Sama-network`, `ICP`, `ICRC-1`
  - **Project Website/Homepage:** (Can be your project introduction page or your personal website. Leave blank if not available yet)
  - **Code Repositories:**
      - [satslink\_client\_android](https://github.com/Sama-X/satslink_client_android) - Android client code repository
      - [satslink](https://github.com/Sama-X/satslink) - ICP - Satslink core code repository

**Local development**

This project is managed with script and pnpm. On Dev ..

<!-- end list -->

```
./script/before_build.sh dev
```

```
./script/extract-did.sh dev
```

```
./script/generate-bind.sh dev
```

```
./script/deploy.back.sh dev
```

pnpm install

```
./script/deploy.front.sh dev
```

**Core Interfaces**

  - **Get the number of billed users:**

<!-- end list -->

```
count_payment_users : () -> (nat64) query;
```

  - **Get all non-expired payment bills:**

<!-- end list -->

```
get_payment_stats : () -> (Result_1) query;
```

  - **Get payment bills by eth address:**

<!-- end list -->

```
get_payments_by_eth_address : (text) -> (nat64) query;
```

  - **Get payment bills by principal:**

<!-- end list -->

```
get_payments_by_principal : (text) -> (vec PaymentRecord) query;
```

  - **Support payment currency whitelist:**

<!-- end list -->

```
manage_whitelist : (text, WhitelistOperation) -> (Result_2);
```

  - **Pay for Satslink VPN VIP:**

<!-- end list -->

```
pay : (principal, nat, text, text) -> (Result_3);
```

  - **Authorize withdrawal of payment amount:**

<!-- end list -->

```
withdraw : (Account, nat) -> (Result_3);
```

**Contribution**

Feel free to open an issue if you found a bug or want to suggest a feature.

