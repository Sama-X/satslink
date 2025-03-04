import { createContext, createEffect, createSignal, onCleanup, on, onMount, useContext } from "solid-js";
import { IChildren } from "../utils/types";
import { ErrorCode, err, logErr, logInfo } from "../utils/error";
import { createStore, Store } from "solid-js/store";
import { newSatslinkerActor } from "@utils/backend";
import { Principal } from "@dfinity/principal";
import { useAuth } from "./auth";
import { DEFAULT_TOKENS, useTokens } from "./tokens";
import { E8s, EDs } from "@utils/math";
import { debugStringify } from "@utils/encoding";
import { ICP_INDEX_TOKEN_IDX } from "@fort-major/msq-shared";


export interface IPaymentRecord {
  principal: Principal;
  canister_id: string;
  eth_address: string;
  expiry_time: bigint;
  amount: bigint;
  payment_create: bigint;
}

export interface IPaymentStats {
  user_payments : Array<IPaymentRecord>,
  user_vip_expiry : bigint,
  total_usd_value_all_users : number,
  total_usd_value_user : number,
  user_total_amount : bigint,
  all_payments : Array<IPaymentRecord>,
}

// 新的 Store 仅处理支付相关操作
export interface ISatslinkerStoreContext {
  paymentsStore: Store<{ allPayments?: IPaymentStats }>;
  getMyPayAccount: () => { owner: Principal; subaccount?: Uint8Array } | undefined;

  fetchAllPayments: () => Promise<void>;

  myPayments: () => IPaymentRecord[];
  fetchMyPayments: () => Promise<void>;

  paymentUserCount: () => number;
  fetchPaymentUserCount: () => Promise<void>;

  canPay: () => boolean;
  pay: (amount: bigint, eth_address: string, canister_id: string) => Promise<void>;

  fetchPaymentsByEthAddress: (eth_address: string) =>  Promise<IPaymentRecord[]>; 
}

const SatslinkerContext = createContext<ISatslinkerStoreContext>();

