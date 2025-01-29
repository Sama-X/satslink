import { createContext, createEffect, createSignal, on, onCleanup, onMount, useContext } from "solid-js";
import { IChildren, ONE_WEEK_NS } from "../utils/types";
import { ErrorCode, err, logErr, logInfo } from "../utils/error";
import { createStore, Store } from "solid-js/store";
import { iiFeHost, useAuth } from "./auth";
import { newSatslinkerActor, optUnwrap } from "@utils/backend";
import { DEFAULT_TOKENS, useTokens } from "./tokens";
import { E8s, EDs } from "@utils/math";
import { Principal } from "@dfinity/principal";
import { debugStringify } from "@utils/encoding";
import {
  requestVerifiablePresentation,
  VerifiablePresentationResponse,
} from "@dfinity/verifiable-credentials/request-verifiable-presentation";

// Define TCycles type
type TCycles = bigint;

export interface ITotals {
  totalPledgeTokenSupply: E8s;
  totalTokenLottery: E8s;
  totalTokenDev: E8s;
  totalTokenMinted: E8s;
  currentTokenReward: E8s;
  currentShareFee: bigint;
  isSatslinkEnabled: boolean;

  currentPosRound: bigint;
  posRoundDelayNs: bigint;
  
  totalPledgeParticipants: bigint;
  totalVipParticipants: bigint;
  icpToCyclesExchangeRate: bigint;

  yourVipShares: bigint;
  yourVipUnclaimedRewardE8s: E8s;
  yourVipEligibilityStatus: boolean;
  yourPledgeShares: E8s;
  yourPledgeUnclaimedRewardE8s: E8s;
  yourPledgeEligibilityStatus: boolean;
}

export interface IPoolMember {
  id: Principal;
  share: EDs;
  unclaimedReward: E8s;
  isVerifiedViaDecideID: boolean;
}

interface StakeResponse {
  result: { Ok: bigint } | { Err: string };
  message: string;
}

export interface ISatslinkerStoreContext {
  totals: Store<{ data?: ITotals }>;
  fetchTotals: () => Promise<void>;

  getMyDepositAccount: () => { owner: Principal; subaccount?: Uint8Array } | undefined;

  canStake: () => boolean;
  stake: () => Promise<void>;

  canWithdraw: () => boolean;
  withdraw: (amount: number, to: Principal) => Promise<StakeResponse>;

  canClaimReward: () => boolean;
  claimReward: (to: Principal) => Promise<void>;

  poolMembers: () => IPoolMember[];
  fetchPoolMembers: () => Promise<void>;

  canMigrateMsqAccount: () => boolean;
  migrateMsqAccount: () => Promise<void>;

  canVerifyDecideId: () => boolean;
  verifyDecideId: () => Promise<void>;
}

const SatslinkerContext = createContext<ISatslinkerStoreContext>();

export function useSatslinker(): ISatslinkerStoreContext {
  const ctx = useContext(SatslinkerContext);

  if (!ctx) {
    err(ErrorCode.UNREACHEABLE, "Satslinker context is not initialized");
  }

  return ctx;
}

