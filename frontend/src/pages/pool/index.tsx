import { ROOT } from "@/routes";
import { Avatar } from "@components/avatar";
import { BalanceOf } from "@components/balance-of";
import { Btn } from "@components/btn";
import { Copyable } from "@components/copyable";
import { EIconKind, Icon } from "@components/icon";
import { Modal } from "@components/modal";
import { Page } from "@components/page";
import { QtyInput } from "@components/qty-input";
import { getAvatarSrc, getPseudonym, ProfileFull } from "@components/profile/profile";
import { Spoiler } from "@components/spoiler";
import { TextInput } from "@components/text-input";
import { AccountIdentifier, SubAccount } from "@dfinity/ledger-icp";
import { IcrcLedgerCanister } from "@dfinity/ledger-icrc";
import { Principal } from "@dfinity/principal";
import { MsqClient } from "@fort-major/msq-client";
import { areWeOnMobile } from "@pages/home";
import { useNavigate } from "@solidjs/router";
import { useAuth } from "@store/auth";
import { useSatslinker } from "@store/satslinker";
import { DEFAULT_TOKENS, useTokens } from "@store/tokens";
import { COLORS } from "@utils/colors";
import { avatarSrcFromPrincipal } from "@utils/common";
import { bytesToHex, tokensToStr } from "@utils/encoding";
import { logInfo } from "@utils/error";
import { eventHandler } from "@utils/security";
import { ONE_MIN_NS, Result } from "@utils/types";
import { batch, createEffect, createResource, createSignal, For, Match, on, onMount, Show, Switch } from "solid-js";