export function useSatslinker(): ISatslinkerStoreContext {
  const ctx = useContext(SatslinkerContext);
  if (!ctx) {
    err(ErrorCode.UNREACHEABLE, "Satslinker context is not initialized");
  }
  return ctx!;
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

  // 使用 Signal 保存支付相关的数据
  const [int, setInt] = createSignal<NodeJS.Timeout>();
  const [paymentsStore, setPaymentsStore] = createStore<ISatslinkerStoreContext["paymentsStore"]>();
  const [myPayments, setMyPayments] = createSignal<IPaymentRecord[]>([]);
  const [paymentUserCount, setPaymentUserCount] = createSignal<number>(0);
  const { subaccounts, approve, fetchSubaccountOf, balanceOf, fetchBalanceOf } = useTokens();
  
  onMount(() => {
    const t = setInterval(() => {
      fetchAllPayments();
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

      fetchAllPayments();
    })
  );

  createEffect(
    on(agent, (a) => {
      if (a!) {
        fetchSubaccountOf(identity()!.getPrincipal());
        fetchAllPayments();

        if (authProvider() === "MSQ") {
          //fetchCanMigrateMsqAccount();
        }
      }
    })
  );

  // 获取所有 ICP 支付记录
  const fetchAllPayments: ISatslinkerStoreContext["fetchAllPayments"] = async () => {
    const ag = agent() ? agent()! : anonymousAgent()!;
    const satslinker = newSatslinkerActor(ag);
    try {
      const result = await satslinker.get_payment_stats();
      if ("Ok" in result) {
        setPaymentsStore("allPayments", result as unknown as IPaymentStats);
      } else {
        logErr("FETCH_ALL_PAYMENTS" as ErrorCode, `Error: ${result.Err}`);
      }
    } catch (error: any) {
      logErr("GET_ALL_PAYMENTS" as ErrorCode, `Error: ${error.message || error}`);
    }
  };

  // 获取当前用户的支付记录
  const fetchMyPayments: ISatslinkerStoreContext["fetchMyPayments"] = async () => {
    assertAuthorized();

    const principal = identity()!.getPrincipal();
    const ag = agent() ? agent()! : anonymousAgent()!;
    const satslinker = newSatslinkerActor(ag);
    try {
      const payments = await satslinker.get_payments_by_eth_principal(principal.toString());
      setMyPayments(payments as unknown as IPaymentRecord[]);
    } catch (error: any) {
      logErr("GET_MY_PAYMENTS" as unknown as ErrorCode, `Error: ${error.message || error}`);
    }
  };

  // 获取支付用户数量（转换 bigint 为 number）
  const fetchPaymentUserCount: ISatslinkerStoreContext["fetchPaymentUserCount"] = async () => {
    assertAuthorized();

    const ag = agent() ? agent()! : anonymousAgent()!;
    const satslinker = newSatslinkerActor(ag);
    try {
      const countBig = await satslinker.count_payment_users();
      setPaymentUserCount(Number(countBig));
    } catch (error: any) {
      logErr("COUNT_PAYMENT_USERS" as unknown as ErrorCode, `Error: ${error.message || error}`);
    }
  };

  // 调用支付接口，principal 由当前身份决定
  const pay: ISatslinkerStoreContext["pay"] = async (amount, eth_address, canister_id) => {
    assertAuthorized();

    const principal = identity()!.getPrincipal();
    const satslinker = newSatslinkerActor(agent()!);
    if (amount <= 0n) {
      logErr("PAY" as unknown as ErrorCode, "无效的支付金额");
    }
    try {
      //实现调用useTokens 里面的approve
      const tokenId: Principal = DEFAULT_TOKENS.icp;
      // if (tokenType == 'ICRC'){
      //   tokenId = 
      // }
      const amountForApproval = BigInt(Math.floor(Number(amount) * 100000000)) + BigInt(10000);
      // 调用 approve 方法
      await approve(tokenId, Principal.fromText(import.meta.env.VITE_SATSLINKER_CANISTER_ID), amountForApproval);
      amount = amountForApproval - BigInt(10000); // 减去手续费
      const result = await satslinker.pay(principal, amount, eth_address, tokenId.toText());
      if ("Err" in result && result.Err) {
        logErr("PAY" as unknown as ErrorCode, (result.Err as any).toString())
      } else {
        //logInfo(`Payment succeeded. ${amount.toString()} ICP, principal=${principal.toString()}`);
      }
    } catch (error: any) {
      logErr("PAY" as unknown as ErrorCode, `Error: ${error.message || error}`)
    }
  };

  const canPay: ISatslinkerStoreContext["canPay"] = () => {
    // return true;
    const authorized = isAuthorized();
    //console.log(`用户已登录: ${authorized}`);

    const myDepositAccount = getMyPayAccount();
    //console.log(`支付账户: ${JSON.stringify(myDepositAccount)}`);

    if (!authorized || !myDepositAccount) return false;

    const balance = balanceOf(DEFAULT_TOKENS.icp, myDepositAccount.owner, myDepositAccount.subaccount);
    //console.log(`用户余额: ${balance}`);

    if (!balance || E8s.new(balance).le(E8s.f0_5())) return false;

    return true;
  };

  // 根据以太坊地址获取支付记录
  const fetchPaymentsByEthAddress: ISatslinkerStoreContext["fetchPaymentsByEthAddress"] = async (eth_address: string) => {
    assertAuthorized();

    const ag = agent() ? agent()! : anonymousAgent()!;
    const satslinker = newSatslinkerActor(ag);
    try {
      const payments = await satslinker.get_payments_by_eth_address(eth_address);
      setMyPayments(payments as unknown as IPaymentRecord[]);
      return payments as unknown as IPaymentRecord[];
    } catch (error: any) {
      logErr("GET_PAYMENTS_BY_ETH_ADDRESS" as ErrorCode, `Error: ${error.message || error}`);
      return [] as IPaymentRecord[];
    }
  };

  const getMyPayAccount: ISatslinkerStoreContext["getMyPayAccount"] = () => {
    if (!isAuthorized()) return undefined;

    const principal = identity()!.getPrincipal(); // 获取当前用户的 Principal
    const mySubaccount = subaccounts[principal.toText()];
    if (!mySubaccount) return undefined;

    //return { owner: Principal.fromText(import.meta.env.VITE_SATSLINKER_CANISTER_ID), subaccount: mySubaccount };
    return { owner: principal, subaccount: mySubaccount };
  };

  const storeContext: ISatslinkerStoreContext = {
    paymentsStore,
    myPayments,
    paymentUserCount,
    fetchAllPayments,
    fetchMyPayments,
    fetchPaymentUserCount,
    pay,
    canPay,
    fetchPaymentsByEthAddress,
    getMyPayAccount,
  };

  return (
    <SatslinkerContext.Provider value={storeContext}>
      {props.children}
    </SatslinkerContext.Provider>
  );
}
