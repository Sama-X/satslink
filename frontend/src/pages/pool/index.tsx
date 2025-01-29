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
  const { canWithdraw, canStake, totals, fetchTotals, canClaimReward, withdraw, stake, claimReward, poolMembers, fetchPoolMembers } = useSatslinker();
  const navigate = useNavigate();

  const [withdrawModalVisible, setWithdrawModalVisible] = createSignal(false);
  const [satslinkModalVisible, setSatslinkModalVisible] = createSignal(false);
  const [claimModalVisible, setClaimModalVisible] = createSignal(false);
  const [claimLostModalVisible, setClaimLostModalVisible] = createSignal(false);
  const [recepient, setRecepient] = createSignal(Result.Err<string>(""));
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

  const satslinkoutLeftoverBlocks = () => {
    const t = totals.data;
    if (!t) return 0;

    return Number(t.yourShareTcycles.div(t.currentBlockShareFee).toBigIntBase());
  };

  const myShare = () => {
    const t = totals.data;
    if (!t) return undefined;

    if (!t.totalSharesSupply.toBool()) return undefined;

    return t.yourShareTcycles.div(t.totalSharesSupply);
  };

  const myBlockCut = () => {
    const t = totals.data;
    if (!t) return undefined;

    if (!t.totalSharesSupply.toBool()) return undefined;

    return t.currentSatslinkTokenReward
      .toDynamic()
      .toDecimals(12)
      .mul(t.yourShareTcycles)
      .div(t.totalSharesSupply)
      .toDecimals(8)
      .toE8s();
  };

  onMount(async () => {
    if (!auth.isAuthorized()) {
      navigate(ROOT.path);
      return;
    }

    fetchSubaccountOf(myPrincipal()!);
    fetchPoolMembers();
    
    // 获取 ETH 地址
    const address = await auth.getEthAddress();
    if (address) {
      setEthAddress(bytesToHex(new Uint8Array(address)));
    }
  });

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

  const handleWithdrawModalClose = () => {
    batch(() => {
      setRecepient(Result.Err<string>(""));
      setWithdrawModalVisible(false);
    });
  };

  const handleWithdraw = async () => {
    await withdraw(Principal.fromText(recepient().unwrapOk()));
    handleWithdrawModalClose();
  };

  const withdrawForm = (
    <div class="flex flex-col gap-8">
      <div class="flex flex-col gap-4">
        <p class="font-normal text-lg text-white">Are you sure you want to withdraw all ICP from the Pool?</p>
        <div class="flex flex-col gap-2">
          <p class="font-semibold text-sm text-gray-140">
            Recepient Principal ID <span class="text-errorRed">*</span>
          </p>
          <TextInput
            placeholder={import.meta.env.VITE_SATSLINKER_CANISTER_ID}
            validations={[{ principal: null }, { required: null }]}
            value={recepient().unwrap()}
            onChange={setRecepient}
          />
        </div>
      </div>
      <Btn text="Confirm" bgColor={COLORS.orange} disabled={recepient().isErr()} onClick={handleWithdraw} />
    </div>
  );

  const handleSatslinkModalClose = () => {
    batch(() => {
      setStakeAmount(Result.Err<number>(0));
      setSatslinkModalVisible(false);
    });
  };

  const handleSatslink = async () => {
    const amount = stakeAmount().unwrapOk();
    if (amount < 0.5) {
      return;
    }
    await stake();
    handleSatslinkModalClose();
  };

  const satslinkForm = (
    <div class="flex flex-col gap-8">
      <div class="flex flex-col gap-4">
        <p class="font-normal text-lg text-white">请输入要质押的 ICP 数量：</p>
        <div class="flex flex-col gap-2">
          <p class="font-semibold text-sm text-gray-140">
            质押数量 <span class="text-errorRed">*</span>
            <span class="text-gray-400 text-xs ml-2">(最小质押数量: 0.5 ICP)</span>
          </p>
          <QtyInput
            value={stakeAmount().unwrap()}
            onChange={setStakeAmount}
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
          <div class="bg-gray-900 rounded p-3 break-all text-gray-300 font-mono text-sm">
            {ethAddress() || "未获取到 ETH 地址"}
          </div>
          <p class="text-xs text-gray-400">
            这是您授权账户关联的 ETH 地址
          </p>
        </div>

        <div class="flex flex-col gap-2 mt-4">
          <p class="font-semibold text-sm text-gray-140">
            当前可用余额
          </p>
          <BalanceOf
            tokenId={DEFAULT_TOKENS.icp}
            owner={Principal.fromText(import.meta.env.VITE_SATSLINKER_CANISTER_ID)}
            subaccount={mySubaccount()!}
          />
        </div>

        <p class="font-semibold text-orange mt-4">
          质押操作需要一定时间，请耐心等待交易完成。
        </p>
      </div>
      <div class="flex gap-2">
        <Btn text="取消" class="flex-grow" bgColor={COLORS.gray[105]} onClick={handleSatslinkModalClose} />
        <Btn 
          text="确认质押" 
          class="flex-grow" 
          bgColor={COLORS.orange} 
          onClick={handleSatslink}
          disabled={stakeAmount().isErr() || !ethAddress()} 
        />
      </div>
    </div>
  );

  const handleClaimModalClose = () => {
    batch(() => {
      setRecepient(Result.Err<string>(""));
      setClaimModalVisible(false);
    });
  };

  const handleClaim = async () => {
    await claimReward(Principal.fromText(recepient().unwrapOk()));
    handleClaimModalClose();
  };

  const claimForm = (
    <div class="flex flex-col gap-8">
      <div class="flex flex-col gap-4">
        <p class="font-normal text-lg text-white">Mint all unclaimed SATSLINK tokens?</p>
        <div class="flex flex-col gap-2">
          <p class="font-normal text-sm text-white">
            $SATSLINK is supported by an absolute majority of wallets. We still would like to kindly ask you to{" "}
            <span class="font-bold">check if the wallet you send to supports $SATSLINK</span>.
          </p>
          <p class="font-semibold text-sm text-gray-140">
            Recepient Principal ID <span class="text-errorRed">*</span>
          </p>
          <TextInput
            placeholder={import.meta.env.VITE_SATSLINKER_CANISTER_ID}
            validations={[{ principal: null }, { required: null }]}
            value={recepient().unwrap()}
            onChange={setRecepient}
          />
        </div>
      </div>
      <Btn text="Confirm" bgColor={COLORS.orange} disabled={recepient().isErr()} onClick={handleClaim} />
    </div>
  );

  const handleClaimLostModalClose = () => {
    batch(() => {
      setRecepient(Result.Err<string>(""));
      setClaimLostModalVisible(false);
    });
  };

  const handleClaimLost = async () => {
    auth.assertAuthorized();

    await claimLost(Principal.fromText(recepient().unwrapOk()));

    handleClaimLostModalClose();
  };

  const claimLostForm = (
    <div class="flex flex-col gap-8">
      <div class="flex flex-col gap-4">
        <p class="font-normal text-lg text-white">Your lost assets we were able to find:</p>
        <div class="flex flex-col gap-2">
          <BalanceOf tokenId={DEFAULT_TOKENS.satslink} owner={auth.identity()?.getPrincipal()} />
          <BalanceOf tokenId={DEFAULT_TOKENS.icp} owner={auth.identity()?.getPrincipal()} />
        </div>
        <div class="flex flex-col gap-2">
          <p class="font-semibold text-sm text-gray-140">
            Recepient Principal ID <span class="text-errorRed">*</span>
          </p>
          <TextInput
            placeholder={import.meta.env.VITE_SATSLINKER_CANISTER_ID}
            validations={[{ principal: null }, { required: null }]}
            value={recepient().unwrap()}
            onChange={setRecepient}
          />
        </div>
      </div>
      <Btn
        text="Re-claim Lost Assets"
        bgColor={COLORS.orange}
        disabled={recepient().isErr() || !canClaimLost()}
        onClick={handleClaimLost}
      />
    </div>
  );

  return (
    <Page>
      <div class="flex flex-col gap-8 p-4 md:p-8">
        {/* 数据统计卡片 */}
        <div class="grid grid-cols-1 md:grid-cols-4 gap-4">
          <div class="bg-gray-800 rounded-lg p-6">
            <h3 class="text-gray-400 mb-2">账户余额</h3>
            <Show when={auth.isAuthorized() && auth.identity()}>
              <div class="text-2xl font-bold text-white">
                <BalanceOf
                  tokenId={DEFAULT_TOKENS.icp}
                  owner={auth.identity()!.getPrincipal()}
                />
              </div>
            </Show>
          </div>
          <div class="bg-gray-800 rounded-lg p-6">
            <h3 class="text-gray-400 mb-2">总质押量</h3>
            <p class="text-2xl font-bold text-white">
              {totals.data?.totalPledgeTokenSupply.toString() || "0"} ICP
            </p>
          </div>
          <div class="bg-gray-800 rounded-lg p-6">
            <h3 class="text-gray-400 mb-2">我的份额</h3>
            <p class="text-2xl font-bold text-white">
              {myShare()?.toString() || "0"} %
            </p>
          </div>
          <div class="bg-gray-800 rounded-lg p-6">
            <h3 class="text-gray-400 mb-2">未领取奖励</h3>
            <p class="text-2xl font-bold text-orange">
              {totals.data?.yourVipUnclaimedRewardE8s.toString() || "0"} SATSLINK
            </p>
          </div>
        </div>

        {/* 操作按钮组 */}
        <div class="flex flex-wrap gap-4">
          <Btn
            text="质押 ICP"
            bgColor={COLORS.orange}
            disabled={!canStake()}
            onClick={() => setSatslinkModalVisible(true)}
            class="flex-1 min-w-[200px]"
          />
          <Btn
            text="提取 ICP"
            bgColor={COLORS.gray[105]}
            disabled={!canWithdraw()}
            onClick={() => setWithdrawModalVisible(true)}
            class="flex-1 min-w-[200px]"
          />
          <Btn
            text="领取奖励"
            bgColor={COLORS.orange}
            disabled={!canClaimReward()}
            onClick={() => setClaimModalVisible(true)}
            class="flex-1 min-w-[200px]"
          />
        </div>

        {/* 池子成员列表 */}
        <div class="bg-gray-800 rounded-lg p-6">
          <h2 class={headerClass + " mb-4"}>用户列表</h2>
          <div class="overflow-x-auto">
            <table class="w-full">
              <thead>
                <tr class="text-left text-gray-400">
                  <th class="p-2">地址</th>
                  <th class="p-2">到期时间</th>
                  <th class="p-2">未领取奖励</th>
                  <th class="p-2">状态</th>
                </tr>
              </thead>
              <tbody>
                <For each={poolMembers()}>
                  {(member) => (
                    <tr class="border-t border-gray-700">
                      <td class="p-2">
                        <Copyable text={member.id.toText()} />
                      </td>
                      <td class="p-2">{new Date(Number(member.share)).toLocaleString()}</td>
                      <td class="p-2">{member.unclaimedReward.toString()}</td>
                      <td class="p-2">
                        {member.isVerifiedViaDecideID ? (
                          <span class="text-green-500">已验证</span>
                        ) : (
                          <span class="text-gray-400">未验证</span>
                        )}
                      </td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </div>
        </div>

        {/* 现有的 Modal 组件 */}
        <Show when={withdrawModalVisible()}>
          <Modal title="提取 ICP" onClose={handleWithdrawModalClose}>
            {withdrawForm}
          </Modal>
        </Show>

        <Show when={satslinkModalVisible()}>
          <Modal title="质押 ICP" onClose={handleSatslinkModalClose}>
            {satslinkForm}
          </Modal>
        </Show>

        <Show when={claimModalVisible()}>
          <Modal title="领取奖励" onClose={handleClaimModalClose}>
            {claimForm}
          </Modal>
        </Show>

        <Show when={claimLostModalVisible()}>
          <Modal title="找回丢失资产" onClose={handleClaimLostModalClose}>
            {claimLostForm}
          </Modal>
        </Show>
      </div>
    </Page>
  );
};