export const PoolPage = () => {
  const auth = useAuth();
  const { subaccounts, fetchSubaccountOf, balanceOf, fetchBalanceOf, claimLost, canClaimLost } = useTokens();
  // const { canWithdraw, canStake, totals, fetchTotals, canClaimReward, withdraw, stake, claimReward, poolMembers, fetchPoolMembers } = useSatslinker();
  const { paymentsStore, myPayments, paymentUserCount, fetchAllPayments, fetchMyPayments, fetchPaymentUserCount, canPay, pay, fetchPaymentsByEthAddress } = useSatslinker();
  const navigate = useNavigate();

  const [satslinkModalVisible, setSatslinkModalVisible] = createSignal(false);
  const [stakeAmount, setStakeAmount] = createSignal(Result.Err<number>(0));
  const [ethAddress, setEthAddress] = createSignal("");

  const myPrincipal = () => {
    if (!auth.isAuthorized()) return undefined;

    return auth.identity()!.getPrincipal();
  };

  const mySubaccount = () => {
    const p = myPrincipal();
    if (!p) return undefined;

    return subaccounts[p.toText()];
  };

  onMount(async () => {
    if (!auth.isAuthorized()) {
      navigate(ROOT.path);
      return;
    }

    // 调用支付相关接口获取数据
    fetchSubaccountOf(myPrincipal()!);
    await fetchAllPayments();
    // await fetchMyPayments();
    // fetchPaymentUserCount();

    // 获取 ETH 地址
    // const address = await auth.getEthAddress();
    // if (address) {
    //   setEthAddress(bytesToHex(new Uint8Array(address)));
    // }
    // 打印获取到的支付数据
    console.log("Fetched payment stats:", paymentsStore.allPayments);
  });

  createEffect(() => {
    console.log("Current paymentsStore.allPayments:", paymentsStore.allPayments);
  });

  createEffect(
    on(auth.isAuthorized, async (isAuthorized) => {
      if (isAuthorized) {
        const address = await auth.getEthAddress();
        if (address) {
          setEthAddress(bytesToHex(new Uint8Array(address)));
        }
      }
    })
  );

  createEffect(
    on(auth.isAuthorized, (ready) => {

      if (!ready) {
        navigate(ROOT.path);
      }
    })
  );

  createEffect(
    on(myPrincipal, (p) => {
      if (!p) return;

      fetchSubaccountOf(p);
    })
  );

  const headerClass = "font-semibold text-2xl";

  const handleSatslinkModalClose = () => {
    batch(() => {
      setStakeAmount(Result.Err<number>(0));
      setSatslinkModalVisible(false);
    });
  };

  const handleSatslink = async () => {
    // const amount = stakeAmount().unwrapOk();
    // if (amount < 0.5) {
    //   return;
    // }
    // await stake();

    const amount = BigInt(parseFloat(stakeAmount().unwrapOk().toString()));
    if (amount < 0.5) {
      return;
    }
    await pay(amount, ethAddress(), DEFAULT_TOKENS.icp.toText());
    handleSatslinkModalClose();
  };

  const satslinkForm = (
    <div class="flex flex-col gap-8">
      <div class="flex flex-col gap-4">
        <p class="font-normal text-lg text-white">请输入支付的 ICP 数量：</p>
        <div class="flex flex-col gap-2">
          <p class="font-semibold text-sm text-gray-140">
            支付数量 <span class="text-errorRed">*</span>
            <span class="text-gray-400 text-xs ml-2">(最小支付数量: 0.5 ICP)</span>
          </p>
          <QtyInput
            value={stakeAmount().unwrap()}
            onChange={(value)=> {setStakeAmount(value);}}
            symbol="ICP"
            validations={[
              { required: null },
              { min: 0.5 }
            ]}
          />
        </div>

        <div class="flex flex-col gap-2 mt-4">
          <p class="font-semibold text-sm text-gray-140">
            ETH 地址
          </p>
          <TextInput
            placeholder="编辑 ETH 地址"
            value={ethAddress()}
            onChange={setEthAddress}
          />
          <p class="text-xs text-gray-400">
            这是您授权账户关联的 ETH 地址，您可以编辑修改
          </p>
        </div>

        <div class="flex flex-col gap-2 mt-4">
          <p class="font-semibold text-sm text-gray-140">
            当前可用余额
          </p>
          <BalanceOf
            tokenId={DEFAULT_TOKENS.icp}
            owner={myPrincipal()!}
            subaccount={mySubaccount()}
          />
        </div>

        <p class="font-semibold text-orange mt-4">
          支付操作需要一定时间，请耐心等待交易完成。
        </p>
      </div>
      <div class="flex gap-2">
        <Btn text="取消" 
          class="flex-grow" 
          bgColor={COLORS.gray[105]} 
          onClick={handleSatslinkModalClose} 
        />
        <Btn 
          text="确认支付" 
          class="flex-grow" 
          bgColor={COLORS.orange} 
          onClick={handleSatslink}
          disabled={stakeAmount().isErr() || !ethAddress() || stakeAmount().unwrap() < 0.5} 
        />
      </div>
    </div>
  );

  return (
    <Page>
      <div class="flex flex-col gap-8 p-4 md:p-8">
        {/* 数据统计卡片 */}
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div class="bg-gray-800 rounded-lg p-6">
            <h3 class="text-gray-400 mb-2">总支付</h3>
            <p class="text-2xl font-bold text-white">
              {
                paymentsStore.allPayments?.Ok
                ? (console.log("总支付:", paymentsStore.allPayments.Ok.total_usd_value_all_users),
                   paymentsStore.allPayments.Ok.total_usd_value_all_users)
                : "加载中..."
              } $
            </p>
          </div>
          {auth.isAuthorized() && (
            <>
            <div class="bg-gray-800 rounded-lg p-6">
              <h3 class="text-gray-400 mb-2">我的支付</h3>
              <p class="text-2xl font-bold text-white">
                {
                  paymentsStore.allPayments?.Ok 
                    ? (console.log("我的支付:", paymentsStore.allPayments.Ok.total_usd_value_user),
                       paymentsStore.allPayments.Ok.total_usd_value_user)
                    : "加载中..."
                } $
              </p>
            </div>
            <div class="bg-gray-800 rounded-lg p-6">
              <h3 class="text-gray-400 mb-2">截止时间</h3>
              <p class="text-2xl font-bold text-orange">
                {
                  paymentsStore.allPayments?.Ok && paymentsStore.allPayments.Ok.user_payments?.length > 0
                    ? (() => {
                      const maxExpiryTime = paymentsStore.allPayments.Ok.user_vip_expiry;
                      console.log("用户到期时间:", maxExpiryTime);
                      const expiryTime = new Date(Number(maxExpiryTime)/ 1_000_000);
                      console.log("转换后的时间:", expiryTime);
                      const month = String(expiryTime.getMonth() + 1).padStart(2, '0');
                      console.log("月:", month);
                      const day = String(expiryTime.getDate()).padStart(2, '0');
                      console.log("日:", day);
                      const year = expiryTime.getFullYear();
                      console.log("年:", year);
                      return `${month}-${day}-${year}`;
                    })()
                    : "00-00-1900"
                }
              </p>
            </div>
            </>
          )}
        </div>

        {/* 操作按钮组 */}
        <div class="flex flex-wrap gap-4">
          <Btn
            text="支付 ICP"
            bgColor={COLORS.orange}
            disabled={!canPay()}
            onClick={() => setSatslinkModalVisible(true)}
            class="flex-1 min-w-[200px]"
          />
          {/* <p>{`canPay: ${canPay()}`}</p> 调试输出 */}
        </div>

        {/* 池子成员列表 */}
        <div class="bg-gray-800 rounded-lg p-6">
          <h2 class={headerClass + " mb-4"}>用户列表</h2>
          <div class="overflow-x-auto">
            <table class="w-full">
              <thead>
                <tr class="text-left text-gray-400">
                  <th class="p-2">创建时间</th>
                  <th class="p-2">账号</th>
                  <th class="p-2">ETH地址</th>
                  <th class="p-2">到期时间</th>
                  <th class="p-2">状态</th>
                </tr>
              </thead>
              <tbody>
              <For each={paymentsStore.allPayments?.Ok?.all_payments}>
                  {(payment) => (
                      <tr class="border-t border-gray-700">
                        <td class="p-2">{(() => {
                          const paymentCreateTimestamp = Number(payment.payment_create) / 1_000_000;
                          const expiryTime = new Date(paymentCreateTimestamp);
                          const year = expiryTime.getFullYear();
                          const month = String(expiryTime.getMonth() + 1).padStart(2, '0'); // 月份从 0 开始，需要 +1
                          const day = String(expiryTime.getDate()).padStart(2, '0');
                          console.log("用户支付创建时间:", payment.payment_create, "year:", year, "month:", month, "day:", day);
                          return `${year}-${month}-${day}`;
                        })()}</td> {/* 到期时间 */}
                        <td class="p-2">
                          <Copyable text={payment.principal.toText()} /> {/* 账号 */}
                        </td>
                        <td class="p-2">{payment.eth_address}</td> {/* ETH 地址 */}
                        <td class="p-2">{(() => {
                          const expiryTimeTimestamp = Number(payment.expiry_time) / 1_000_000;
                          const expiryTime = new Date(expiryTimeTimestamp);
                          const year = expiryTime.getFullYear();
                          const month = String(expiryTime.getMonth() + 1).padStart(2, '0'); // 月份从 0 开始，需要 +1
                          const day = String(expiryTime.getDate()).padStart(2, '0');
                          console.log("用户支付到期时间:", payment.expiry_time, "year:", year, "month:", month, "day:", day);
                          return `${year}-${month}-${day}`;
                        })()}</td> {/* 到期时间 */}
                        <td class="p-2">
                          <span class="text-green-500">VIP</span> {/* 状态设置为 VIP */}
                        </td>
                      </tr>
                  )}
                </For>
              </tbody>
            </table>
          </div>
        </div>

        {/* 现有的 Modal 组件 */}
        <Show when={satslinkModalVisible()}>
          <Modal title="支付 ICP" onClose={handleSatslinkModalClose}>
            {satslinkForm}
          </Modal>
        </Show>
      </div>
    </Page>
  );
};
