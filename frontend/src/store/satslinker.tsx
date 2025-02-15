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

export interface ITotals {
  totalSharesSupply: EDs;
  totalTcyclesSatslinked: EDs;
  totalSatslinkTokenMinted: E8s;
  currentSatslinkTokenReward: E8s;
  posStartKey?: Principal;
  currentPosRound: bigint;
  currentBlockShareFee: EDs;
  posRoundDelayNs: bigint;
  isLotteryEnabled: boolean;

  totalSatslinkers: bigint;
  totalVerifiedAccounts: bigint;
  totalLotteryParticipants: bigint;

  yourShareTcycles: EDs;
  yourUnclaimedReward: E8s;
  yourDecideIdVerificationStatus: boolean;
  yourLotteryEligibilityStatus: boolean;
}

export interface IPoolMember {
  id: Principal;
  share: EDs;
  unclaimedReward: E8s;
  isVerifiedViaDecideID: boolean;
}

export interface ISatslinkerStoreContext {
  totals: Store<{ data?: ITotals }>;
  fetchTotals: () => Promise<void>;

  getMyDepositAccount: () => { owner: Principal; subaccount?: Uint8Array } | undefined;

  canStake: () => boolean;
  stake: () => Promise<void>;

  canWithdraw: () => boolean;
  withdraw: (to: Principal) => Promise<void>;

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
      totalSharesSupply: EDs.new(resp.total_icp_shares_supply, 12),
      totalTcyclesSatslinked: EDs.new(resp.total_tcycles_satslinked, 12),
      totalSatslinkTokenMinted: E8s.new(resp.total_token_minted),
      currentSatslinkTokenReward: E8s.new(resp.current_token_reward),
      posStartKey: optUnwrap(resp.pos_start_key),
      posRoundDelayNs: resp.pos_round_delay_ns,
      currentPosRound: resp.current_pos_round,
      currentBlockShareFee: EDs.new(resp.current_share_fee, 12),
      isLotteryEnabled: resp.is_satslink_enabled,

      totalSatslinkers: resp.total_satslinkers,
      totalLotteryParticipants: resp.total_lottery_participants,
      totalVerifiedAccounts: resp.total_verified_accounts,

      yourShareTcycles: EDs.new(resp.your_share_tcycles, 12),
      yourUnclaimedReward: E8s.new(resp.your_unclaimed_reward_e8s),
      yourDecideIdVerificationStatus: resp.your_decide_id_verification_status,
      yourLotteryEligibilityStatus: resp.your_lottery_eligibility_status,
    };

    setTotals({ data: iTotals });
  };

  const fetchPoolMembers: ISatslinkerStoreContext["fetchPoolMembers"] = async () => {
    assertReadyToFetch();

    let start: [] | [Principal] = [];
    const members = [];

    const satslinker = newSatslinkerActor(anonymousAgent()!);

    while (true) {
      const { entries } = await satslinker.get_satslinkers({ start, take: 1000 });

      if (entries.length === 0) {
        break;
      }

      for (let entry of entries) {
        let iPoolMember: IPoolMember = {
          id: entry[0],
          share: EDs.new(entry[1], 12),
          unclaimedReward: E8s.new(entry[2]),
          isVerifiedViaDecideID: entry[3],
        };

        members.push(iPoolMember);
        start = [iPoolMember.id];
      }
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
    await satslinker.stake({ qty_e8s_u64: b - 10_000n, address: addressBytes });

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

  const withdraw: ISatslinkerStoreContext["withdraw"] = async (to) => {
    assertAuthorized();

    disable();

    const myDepositAccount = getMyDepositAccount()!;
    const b = balanceOf(DEFAULT_TOKENS.icp, myDepositAccount.owner, myDepositAccount.subaccount)!;

    const satslinker = newSatslinkerActor(agent()!);
    await satslinker.withdraw({ qty_e8s: b - 10_000n, to });

    enable();

    fetchTotals();
    fetchBalanceOf(DEFAULT_TOKENS.icp, myDepositAccount.owner, myDepositAccount.subaccount);
    logInfo(`Successfully withdrawn ${E8s.new(b).toString()} ICP`);
  };

  const canClaimReward: ISatslinkerStoreContext["canClaimReward"] = () => {
    if (!isAuthorized()) return false;

    if (!totals.data) return false;

    return totals.data.yourUnclaimedReward.gt(E8s.zero());
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

    return !t.yourDecideIdVerificationStatus;
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