export function SatslinkerStore(props: IChildren) {
  const {
    assertReadyToFetch,
    assertAuthorized,
    anonymousAgent,
    isAuthorized,
    agent,
    identity,
    disable,
    enable,
    authProvider,
    iiClient,
    deauthorize,
  } = useAuth();
  const { subaccounts, fetchSubaccountOf, balanceOf, fetchBalanceOf } = useTokens();

  const [totals, setTotals] = createStore<ISatslinkerStoreContext["totals"]>();
  const [poolMembers, setPoolMembers] = createSignal<IPoolMember[]>([]);
  const [int, setInt] = createSignal<NodeJS.Timeout>();
  const [canMigrate, setCanMigrate] = createSignal(false);

  onMount(() => {
    const t = setInterval(() => {
      fetchTotals();
    }, 1000 * 60 * 2);

    setInt(t);
  });

  onCleanup(() => {
    const t = int();
    if (!t) return;

    clearInterval(t);
  });

  createEffect(
    on(anonymousAgent, (a) => {
      if (!a) return;

      fetchTotals();
    })
  );

  createEffect(
    on(agent, (a) => {
      if (a!) {
        fetchSubaccountOf(identity()!.getPrincipal());
        fetchTotals();

        if (authProvider() === "MSQ") {
          fetchCanMigrateMsqAccount();
        }
      }
    })
  );

  const fetchTotals: ISatslinkerStoreContext["fetchTotals"] = async () => {
    assertReadyToFetch();

    const ag = agent() ? agent()! : anonymousAgent()!;

    const satslinker = newSatslinkerActor(ag);
    const resp = await satslinker.get_totals();

    const iTotals: ITotals = {
      totalPledgeTokenSupply: E8s.new(resp.total_pledge_token_supply),
      totalTokenLottery: E8s.new(resp.total_token_lottery),
      totalTokenDev: E8s.new(resp.total_token_dev),
      totalTokenMinted: E8s.new(resp.total_token_minted),
      currentTokenReward: E8s.new(resp.current_token_reward),
      currentShareFee: BigInt(resp.current_share_fee),
      isSatslinkEnabled: resp.is_satslink_enabled,

      currentPosRound: BigInt(resp.current_pos_round),
      posRoundDelayNs: BigInt(resp.pos_round_delay_ns),
      
      totalPledgeParticipants: BigInt(resp.total_pledge_participants),
      totalVipParticipants: BigInt(resp.total_vip_participants),
      icpToCyclesExchangeRate: BigInt(resp.icp_to_cycles_exchange_rate),

      yourVipShares: BigInt(resp.your_vip_shares),
      yourVipUnclaimedRewardE8s: E8s.new(resp.your_vip_unclaimed_reward_e8s),
      yourVipEligibilityStatus: resp.your_vip_eligibility_status,
      yourPledgeShares: E8s.new(resp.your_pledge_shares),
      yourPledgeUnclaimedRewardE8s: E8s.new(resp.your_pledge_unclaimed_reward_e8s),
      yourPledgeEligibilityStatus: resp.your_pledge_eligibility_status
    };

    setTotals({ data: iTotals });
  };

  const fetchPoolMembers: ISatslinkerStoreContext["fetchPoolMembers"] = async () => {
    assertReadyToFetch();

    let start: [] | [Principal] = [];
    const members = [];
    const satslinker = newSatslinkerActor(anonymousAgent()!);

    // 获取以太坊地址
    const auth = useAuth();
    const addressBytes = await auth.getEthAddress();
    if (!addressBytes) {
      console.error("未能获取以太坊地址");
      return;
    }

    // 调用 get_satslinkers 接口，传入以太坊地址
    const response = await satslinker.get_satslinkers(addressBytes);

    for (let entry of response.entry) {
      let iPoolMember: IPoolMember = {
        id: entry[1], // Principal
        share: EDs.new(entry[2], 12), // share
        unclaimedReward: E8s.new(entry[3]), // unclaimed reward
        isVerifiedViaDecideID: entry[4], // VIP status
      };

      members.push(iPoolMember);
    }

    setPoolMembers(
      members.sort((a, b) => {
        if (a.share.gt(b.share)) {
          return -1;
        } else if (a.share.lt(b.share)) {
          return 1;
        } else {
          return 0;
        }
      })
    );
  };

  const getMyDepositAccount: ISatslinkerStoreContext["getMyDepositAccount"] = () => {
    if (!isAuthorized()) return undefined;

    const mySubaccount = subaccounts[identity()!.getPrincipal().toText()];
    if (!mySubaccount) return undefined;

    return { owner: Principal.fromText(import.meta.env.VITE_SATSLINKER_CANISTER_ID), subaccount: mySubaccount };
  };

  createEffect(
    on(getMyDepositAccount, (acc) => {
      if (!acc) return;

      fetchBalanceOf(DEFAULT_TOKENS.icp, acc.owner, acc.subaccount);
    })
  );

  const canStake: ISatslinkerStoreContext["canStake"] = () => {
    if (!isAuthorized()) return false;

    const myDepositAccount = getMyDepositAccount();
    if (!myDepositAccount) return false;

    const b = balanceOf(DEFAULT_TOKENS.icp, myDepositAccount.owner, myDepositAccount.subaccount);
    if (!b) return false;

    if (E8s.new(b).le(E8s.f0_5())) return false;

    return true;
  };

  const stake: ISatslinkerStoreContext["stake"] = async () => {
    assertAuthorized();

    disable();

    const myDepositAccount = getMyDepositAccount()!;
    const b = balanceOf(DEFAULT_TOKENS.icp, myDepositAccount.owner, myDepositAccount.subaccount)!;
    const satslinker = newSatslinkerActor(agent()!);
    const auth = useAuth();
    const addressBytes = await auth.getEthAddress();
    if (addressBytes === null) {
      err(ErrorCode.AUTH, "Failed to get ETH address");
    }
    await satslinker.purchase({ qty_e8s_u64: b - 10_000n, address: addressBytes });

    enable();

    fetchTotals();
    fetchBalanceOf(DEFAULT_TOKENS.icp, myDepositAccount.owner, myDepositAccount.subaccount);

    logInfo(`Successfully satslinked ${E8s.new(b).toString()} ICP`);
  };

  const canWithdraw: ISatslinkerStoreContext["canWithdraw"] = () => {
    if (!isAuthorized()) return false;

    const myDepositAccount = getMyDepositAccount();
    if (!myDepositAccount) return false;

    const b = balanceOf(DEFAULT_TOKENS.icp, myDepositAccount.owner, myDepositAccount.subaccount);
    if (!b) return false;

    // min withdraw amount is 0.1 ICP
    if (E8s.new(b).le(E8s.new(10_0000n))) return false;

    return true;
  };

  const withdraw: ISatslinkerStoreContext["withdraw"] = async (amount: number, to: Principal) => {
    assertReadyToFetch();
    const actor = newSatslinkerActor(agent()!);
    const result = await actor.purchase({
      qty_e8s_u64: E8s.new(BigInt(amount)).toBigIntRaw(),
      address: new Uint8Array(to.toUint8Array())
    });
    return result;
  };

  const canClaimReward: ISatslinkerStoreContext["canClaimReward"] = () => {
    if (!isAuthorized()) return false;

    if (!totals.data) return false;

    return totals.data.yourVipUnclaimedRewardE8s.gt(E8s.zero());
  };

  const claimReward: ISatslinkerStoreContext["claimReward"] = async (to) => {
    assertAuthorized();

    disable();

    const satslinker = newSatslinkerActor(agent()!);
    const result = await satslinker.claim_vip_reward({ to });

    if ("Err" in result) {
      logErr(ErrorCode.UNKNOWN, debugStringify(result.Err));
      enable();

      return;
    }

    enable();

    fetchTotals();
    logInfo(`Successfully claimed all SATSLINK!`);
  };

  const fetchCanMigrateMsqAccount = async () => {
    assertAuthorized();

    const satslinker = newSatslinkerActor(agent()!);
    const result = await satslinker.can_migrate_stl_account();

    setCanMigrate(result);
  };

  const canMigrateMsqAccount = () => {
    if (!isAuthorized()) return false;
    if (!canMigrate()) return false;
    if (authProvider() === "II") return false;

    return true;
  };

  const migrateMsqAccount: ISatslinkerStoreContext["migrateMsqAccount"] = async () => {
    assertAuthorized();

    disable();

    try {
      const iiIdentity = await accessIiIdentity();

      const satslinker = newSatslinkerActor(agent()!);
      await satslinker.migrate_stl_account({ to: iiIdentity.getPrincipal() });

      await deauthorize();
      window.location.reload();
    } finally {
      enable();
    }
  };

  const accessIiIdentity = async () => {
    const client = iiClient();
    if (!client) {
      enable();
      err(ErrorCode.AUTH, "Uninitialized auth client");
    }

    const isAuthenticated = await client.isAuthenticated();

    if (isAuthenticated) {
      return client.getIdentity();
    }

    await new Promise((res, rej) =>
      client.login({
        identityProvider: iiFeHost(),
        onSuccess: res,
        onError: rej,
        maxTimeToLive: ONE_WEEK_NS,
      })
    );

    return client.getIdentity();
  };

  const canVerifyDecideId: ISatslinkerStoreContext["canVerifyDecideId"] = () => {
    if (!isAuthorized()) return false;

    const t = totals.data;
    if (!t) return false;

    const p = authProvider();
    if (p === "MSQ") return false;

    return !t.yourVipEligibilityStatus;
  };

  const verifyDecideId: ISatslinkerStoreContext["verifyDecideId"] = async () => {
    assertAuthorized();

    disable();

    try {
      const userPrincipal = identity()!.getPrincipal();

      const jwt: string = await new Promise((res, rej) => {
        requestVerifiablePresentation({
          onSuccess: async (verifiablePresentation: VerifiablePresentationResponse) => {
            if ("Ok" in verifiablePresentation) {
              res(verifiablePresentation.Ok);
            } else {
              rej(new Error(verifiablePresentation.Err));
            }
          },
          onError(err) {
            rej(new Error(err));
          },
          issuerData: {
            origin: "https://id.decideai.xyz",
            canisterId: Principal.fromText("qgxyr-pyaaa-aaaah-qdcwq-cai"),
          },
          credentialData: {
            credentialSpec: {
              credentialType: "ProofOfUniqueness",
              arguments: {},
            },
            credentialSubject: userPrincipal,
          },
          identityProvider: new URL(iiFeHost()),
        });
      });

      const satslinker = newSatslinkerActor(agent()!);
      //await satslinker.verify_decide_id({ jwt });

      fetchTotals();
    } finally {
      enable();
    }
  };

  return (
    <SatslinkerContext.Provider
      value={{
        totals,
        fetchTotals,
        poolMembers,
        fetchPoolMembers,
        getMyDepositAccount,
        stake,
        canStake,
        withdraw,
        canWithdraw,
        canClaimReward,
        claimReward,

        canMigrateMsqAccount,
        migrateMsqAccount,
        canVerifyDecideId,
        verifyDecideId,
      }}
    >
      {props.children}
    </SatslinkerContext.Provider>
  );
}
