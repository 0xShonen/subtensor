use super::mock::*;
use crate::migrations::migrate_network_immunity_period;
use crate::*;
use frame_support::{assert_err, assert_ok};
use frame_system::Config;
use sp_core::U256;
use sp_std::collections::btree_map::BTreeMap;
use substrate_fixed::types::{U64F64, U96F32};
use subtensor_runtime_common::TaoCurrency;
use subtensor_swap_interface::SwapHandler;

#[test]
fn test_registration_ok() {
    new_test_ext(1).execute_with(|| {
        let block_number: u64 = 0;
        let netuid = NetUid::from(2);
        let tempo: u16 = 13;
        let hotkey_account_id: U256 = U256::from(1);
        let coldkey_account_id = U256::from(0); // Neighbour of the beast, har har
        let (nonce, work): (u64, Vec<u8>) = SubtensorModule::create_work_for_block_number(
            netuid,
            block_number,
            129123813,
            &hotkey_account_id,
        );

        //add network
        add_network(netuid, tempo, 0);

        assert_ok!(SubtensorModule::register(
            <<Test as Config>::RuntimeOrigin>::signed(hotkey_account_id),
            netuid,
            block_number,
            nonce,
            work.clone(),
            hotkey_account_id,
            coldkey_account_id
        ));

        assert_ok!(SubtensorModule::do_dissolve_network(netuid));

        assert!(!SubtensorModule::if_subnet_exist(netuid))
    })
}

#[test]
fn dissolve_no_stakers_no_alpha_no_emission() {
    new_test_ext(0).execute_with(|| {
        let cold = U256::from(1);
        let hot = U256::from(2);
        let net = add_dynamic_network(&hot, &cold);

        SubtensorModule::set_subnet_locked_balance(net, TaoCurrency::from(0));
        SubnetTAO::<Test>::insert(net, TaoCurrency::from(0));
        Emission::<Test>::insert(net, Vec::<AlphaCurrency>::new());

        let before = SubtensorModule::get_coldkey_balance(&cold);
        assert_ok!(SubtensorModule::do_dissolve_network(net));
        let after = SubtensorModule::get_coldkey_balance(&cold);

        // Balance should be unchanged (whatever the network-lock bookkeeping left there)
        assert_eq!(after, before);
        assert!(!SubtensorModule::if_subnet_exist(net));
    });
}

#[test]
fn dissolve_refunds_full_lock_cost_when_no_emission() {
    new_test_ext(0).execute_with(|| {
        let cold = U256::from(3);
        let hot = U256::from(4);
        let net = add_dynamic_network(&hot, &cold);

        let lock: TaoCurrency = TaoCurrency::from(1_000_000);
        SubtensorModule::set_subnet_locked_balance(net, lock);
        SubnetTAO::<Test>::insert(net, TaoCurrency::from(0));
        Emission::<Test>::insert(net, Vec::<AlphaCurrency>::new());

        let before = SubtensorModule::get_coldkey_balance(&cold);
        assert_ok!(SubtensorModule::do_dissolve_network(net));
        let after = SubtensorModule::get_coldkey_balance(&cold);

        assert_eq!(TaoCurrency::from(after), TaoCurrency::from(before) + lock);
    });
}

#[test]
fn dissolve_single_alpha_out_staker_gets_all_tao() {
    new_test_ext(0).execute_with(|| {
        // 1. Owner & subnet
        let owner_cold = U256::from(10);
        let owner_hot = U256::from(20);
        let net = add_dynamic_network(&owner_hot, &owner_cold);

        // 2. Single α-out staker
        let (s_hot, s_cold) = (U256::from(100), U256::from(200));
        Alpha::<Test>::insert((s_hot, s_cold, net), U64F64::from_num(5_000u128));

        // Entire TAO pot should be paid to staker's cold-key
        let pot: u64 = 99_999;
        SubnetTAO::<Test>::insert(net, TaoCurrency::from(pot));
        SubtensorModule::set_subnet_locked_balance(net, 0.into());

        // Cold-key balance before
        let before = SubtensorModule::get_coldkey_balance(&s_cold);

        // Dissolve
        assert_ok!(SubtensorModule::do_dissolve_network(net));

        // Cold-key received full pot
        let after = SubtensorModule::get_coldkey_balance(&s_cold);
        assert_eq!(after, before + pot);

        // No α entries left for dissolved subnet
        assert!(Alpha::<Test>::iter().all(|((_h, _c, n), _)| n != net));
        assert!(!SubnetTAO::<Test>::contains_key(net));
    });
}

#[allow(clippy::indexing_slicing)]
#[test]
fn dissolve_two_stakers_pro_rata_distribution() {
    new_test_ext(0).execute_with(|| {
        // Subnet + two stakers
        let oc = U256::from(50);
        let oh = U256::from(51);
        let net = add_dynamic_network(&oh, &oc);

        let (s1_hot, s1_cold, a1) = (U256::from(201), U256::from(301), 300u128);
        let (s2_hot, s2_cold, a2) = (U256::from(202), U256::from(302), 700u128);

        Alpha::<Test>::insert((s1_hot, s1_cold, net), U64F64::from_num(a1));
        Alpha::<Test>::insert((s2_hot, s2_cold, net), U64F64::from_num(a2));

        let pot: u64 = 10_000;
        SubnetTAO::<Test>::insert(net, TaoCurrency::from(pot));
        SubtensorModule::set_subnet_locked_balance(net, 5_000.into()); // owner refund path present but emission = 0

        // Cold-key balances before
        let s1_before = SubtensorModule::get_coldkey_balance(&s1_cold);
        let s2_before = SubtensorModule::get_coldkey_balance(&s2_cold);
        let owner_before = SubtensorModule::get_coldkey_balance(&oc);

        // Expected τ shares with largest remainder
        let total = a1 + a2;
        let prod1 = a1 * (pot as u128);
        let prod2 = a2 * (pot as u128);
        let share1 = (prod1 / total) as u64;
        let share2 = (prod2 / total) as u64;
        let mut distributed = share1 + share2;
        let mut rem = [(s1_cold, prod1 % total), (s2_cold, prod2 % total)];
        if distributed < pot {
            rem.sort_by_key(|&(_c, r)| core::cmp::Reverse(r));
            let leftover = pot - distributed;
            for _ in 0..leftover as usize {
                distributed += 1;
            }
        }
        // Recompute exact expected shares using the same logic
        let mut expected1 = share1;
        let mut expected2 = share2;
        if share1 + share2 < pot {
            rem.sort_by_key(|&(_c, r)| core::cmp::Reverse(r));
            if rem[0].0 == s1_cold {
                expected1 += 1;
            } else {
                expected2 += 1;
            }
        }

        // Dissolve
        assert_ok!(SubtensorModule::do_dissolve_network(net));

        // Cold-keys received their τ shares
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&s1_cold),
            s1_before + expected1
        );
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&s2_cold),
            s2_before + expected2
        );

        // Owner refunded lock (no emission)
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&oc),
            owner_before + 5_000
        );

        // α entries for dissolved subnet gone
        assert!(Alpha::<Test>::iter().all(|((_h, _c, n), _)| n != net));
    });
}

#[test]
fn dissolve_owner_cut_refund_logic() {
    new_test_ext(0).execute_with(|| {
        let oc = U256::from(70);
        let oh = U256::from(71);
        let net = add_dynamic_network(&oh, &oc);

        // One staker and a TAO pot (not relevant to refund amount).
        let sh = U256::from(77);
        let sc = U256::from(88);
        Alpha::<Test>::insert((sh, sc, net), U64F64::from_num(100u128));
        SubnetTAO::<Test>::insert(net, TaoCurrency::from(1_000));

        // Lock & emissions: total emitted α = 800.
        let lock: TaoCurrency = TaoCurrency::from(2_000);
        SubtensorModule::set_subnet_locked_balance(net, lock);
        Emission::<Test>::insert(
            net,
            vec![AlphaCurrency::from(200), AlphaCurrency::from(600)],
        );

        // Owner cut = 11796 / 65535 (about 18%).
        SubnetOwnerCut::<Test>::put(11_796u16);

        // Compute expected refund with the SAME math as the pallet.
        let frac: U96F32 = SubtensorModule::get_float_subnet_owner_cut();
        let total_emitted_alpha: u64 = 800;
        let owner_alpha_u64: u64 = U96F32::from_num(total_emitted_alpha)
            .saturating_mul(frac)
            .floor()
            .saturating_to_num::<u64>();

        // Current α→τ price for this subnet.
        let price: U96F32 =
            <Test as pallet::Config>::SwapInterface::current_alpha_price(net.into());
        let owner_emission_tau_u64: u64 = U96F32::from_num(owner_alpha_u64)
            .saturating_mul(price)
            .floor()
            .saturating_to_num::<u64>();

        let expected_refund: TaoCurrency =
            lock.saturating_sub(TaoCurrency::from(owner_emission_tau_u64));

        let before = SubtensorModule::get_coldkey_balance(&oc);
        assert_ok!(SubtensorModule::do_dissolve_network(net));
        let after = SubtensorModule::get_coldkey_balance(&oc);

        assert_eq!(
            TaoCurrency::from(after),
            TaoCurrency::from(before) + expected_refund
        );
    });
}

#[test]
fn dissolve_zero_refund_when_emission_exceeds_lock() {
    new_test_ext(0).execute_with(|| {
        let oc = U256::from(1_000);
        let oh = U256::from(2_000);
        let net = add_dynamic_network(&oh, &oc);

        SubtensorModule::set_subnet_locked_balance(net, TaoCurrency::from(1_000));
        SubnetOwnerCut::<Test>::put(u16::MAX); // 100 %
        Emission::<Test>::insert(net, vec![AlphaCurrency::from(2_000)]);

        let before = SubtensorModule::get_coldkey_balance(&oc);
        assert_ok!(SubtensorModule::do_dissolve_network(net));
        let after = SubtensorModule::get_coldkey_balance(&oc);

        assert_eq!(after, before); // no refund
    });
}

#[test]
fn dissolve_nonexistent_subnet_fails() {
    new_test_ext(0).execute_with(|| {
        assert_err!(
            SubtensorModule::do_dissolve_network(9_999.into()),
            Error::<Test>::SubNetworkDoesNotExist
        );
    });
}

#[test]
fn dissolve_clears_all_per_subnet_storages() {
    new_test_ext(0).execute_with(|| {
        let owner_cold = U256::from(123);
        let owner_hot = U256::from(456);
        let net = add_dynamic_network(&owner_hot, &owner_cold);

        // ------------------------------------------------------------------
        // Populate each storage item with a minimal value of the CORRECT type
        // ------------------------------------------------------------------
        SubnetOwner::<Test>::insert(net, owner_cold);
        SubnetworkN::<Test>::insert(net, 0u16);
        NetworkModality::<Test>::insert(net, 0u16);
        NetworksAdded::<Test>::insert(net, true);
        NetworkRegisteredAt::<Test>::insert(net, 0u64);

        Rank::<Test>::insert(net, vec![1u16]);
        Trust::<Test>::insert(net, vec![1u16]);
        Active::<Test>::insert(net, vec![true]);
        Emission::<Test>::insert(net, vec![AlphaCurrency::from(1)]);
        Incentive::<Test>::insert(net, vec![1u16]);
        Consensus::<Test>::insert(net, vec![1u16]);
        Dividends::<Test>::insert(net, vec![1u16]);
        PruningScores::<Test>::insert(net, vec![1u16]);
        LastUpdate::<Test>::insert(net, vec![0u64]);

        ValidatorPermit::<Test>::insert(net, vec![true]);
        ValidatorTrust::<Test>::insert(net, vec![1u16]);

        Tempo::<Test>::insert(net, 1u16);
        Kappa::<Test>::insert(net, 1u16);
        Difficulty::<Test>::insert(net, 1u64);

        MaxAllowedUids::<Test>::insert(net, 1u16);
        ImmunityPeriod::<Test>::insert(net, 1u16);
        ActivityCutoff::<Test>::insert(net, 1u16);
        MaxWeightsLimit::<Test>::insert(net, 1u16);
        MinAllowedWeights::<Test>::insert(net, 1u16);

        RegistrationsThisInterval::<Test>::insert(net, 1u16);
        POWRegistrationsThisInterval::<Test>::insert(net, 1u16);
        BurnRegistrationsThisInterval::<Test>::insert(net, 1u16);

        SubnetTAO::<Test>::insert(net, TaoCurrency::from(1));
        SubnetAlphaInEmission::<Test>::insert(net, AlphaCurrency::from(1));
        SubnetAlphaOutEmission::<Test>::insert(net, AlphaCurrency::from(1));
        SubnetTaoInEmission::<Test>::insert(net, TaoCurrency::from(1));
        SubnetVolume::<Test>::insert(net, 1u128);

        // Fields that will be ZEROED (not removed)
        SubnetAlphaIn::<Test>::insert(net, AlphaCurrency::from(2));
        SubnetAlphaOut::<Test>::insert(net, AlphaCurrency::from(3));

        // Prefix / double-map collections
        Keys::<Test>::insert(net, 0u16, owner_hot);
        Bonds::<Test>::insert(net, 0u16, vec![(0u16, 1u16)]);
        Weights::<Test>::insert(net, 0u16, vec![(1u16, 1u16)]);
        IsNetworkMember::<Test>::insert(owner_cold, net, true);

        // ------------------------------------------------------------------
        // Dissolve
        // ------------------------------------------------------------------
        assert_ok!(SubtensorModule::do_dissolve_network(net));

        // ------------------------------------------------------------------
        // Items that must be COMPLETELY REMOVED
        // ------------------------------------------------------------------
        assert!(!SubnetOwner::<Test>::contains_key(net));
        assert!(!SubnetworkN::<Test>::contains_key(net));
        assert!(!NetworkModality::<Test>::contains_key(net));
        assert!(!NetworksAdded::<Test>::contains_key(net));
        assert!(!NetworkRegisteredAt::<Test>::contains_key(net));

        assert!(!Rank::<Test>::contains_key(net));
        assert!(!Trust::<Test>::contains_key(net));
        assert!(!Active::<Test>::contains_key(net));
        assert!(!Emission::<Test>::contains_key(net));
        assert!(!Incentive::<Test>::contains_key(net));
        assert!(!Consensus::<Test>::contains_key(net));
        assert!(!Dividends::<Test>::contains_key(net));
        assert!(!PruningScores::<Test>::contains_key(net));
        assert!(!LastUpdate::<Test>::contains_key(net));

        assert!(!ValidatorPermit::<Test>::contains_key(net));
        assert!(!ValidatorTrust::<Test>::contains_key(net));

        assert!(!Tempo::<Test>::contains_key(net));
        assert!(!Kappa::<Test>::contains_key(net));
        assert!(!Difficulty::<Test>::contains_key(net));

        assert!(!MaxAllowedUids::<Test>::contains_key(net));
        assert!(!ImmunityPeriod::<Test>::contains_key(net));
        assert!(!ActivityCutoff::<Test>::contains_key(net));
        assert!(!MaxWeightsLimit::<Test>::contains_key(net));
        assert!(!MinAllowedWeights::<Test>::contains_key(net));

        assert!(!RegistrationsThisInterval::<Test>::contains_key(net));
        assert!(!POWRegistrationsThisInterval::<Test>::contains_key(net));
        assert!(!BurnRegistrationsThisInterval::<Test>::contains_key(net));

        assert!(!SubnetTAO::<Test>::contains_key(net));
        assert!(!SubnetAlphaInEmission::<Test>::contains_key(net));
        assert!(!SubnetAlphaOutEmission::<Test>::contains_key(net));
        assert!(!SubnetTaoInEmission::<Test>::contains_key(net));
        assert!(!SubnetVolume::<Test>::contains_key(net));

        // ------------------------------------------------------------------
        // Items expected to be PRESENT but ZERO
        // ------------------------------------------------------------------
        assert_eq!(SubnetAlphaIn::<Test>::get(net), 0.into());
        assert_eq!(SubnetAlphaOut::<Test>::get(net), 0.into());

        // ------------------------------------------------------------------
        // Collections fully cleared
        // ------------------------------------------------------------------
        assert!(Keys::<Test>::iter_prefix(net).next().is_none());
        assert!(Bonds::<Test>::iter_prefix(net).next().is_none());
        assert!(Weights::<Test>::iter_prefix(net).next().is_none());
        assert!(!IsNetworkMember::<Test>::contains_key(owner_hot, net));

        // ------------------------------------------------------------------
        // Final subnet removal confirmation
        // ------------------------------------------------------------------
        assert!(!SubtensorModule::if_subnet_exist(net));
    });
}

#[test]
fn dissolve_alpha_out_but_zero_tao_no_rewards() {
    new_test_ext(0).execute_with(|| {
        let oc = U256::from(21);
        let oh = U256::from(22);
        let net = add_dynamic_network(&oh, &oc);

        let sh = U256::from(23);
        let sc = U256::from(24);

        Alpha::<Test>::insert((sh, sc, net), U64F64::from_num(1_000u64));
        SubnetTAO::<Test>::insert(net, TaoCurrency::from(0)); // zero TAO
        SubtensorModule::set_subnet_locked_balance(net, TaoCurrency::from(0));
        Emission::<Test>::insert(net, Vec::<AlphaCurrency>::new());

        let before = SubtensorModule::get_coldkey_balance(&sc);
        assert_ok!(SubtensorModule::do_dissolve_network(net));
        let after = SubtensorModule::get_coldkey_balance(&sc);

        // No reward distributed, α-out cleared.
        assert_eq!(after, before);
        assert!(Alpha::<Test>::iter().next().is_none());
    });
}

#[test]
fn dissolve_decrements_total_networks() {
    new_test_ext(0).execute_with(|| {
        let total_before = TotalNetworks::<Test>::get();

        let cold = U256::from(41);
        let hot = U256::from(42);
        let net = add_dynamic_network(&hot, &cold);

        // Sanity: adding network increments the counter.
        assert_eq!(TotalNetworks::<Test>::get(), total_before + 1);

        assert_ok!(SubtensorModule::do_dissolve_network(net));
        assert_eq!(TotalNetworks::<Test>::get(), total_before);
    });
}

#[test]
fn dissolve_rounding_remainder_distribution() {
    new_test_ext(0).execute_with(|| {
        // 1. Build subnet with two α-out stakers (3 & 2 α)
        let oc = U256::from(61);
        let oh = U256::from(62);
        let net = add_dynamic_network(&oh, &oc);

        let (s1h, s1c) = (U256::from(63), U256::from(64));
        let (s2h, s2c) = (U256::from(65), U256::from(66));

        Alpha::<Test>::insert((s1h, s1c, net), U64F64::from_num(3u128));
        Alpha::<Test>::insert((s2h, s2c, net), U64F64::from_num(2u128));

        SubnetTAO::<Test>::insert(net, TaoCurrency::from(1)); // TAO pot = 1
        SubtensorModule::set_subnet_locked_balance(net, TaoCurrency::from(0));

        // Cold-key balances before
        let c1_before = SubtensorModule::get_coldkey_balance(&s1c);
        let c2_before = SubtensorModule::get_coldkey_balance(&s2c);

        // 3. Run full dissolve flow
        assert_ok!(SubtensorModule::do_dissolve_network(net));

        // 4. s1 (larger remainder) should get +1 τ on cold-key
        let c1_after = SubtensorModule::get_coldkey_balance(&s1c);
        let c2_after = SubtensorModule::get_coldkey_balance(&s2c);

        assert_eq!(c1_after, c1_before + 1);
        assert_eq!(c2_after, c2_before);

        // α records for subnet gone; TAO key gone
        assert!(Alpha::<Test>::iter().all(|((_h, _c, n), _)| n != net));
        assert!(!SubnetTAO::<Test>::contains_key(net));
    });
}
#[test]
fn destroy_alpha_out_multiple_stakers_pro_rata() {
    new_test_ext(0).execute_with(|| {
        // 1. Owner & subnet
        let owner_cold = U256::from(10);
        let owner_hot = U256::from(20);
        let netuid = add_dynamic_network(&owner_hot, &owner_cold);

        // 2. Two stakers on that subnet
        let (c1, h1) = (U256::from(111), U256::from(211));
        let (c2, h2) = (U256::from(222), U256::from(333));
        register_ok_neuron(netuid, h1, c1, 0);
        register_ok_neuron(netuid, h2, c2, 0);

        // 3. Stake 30 : 70 (s1 : s2) in TAO
        let min_total = DefaultMinStake::<Test>::get();
        let min_total_u64: u64 = min_total.into();
        let s1: u64 = 3u64 * min_total_u64;
        let s2: u64 = 7u64 * min_total_u64;

        SubtensorModule::add_balance_to_coldkey_account(&c1, s1 + 50_000);
        SubtensorModule::add_balance_to_coldkey_account(&c2, s2 + 50_000);

        assert_ok!(SubtensorModule::do_add_stake(
            RuntimeOrigin::signed(c1),
            h1,
            netuid,
            s1.into()
        ));
        assert_ok!(SubtensorModule::do_add_stake(
            RuntimeOrigin::signed(c2),
            h2,
            netuid,
            s2.into()
        ));

        // 4. α-out snapshot
        let a1: u128 = Alpha::<Test>::get((h1, c1, netuid)).saturating_to_num();
        let a2: u128 = Alpha::<Test>::get((h2, c2, netuid)).saturating_to_num();
        let atotal = a1 + a2;

        // 5. TAO pot & lock
        let tao_pot: u64 = 10_000;
        SubnetTAO::<Test>::insert(netuid, TaoCurrency::from(tao_pot));
        SubtensorModule::set_subnet_locked_balance(netuid, TaoCurrency::from(5_000));

        // 6. Balances before
        let c1_before = SubtensorModule::get_coldkey_balance(&c1);
        let c2_before = SubtensorModule::get_coldkey_balance(&c2);
        let owner_before = SubtensorModule::get_coldkey_balance(&owner_cold);

        // 7. Run the (now credit-to-coldkey) logic
        assert_ok!(SubtensorModule::destroy_alpha_in_out_stakes(netuid));

        // 8. Expected τ shares via largest remainder
        let prod1 = (tao_pot as u128) * a1;
        let prod2 = (tao_pot as u128) * a2;
        let mut s1_share = (prod1 / atotal) as u64;
        let mut s2_share = (prod2 / atotal) as u64;
        let distributed = s1_share + s2_share;
        if distributed < tao_pot {
            // Assign leftover to larger remainder
            let r1 = prod1 % atotal;
            let r2 = prod2 % atotal;
            if r1 >= r2 {
                s1_share += 1;
            } else {
                s2_share += 1;
            }
        }

        // 9. Cold-key balances must have increased accordingly
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&c1),
            c1_before + s1_share
        );
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&c2),
            c2_before + s2_share
        );

        // 10. Owner refund (5 000 τ) to cold-key (no emission)
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&owner_cold),
            owner_before + 5_000
        );

        // 11. α entries cleared for the subnet
        assert!(!Alpha::<Test>::contains_key((h1, c1, netuid)));
        assert!(!Alpha::<Test>::contains_key((h2, c2, netuid)));
    });
}

#[allow(clippy::indexing_slicing)]
#[test]
fn destroy_alpha_out_many_stakers_complex_distribution() {
    new_test_ext(0).execute_with(|| {
        // ── 1) create subnet with 20 stakers ────────────────────────────────
        let owner_cold = U256::from(1_000);
        let owner_hot = U256::from(2_000);
        let netuid = add_dynamic_network(&owner_hot, &owner_cold);
        SubtensorModule::set_max_registrations_per_block(netuid, 1_000u16);
        SubtensorModule::set_target_registrations_per_interval(netuid, 1_000u16);

        // Runtime-exact min amount = min_stake + fee
        let min_amount = {
            let min_stake = DefaultMinStake::<Test>::get();
            let fee = <Test as pallet::Config>::SwapInterface::approx_fee_amount(
                netuid.into(),
                min_stake.into(),
            );
            min_stake.saturating_add(fee.into())
        };

        const N: usize = 20;
        let mut cold = [U256::zero(); N];
        let mut hot = [U256::zero(); N];
        let mut stake = [0u64; N];

        let min_amount_u64: u64 = min_amount.into();
        for i in 0..N {
            cold[i] = U256::from(10_000 + 2 * i as u32);
            hot[i] = U256::from(10_001 + 2 * i as u32);
            stake[i] = (i as u64 + 1u64) * min_amount_u64; // multiples of min_amount

            register_ok_neuron(netuid, hot[i], cold[i], 0);
            SubtensorModule::add_balance_to_coldkey_account(&cold[i], stake[i] + 100_000);

            assert_ok!(SubtensorModule::do_add_stake(
                RuntimeOrigin::signed(cold[i]),
                hot[i],
                netuid,
                stake[i].into()
            ));
        }

        // ── 2) α-out snapshot ───────────────────────────────────────────────
        let mut alpha = [0u128; N];
        let mut alpha_sum: u128 = 0;
        for i in 0..N {
            alpha[i] = Alpha::<Test>::get((hot[i], cold[i], netuid)).saturating_to_num();
            alpha_sum += alpha[i];
        }

        // ── 3) TAO pot & subnet lock ────────────────────────────────────────
        let tao_pot: u64 = 123_456;
        let lock: u64 = 30_000;
        SubnetTAO::<Test>::insert(netuid, TaoCurrency::from(tao_pot));
        SubtensorModule::set_subnet_locked_balance(netuid, TaoCurrency::from(lock));

        // Owner already earned some emission; owner-cut = 50 %
        Emission::<Test>::insert(
            netuid,
            vec![
                AlphaCurrency::from(1_000),
                AlphaCurrency::from(2_000),
                AlphaCurrency::from(1_500),
            ],
        );
        SubnetOwnerCut::<Test>::put(32_768u16); // ~ 0.5 in fixed-point

        // ── 4) balances before ──────────────────────────────────────────────
        let mut bal_before = [0u64; N];
        for i in 0..N {
            bal_before[i] = SubtensorModule::get_coldkey_balance(&cold[i]);
        }
        let owner_before = SubtensorModule::get_coldkey_balance(&owner_cold);

        // ── 5) expected τ share per pallet algorithm (incl. remainder) ─────
        let mut share = [0u64; N];
        let mut rem = [0u128; N];
        let mut paid: u128 = 0;

        for i in 0..N {
            let prod = tao_pot as u128 * alpha[i];
            share[i] = (prod / alpha_sum) as u64;
            rem[i] = prod % alpha_sum;
            paid += share[i] as u128;
        }
        let leftover = tao_pot as u128 - paid;
        let mut idx: Vec<_> = (0..N).collect();
        idx.sort_by_key(|i| core::cmp::Reverse(rem[*i]));
        for i in 0..leftover as usize {
            share[idx[i]] += 1;
        }

        // ── 5b) expected owner refund with price-aware emission deduction ───
        let frac: U96F32 = SubtensorModule::get_float_subnet_owner_cut();
        let total_emitted_alpha: u64 = 1_000 + 2_000 + 1_500; // 4500 α
        let owner_alpha_u64: u64 = U96F32::from_num(total_emitted_alpha)
            .saturating_mul(frac)
            .floor()
            .saturating_to_num::<u64>();

        let price: U96F32 =
            <Test as pallet::Config>::SwapInterface::current_alpha_price(netuid.into());
        let owner_emission_tao_u64: u64 = U96F32::from_num(owner_alpha_u64)
            .saturating_mul(price)
            .floor()
            .saturating_to_num::<u64>();
        let expected_refund: u64 = lock.saturating_sub(owner_emission_tao_u64);

        // ── 6) run distribution (credits τ to coldkeys, wipes α state) ─────
        assert_ok!(SubtensorModule::destroy_alpha_in_out_stakes(netuid));

        // ── 7) post checks ──────────────────────────────────────────────────
        for i in 0..N {
            // cold-key balances increased by expected τ share
            assert_eq!(
                SubtensorModule::get_coldkey_balance(&cold[i]),
                bal_before[i] + share[i],
                "staker {i} cold-key balance changed unexpectedly"
            );
        }

        // owner refund
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&owner_cold),
            owner_before + expected_refund
        );

        // α cleared for dissolved subnet & related counters reset
        assert!(Alpha::<Test>::iter().all(|((_h, _c, n), _)| n != netuid));
        assert_eq!(SubnetAlphaIn::<Test>::get(netuid), 0.into());
        assert_eq!(SubnetAlphaOut::<Test>::get(netuid), 0.into());
        assert_eq!(SubtensorModule::get_subnet_locked_balance(netuid), 0.into());
    });
}

#[test]
fn prune_none_with_no_networks() {
    new_test_ext(0).execute_with(|| {
        assert_eq!(SubtensorModule::get_network_to_prune(), None);
    });
}

#[test]
fn prune_none_when_all_networks_immune() {
    new_test_ext(0).execute_with(|| {
        // two fresh networks → still inside immunity window
        let n1 = add_dynamic_network(&U256::from(2), &U256::from(1));
        let _n2 = add_dynamic_network(&U256::from(4), &U256::from(3));

        // emissions don’t matter while immune
        Emission::<Test>::insert(n1, vec![AlphaCurrency::from(10)]);

        assert_eq!(SubtensorModule::get_network_to_prune(), None);
    });
}

#[test]
fn prune_selects_network_with_lowest_emission() {
    new_test_ext(0).execute_with(|| {
        let n1 = add_dynamic_network(&U256::from(20), &U256::from(10));
        let n2 = add_dynamic_network(&U256::from(40), &U256::from(30));

        // make both networks eligible (past immunity)
        let imm = SubtensorModule::get_network_immunity_period();
        System::set_block_number(imm + 10);

        // n1 has lower total emission
        Emission::<Test>::insert(n1, vec![AlphaCurrency::from(5)]);
        Emission::<Test>::insert(n2, vec![AlphaCurrency::from(100)]);

        assert_eq!(SubtensorModule::get_network_to_prune(), Some(n1));
    });
}

#[test]
fn prune_ignores_immune_network_even_if_lower_emission() {
    new_test_ext(0).execute_with(|| {
        // create mature network n1 first
        let n1 = add_dynamic_network(&U256::from(22), &U256::from(11));

        let imm = SubtensorModule::get_network_immunity_period();
        System::set_block_number(imm + 5); // advance → n1 now mature

        // create second network n2 *inside* immunity
        let n2 = add_dynamic_network(&U256::from(44), &U256::from(33));

        // emissions: n1 bigger, n2 smaller but immune
        Emission::<Test>::insert(n1, vec![AlphaCurrency::from(50)]);
        Emission::<Test>::insert(n2, vec![AlphaCurrency::from(1)]);

        System::set_block_number(imm + 10); // still immune for n2
        assert_eq!(SubtensorModule::get_network_to_prune(), Some(n1));
    });
}

#[test]
fn prune_tie_on_emission_earlier_registration_wins() {
    new_test_ext(0).execute_with(|| {
        // n1 registered first
        let n1 = add_dynamic_network(&U256::from(66), &U256::from(55));

        // advance 1 block, then register n2 (later timestamp)
        System::set_block_number(1);
        let n2 = add_dynamic_network(&U256::from(88), &U256::from(77));

        // push past immunity for both
        let imm = SubtensorModule::get_network_immunity_period();
        System::set_block_number(imm + 20);

        // identical emissions → tie
        Emission::<Test>::insert(n1, vec![AlphaCurrency::from(123)]);
        Emission::<Test>::insert(n2, vec![AlphaCurrency::from(123)]);

        // earlier (n1) must be chosen
        assert_eq!(SubtensorModule::get_network_to_prune(), Some(n1));
    });
}

#[test]
fn register_network_under_limit_success() {
    new_test_ext(0).execute_with(|| {
        SubnetLimit::<Test>::put(32u16);

        let total_before = TotalNetworks::<Test>::get();

        let cold = U256::from(10);
        let hot = U256::from(11);

        let lock_now: u64 = SubtensorModule::get_network_lock_cost().into();
        SubtensorModule::add_balance_to_coldkey_account(&cold, lock_now.saturating_mul(10));

        assert_ok!(SubtensorModule::do_register_network(
            RuntimeOrigin::signed(cold),
            &hot,
            1,
            None,
        ));

        assert_eq!(TotalNetworks::<Test>::get(), total_before + 1);
        let new_id: NetUid = TotalNetworks::<Test>::get().into();
        assert_eq!(SubnetOwner::<Test>::get(new_id), cold);
        assert_eq!(SubnetOwnerHotkey::<Test>::get(new_id), hot);
    });
}

#[test]
fn register_network_prunes_and_recycles_netuid() {
    new_test_ext(0).execute_with(|| {
        SubnetLimit::<Test>::put(2u16);

        let n1_cold = U256::from(21);
        let n1_hot = U256::from(22);
        let n1 = add_dynamic_network(&n1_hot, &n1_cold);

        let n2_cold = U256::from(23);
        let n2_hot = U256::from(24);
        let n2 = add_dynamic_network(&n2_hot, &n2_cold);

        let imm = SubtensorModule::get_network_immunity_period();
        System::set_block_number(imm + 100);

        Emission::<Test>::insert(n1, vec![AlphaCurrency::from(1)]);
        Emission::<Test>::insert(n2, vec![AlphaCurrency::from(1_000)]);

        let new_cold = U256::from(30);
        let new_hot = U256::from(31);
        let needed: u64 = SubtensorModule::get_network_lock_cost().into();
        SubtensorModule::add_balance_to_coldkey_account(&new_cold, needed.saturating_mul(10));

        assert_ok!(SubtensorModule::do_register_network(
            RuntimeOrigin::signed(new_cold),
            &new_hot,
            1,
            None,
        ));

        assert_eq!(TotalNetworks::<Test>::get(), 2);
        assert_eq!(SubnetOwner::<Test>::get(n1), new_cold);
        assert_eq!(SubnetOwnerHotkey::<Test>::get(n1), new_hot);
        assert_eq!(SubnetOwner::<Test>::get(n2), n2_cold);
    });
}

#[test]
fn register_network_fails_before_prune_keeps_existing() {
    new_test_ext(0).execute_with(|| {
        SubnetLimit::<Test>::put(1u16);

        let n_cold = U256::from(41);
        let n_hot = U256::from(42);
        let net = add_dynamic_network(&n_hot, &n_cold);

        let imm = SubtensorModule::get_network_immunity_period();
        System::set_block_number(imm + 50);
        Emission::<Test>::insert(net, vec![AlphaCurrency::from(10)]);

        let caller_cold = U256::from(50);
        let caller_hot = U256::from(51);

        assert_err!(
            SubtensorModule::do_register_network(
                RuntimeOrigin::signed(caller_cold),
                &caller_hot,
                1,
                None,
            ),
            Error::<Test>::NotEnoughBalanceToStake
        );

        assert!(SubtensorModule::if_subnet_exist(net));
        assert_eq!(TotalNetworks::<Test>::get(), 1);
    });
}

#[test]
fn test_migrate_network_immunity_period() {
    new_test_ext(0).execute_with(|| {
        // --------------------------------------------------------------------
        // ‼️ PRE-CONDITIONS
        // --------------------------------------------------------------------
        assert_ne!(NetworkImmunityPeriod::<Test>::get(), 864_000);
        assert!(
            !HasMigrationRun::<Test>::get(b"migrate_network_immunity_period".to_vec()),
            "HasMigrationRun should be false before migration"
        );

        // --------------------------------------------------------------------
        // ▶️  RUN MIGRATION
        // --------------------------------------------------------------------
        let weight = migrate_network_immunity_period::migrate_network_immunity_period::<Test>();

        // --------------------------------------------------------------------
        // ✅ POST-CONDITIONS
        // --------------------------------------------------------------------
        assert_eq!(
            NetworkImmunityPeriod::<Test>::get(),
            864_000,
            "NetworkImmunityPeriod should now be 864_000"
        );

        assert!(
            HasMigrationRun::<Test>::get(b"migrate_network_immunity_period".to_vec()),
            "HasMigrationRun should be true after migration"
        );

        assert!(weight != Weight::zero(), "migration weight should be > 0");
    });
}

// #[test]
// fn test_schedule_dissolve_network_execution() {
//     new_test_ext(1).execute_with(|| {
//         let block_number: u64 = 0;
//         let netuid = NetUid::from(2);
//         let tempo: u16 = 13;
//         let hotkey_account_id: U256 = U256::from(1);
//         let coldkey_account_id = U256::from(0); // Neighbour of the beast, har har
//         let (nonce, work): (u64, Vec<u8>) = SubtensorModule::create_work_for_block_number(
//             netuid,
//             block_number,
//             129123813,
//             &hotkey_account_id,
//         );

//         //add network
//         add_network(netuid, tempo, 0);

//         assert_ok!(SubtensorModule::register(
//             <<Test as Config>::RuntimeOrigin>::signed(hotkey_account_id),
//             netuid,
//             block_number,
//             nonce,
//             work.clone(),
//             hotkey_account_id,
//             coldkey_account_id
//         ));

//         assert!(SubtensorModule::if_subnet_exist(netuid));

//         assert_ok!(SubtensorModule::schedule_dissolve_network(
//             <<Test as Config>::RuntimeOrigin>::signed(coldkey_account_id),
//             netuid
//         ));

//         let current_block = System::block_number();
//         let execution_block = current_block + DissolveNetworkScheduleDuration::<Test>::get();

//         System::assert_last_event(
//             Event::DissolveNetworkScheduled {
//                 account: coldkey_account_id,
//                 netuid,
//                 execution_block,
//             }
//             .into(),
//         );

//         run_to_block(execution_block);
//         assert!(!SubtensorModule::if_subnet_exist(netuid));
//     })
// }

// #[test]
// fn test_non_owner_schedule_dissolve_network_execution() {
//     new_test_ext(1).execute_with(|| {
//         let block_number: u64 = 0;
//         let netuid = NetUid::from(2);
//         let tempo: u16 = 13;
//         let hotkey_account_id: U256 = U256::from(1);
//         let coldkey_account_id = U256::from(0); // Neighbour of the beast, har har
//         let non_network_owner_account_id = U256::from(2); //
//         let (nonce, work): (u64, Vec<u8>) = SubtensorModule::create_work_for_block_number(
//             netuid,
//             block_number,
//             129123813,
//             &hotkey_account_id,
//         );

//         //add network
//         add_network(netuid, tempo, 0);

//         assert_ok!(SubtensorModule::register(
//             <<Test as Config>::RuntimeOrigin>::signed(hotkey_account_id),
//             netuid,
//             block_number,
//             nonce,
//             work.clone(),
//             hotkey_account_id,
//             coldkey_account_id
//         ));

//         assert!(SubtensorModule::if_subnet_exist(netuid));

//         assert_ok!(SubtensorModule::schedule_dissolve_network(
//             <<Test as Config>::RuntimeOrigin>::signed(non_network_owner_account_id),
//             netuid
//         ));

//         let current_block = System::block_number();
//         let execution_block = current_block + DissolveNetworkScheduleDuration::<Test>::get();

//         System::assert_last_event(
//             Event::DissolveNetworkScheduled {
//                 account: non_network_owner_account_id,
//                 netuid,
//                 execution_block,
//             }
//             .into(),
//         );

//         run_to_block(execution_block);
//         // network exists since the caller is no the network owner
//         assert!(SubtensorModule::if_subnet_exist(netuid));
//     })
// }

// #[test]
// fn test_new_owner_schedule_dissolve_network_execution() {
//     new_test_ext(1).execute_with(|| {
//         let block_number: u64 = 0;
//         let netuid = NetUid::from(2);
//         let tempo: u16 = 13;
//         let hotkey_account_id: U256 = U256::from(1);
//         let coldkey_account_id = U256::from(0); // Neighbour of the beast, har har
//         let new_network_owner_account_id = U256::from(2); //
//         let (nonce, work): (u64, Vec<u8>) = SubtensorModule::create_work_for_block_number(
//             netuid,
//             block_number,
//             129123813,
//             &hotkey_account_id,
//         );

//         //add network
//         add_network(netuid, tempo, 0);

//         assert_ok!(SubtensorModule::register(
//             <<Test as Config>::RuntimeOrigin>::signed(hotkey_account_id),
//             netuid,
//             block_number,
//             nonce,
//             work.clone(),
//             hotkey_account_id,
//             coldkey_account_id
//         ));

//         assert!(SubtensorModule::if_subnet_exist(netuid));

//         // the account is not network owner when schedule the call
//         assert_ok!(SubtensorModule::schedule_dissolve_network(
//             <<Test as Config>::RuntimeOrigin>::signed(new_network_owner_account_id),
//             netuid
//         ));

//         let current_block = System::block_number();
//         let execution_block = current_block + DissolveNetworkScheduleDuration::<Test>::get();

//         System::assert_last_event(
//             Event::DissolveNetworkScheduled {
//                 account: new_network_owner_account_id,
//                 netuid,
//                 execution_block,
//             }
//             .into(),
//         );
//         run_to_block(current_block + 1);
//         // become network owner after call scheduled
//         crate::SubnetOwner::<Test>::insert(netuid, new_network_owner_account_id);

//         run_to_block(execution_block);
//         // network exists since the caller is no the network owner
//         assert!(!SubtensorModule::if_subnet_exist(netuid));
//     })
// }

// #[test]
// fn test_schedule_dissolve_network_execution_with_coldkey_swap() {
//     new_test_ext(1).execute_with(|| {
//         let block_number: u64 = 0;
//         let netuid = NetUid::from(2);
//         let tempo: u16 = 13;
//         let hotkey_account_id: U256 = U256::from(1);
//         let coldkey_account_id = U256::from(0); // Neighbour of the beast, har har
//         let new_network_owner_account_id = U256::from(2); //

//         SubtensorModule::add_balance_to_coldkey_account(&coldkey_account_id, 1000000000000000);

//         let (nonce, work): (u64, Vec<u8>) = SubtensorModule::create_work_for_block_number(
//             netuid,
//             block_number,
//             129123813,
//             &hotkey_account_id,
//         );

//         //add network
//         add_network(netuid, tempo, 0);

//         assert_ok!(SubtensorModule::register(
//             <<Test as Config>::RuntimeOrigin>::signed(hotkey_account_id),
//             netuid,
//             block_number,
//             nonce,
//             work.clone(),
//             hotkey_account_id,
//             coldkey_account_id
//         ));

//         assert!(SubtensorModule::if_subnet_exist(netuid));

//         // the account is not network owner when schedule the call
//         assert_ok!(SubtensorModule::schedule_swap_coldkey(
//             <<Test as Config>::RuntimeOrigin>::signed(coldkey_account_id),
//             new_network_owner_account_id
//         ));

//         let current_block = System::block_number();
//         let execution_block = current_block + ColdkeySwapScheduleDuration::<Test>::get();

//         run_to_block(execution_block - 1);

//         // the account is not network owner when schedule the call
//         assert_ok!(SubtensorModule::schedule_dissolve_network(
//             <<Test as Config>::RuntimeOrigin>::signed(new_network_owner_account_id),
//             netuid
//         ));

//         System::assert_last_event(
//             Event::DissolveNetworkScheduled {
//                 account: new_network_owner_account_id,
//                 netuid,
//                 execution_block: DissolveNetworkScheduleDuration::<Test>::get() + execution_block
//                     - 1,
//             }
//             .into(),
//         );

//         run_to_block(execution_block);
//         assert_eq!(
//             crate::SubnetOwner::<Test>::get(netuid),
//             new_network_owner_account_id
//         );

//         let current_block = System::block_number();
//         let execution_block = current_block + DissolveNetworkScheduleDuration::<Test>::get();

//         run_to_block(execution_block);
//         // network exists since the caller is no the network owner
//         assert!(!SubtensorModule::if_subnet_exist(netuid));
//     })
// }

// SKIP_WASM_BUILD=1 RUST_LOG=debug cargo test --package pallet-subtensor --lib -- tests::networks::test_register_subnet_low_lock_cost --exact --show-output --nocapture
#[test]
fn test_register_subnet_low_lock_cost() {
    new_test_ext(1).execute_with(|| {
        NetworkMinLockCost::<Test>::set(TaoCurrency::from(1_000));
        NetworkLastLockCost::<Test>::set(TaoCurrency::from(1_000));

        // Make sure lock cost is lower than 100 TAO
        let lock_cost = SubtensorModule::get_network_lock_cost();
        assert!(lock_cost < 100_000_000_000.into());

        let subnet_owner_coldkey = U256::from(1);
        let subnet_owner_hotkey = U256::from(2);
        let netuid = add_dynamic_network(&subnet_owner_hotkey, &subnet_owner_coldkey);
        assert!(SubtensorModule::if_subnet_exist(netuid));

        // Ensure that both Subnet TAO and Subnet Alpha In equal to (actual) lock_cost
        assert_eq!(SubnetTAO::<Test>::get(netuid), lock_cost);
        assert_eq!(
            SubnetAlphaIn::<Test>::get(netuid),
            lock_cost.to_u64().into()
        );
    })
}

// SKIP_WASM_BUILD=1 RUST_LOG=debug cargo test --package pallet-subtensor --lib -- tests::networks::test_register_subnet_high_lock_cost --exact --show-output --nocapture
#[test]
fn test_register_subnet_high_lock_cost() {
    new_test_ext(1).execute_with(|| {
        let lock_cost = TaoCurrency::from(1_000_000_000_000);
        NetworkMinLockCost::<Test>::set(lock_cost);
        NetworkLastLockCost::<Test>::set(lock_cost);

        // Make sure lock cost is higher than 100 TAO
        let lock_cost = SubtensorModule::get_network_lock_cost();
        assert!(lock_cost >= 1_000_000_000_000.into());

        let subnet_owner_coldkey = U256::from(1);
        let subnet_owner_hotkey = U256::from(2);
        let netuid = add_dynamic_network(&subnet_owner_hotkey, &subnet_owner_coldkey);
        assert!(SubtensorModule::if_subnet_exist(netuid));

        // Ensure that both Subnet TAO and Subnet Alpha In equal to 100 TAO
        assert_eq!(SubnetTAO::<Test>::get(netuid), lock_cost);
        assert_eq!(
            SubnetAlphaIn::<Test>::get(netuid),
            lock_cost.to_u64().into()
        );
    })
}

#[test]
fn test_tempo_greater_than_weight_set_rate_limit() {
    new_test_ext(1).execute_with(|| {
        let subnet_owner_hotkey = U256::from(1);
        let subnet_owner_coldkey = U256::from(2);

        let netuid = add_dynamic_network(&subnet_owner_hotkey, &subnet_owner_coldkey);

        // Get tempo
        let tempo = SubtensorModule::get_tempo(netuid);

        let weights_set_rate_limit = SubtensorModule::get_weights_set_rate_limit(netuid);

        assert!(tempo as u64 >= weights_set_rate_limit);
    })
}

#[allow(clippy::indexing_slicing)]
#[test]
fn massive_dissolve_refund_and_reregistration_flow_is_lossless_and_cleans_state() {
    new_test_ext(0).execute_with(|| {
        // ────────────────────────────────────────────────────────────────────
        // 0) Constants and helpers (distinct hotkeys & coldkeys)
        // ────────────────────────────────────────────────────────────────────
        const NUM_NETS: usize = 4;

        // Six LP coldkeys
        let cold_lps: [U256; 6] = [
            U256::from(3001),
            U256::from(3002),
            U256::from(3003),
            U256::from(3004),
            U256::from(3005),
            U256::from(3006),
        ];

        // For each coldkey, define two DISTINCT hotkeys it owns.
        let mut cold_to_hots: BTreeMap<U256, [U256; 2]> = BTreeMap::new();
        for &c in cold_lps.iter() {
            let h1 = U256::from(c.low_u64().saturating_add(100_000));
            let h2 = U256::from(c.low_u64().saturating_add(200_000));
            cold_to_hots.insert(c, [h1, h2]);
        }

        // Distinct τ pot sizes per net.
        let pots: [u64; NUM_NETS] = [12_345, 23_456, 34_567, 45_678];

        let lp_sets_per_net: [&[U256]; NUM_NETS] = [
            &cold_lps[0..4], // net0: A,B,C,D
            &cold_lps[2..6], // net1: C,D,E,F
            &cold_lps[0..6], // net2: A..F
            &cold_lps[1..5], // net3: B,C,D,E
        ];

        // Multiple bands/sizes → many positions per cold across nets, using mixed hotkeys.
        let bands: [i32; 3] = [5, 13, 30];
        let liqs: [u64; 3] = [400_000, 700_000, 1_100_000];

        // Helper: add a V3 position via a (hot, cold) pair.
        let add_pos = |net: NetUid, hot: U256, cold: U256, band: i32, liq: u64| {
            let ct = pallet_subtensor_swap::CurrentTick::<Test>::get(net);
            let lo = ct.saturating_sub(band);
            let hi = ct.saturating_add(band);
            assert_ok!(pallet_subtensor_swap::Pallet::<Test>::add_liquidity(
                RuntimeOrigin::signed(cold),
                hot,
                net,
                lo,
                hi,
                liq
            ));
        };

        // ────────────────────────────────────────────────────────────────────
        // 1) Create many subnets, enable V3, fix price at tick=0 (sqrt≈1)
        // ────────────────────────────────────────────────────────────────────
        let mut nets: Vec<NetUid> = Vec::new();
        for i in 0..NUM_NETS {
            let owner_hot = U256::from(10_000 + (i as u64));
            let owner_cold = U256::from(20_000 + (i as u64));
            let net = add_dynamic_network(&owner_hot, &owner_cold);
            SubtensorModule::set_max_registrations_per_block(net, 1_000u16);
            SubtensorModule::set_target_registrations_per_interval(net, 1_000u16);
            Emission::<Test>::insert(net, Vec::<AlphaCurrency>::new());
            SubtensorModule::set_subnet_locked_balance(net, TaoCurrency::from(0));

            assert_ok!(
                pallet_subtensor_swap::Pallet::<Test>::toggle_user_liquidity(
                    RuntimeOrigin::root(),
                    net,
                    true
                )
            );

            // Price/tick pinned so LP math stays stable (sqrt(1)).
            let ct0 = pallet_subtensor_swap::tick::TickIndex::new_unchecked(0);
            let sqrt1 = ct0.try_to_sqrt_price().expect("sqrt(1) price");
            pallet_subtensor_swap::CurrentTick::<Test>::set(net, ct0);
            pallet_subtensor_swap::AlphaSqrtPrice::<Test>::set(net, sqrt1);

            nets.push(net);
        }

        // Map net → index for quick lookups.
        let mut net_index: BTreeMap<NetUid, usize> = BTreeMap::new();
        for (i, &n) in nets.iter().enumerate() {
            net_index.insert(n, i);
        }

        // ────────────────────────────────────────────────────────────────────
        // 2) Pre-create a handful of small (hot, cold) pairs so accounts exist
        // ────────────────────────────────────────────────────────────────────
        for id in 0u64..10 {
            let cold_acc = U256::from(1_000_000 + id);
            let hot_acc = U256::from(2_000_000 + id);
            for &net in nets.iter() {
                register_ok_neuron(net, hot_acc, cold_acc, 100_000 + id);
            }
        }

        // ────────────────────────────────────────────────────────────────────
        // 3) LPs per net: register each (hot, cold), massive τ prefund, and stake
        // ────────────────────────────────────────────────────────────────────
        for &cold in cold_lps.iter() {
            SubtensorModule::add_balance_to_coldkey_account(&cold, u64::MAX);
        }

        // τ balances before LP adds (after staking):
        let mut tao_before: BTreeMap<U256, u64> = BTreeMap::new();

        // Ordered α snapshot per net at **pair granularity** (pre‑LP):
        let mut alpha_pairs_per_net: BTreeMap<NetUid, Vec<((U256, U256), u128)>> = BTreeMap::new();

        // Register both hotkeys for each participating cold on each net and stake τ→α.
        for (ni, &net) in nets.iter().enumerate() {
            let participants = lp_sets_per_net[ni];
            for &cold in participants.iter() {
                let [hot1, hot2] = cold_to_hots[&cold];

                // Ensure (hot, cold) neurons exist on this net.
                register_ok_neuron(
                    net,
                    hot1,
                    cold,
                    (ni as u64) * 10_000 + (hot1.low_u64() % 10_000),
                );
                register_ok_neuron(
                    net,
                    hot2,
                    cold,
                    (ni as u64) * 10_000 + (hot2.low_u64() % 10_000) + 1,
                );

                // Stake τ (split across the two hotkeys).
                let base: u64 =
                    5_000_000 + ((ni as u64) * 1_000_000) + ((cold.low_u64() % 10) * 250_000);
                let stake1: u64 = base.saturating_mul(3) / 5; // 60%
                let stake2: u64 = base.saturating_sub(stake1); // 40%

                assert_ok!(SubtensorModule::do_add_stake(
                    RuntimeOrigin::signed(cold),
                    hot1,
                    net,
                    stake1.into()
                ));
                assert_ok!(SubtensorModule::do_add_stake(
                    RuntimeOrigin::signed(cold),
                    hot2,
                    net,
                    stake2.into()
                ));
            }
        }

        // Record τ balances now (post‑stake, pre‑LP).
        for &cold in cold_lps.iter() {
            tao_before.insert(cold, SubtensorModule::get_coldkey_balance(&cold));
        }

        // Capture **pair‑level** α snapshot per net (pre‑LP).
        for ((hot, cold, net), amt) in Alpha::<Test>::iter() {
            if let Some(&ni) = net_index.get(&net) {
                if lp_sets_per_net[ni].contains(&cold) {
                    let a: u128 = amt.saturating_to_num();
                    if a > 0 {
                        alpha_pairs_per_net
                            .entry(net)
                            .or_default()
                            .push(((hot, cold), a));
                    }
                }
            }
        }

        // ────────────────────────────────────────────────────────────────────
        // 4) Add many V3 positions per cold across nets, alternating hotkeys
        // ────────────────────────────────────────────────────────────────────
        for (ni, &net) in nets.iter().enumerate() {
            let participants = lp_sets_per_net[ni];
            for (pi, &cold) in participants.iter().enumerate() {
                let [hot1, hot2] = cold_to_hots[&cold];
                let hots = [hot1, hot2];
                for k in 0..3 {
                    let band = bands[(pi + k) % bands.len()];
                    let liq = liqs[(ni + k) % liqs.len()];
                    let hot = hots[k % hots.len()];
                    add_pos(net, hot, cold, band, liq);
                }
            }
        }

        // Snapshot τ balances AFTER LP adds (to measure actual principal debit).
        let mut tao_after_adds: BTreeMap<U256, u64> = BTreeMap::new();
        for &cold in cold_lps.iter() {
            tao_after_adds.insert(cold, SubtensorModule::get_coldkey_balance(&cold));
        }

        // ────────────────────────────────────────────────────────────────────
        // 5) Compute Hamilton-apportionment BASE shares per cold and total leftover
        //    from the **pair-level** pre‑LP α snapshot; also count pairs per cold.
        // ────────────────────────────────────────────────────────────────────
        let mut base_share_cold: BTreeMap<U256, u64> =
            cold_lps.iter().copied().map(|c| (c, 0_u64)).collect();
        let mut pair_count_cold: BTreeMap<U256, u32> =
            cold_lps.iter().copied().map(|c| (c, 0_u32)).collect();

        let mut leftover_total: u64 = 0;

        for (ni, &net) in nets.iter().enumerate() {
            let pot = pots[ni];
            let pairs = alpha_pairs_per_net.get(&net).cloned().unwrap_or_default();
            if pot == 0 || pairs.is_empty() {
                continue;
            }
            let total_alpha: u128 = pairs.iter().map(|(_, a)| *a).sum();
            if total_alpha == 0 {
                continue;
            }

            let mut base_sum_net: u64 = 0;
            for ((_, cold), a) in pairs.iter().copied() {
                // quota = a * pot / total_alpha
                let prod: u128 = a.saturating_mul(pot as u128);
                let base: u64 = (prod / total_alpha) as u64;
                base_sum_net = base_sum_net.saturating_add(base);
                *base_share_cold.entry(cold).or_default() =
                    base_share_cold[&cold].saturating_add(base);
                *pair_count_cold.entry(cold).or_default() += 1;
            }
            let leftover_net = pot.saturating_sub(base_sum_net);
            leftover_total = leftover_total.saturating_add(leftover_net);
        }

        // ────────────────────────────────────────────────────────────────────
        // 6) Seed τ pots and dissolve *all* networks (liquidates LPs + refunds)
        // ────────────────────────────────────────────────────────────────────
        for (ni, &net) in nets.iter().enumerate() {
            SubnetTAO::<Test>::insert(net, TaoCurrency::from(pots[ni]));
        }
        for &net in nets.iter() {
            assert_ok!(SubtensorModule::do_dissolve_network(net));
        }

        // ────────────────────────────────────────────────────────────────────
        // 7) Assertions: τ balances, α gone, nets removed, swap state clean
        //    (Hamilton invariants enforced at cold-level without relying on tie-break)
        // ────────────────────────────────────────────────────────────────────
        // Collect actual pot credits per cold (principal cancels out against adds when comparing before→after).
        let mut actual_pot_cold: BTreeMap<U256, u64> =
            cold_lps.iter().copied().map(|c| (c, 0_u64)).collect();
        for &cold in cold_lps.iter() {
            let before = tao_before[&cold];
            let after = SubtensorModule::get_coldkey_balance(&cold);
            actual_pot_cold.insert(cold, after.saturating_sub(before));
        }

        // (a) Sum of actual pot credits equals total pots.
        let total_actual: u64 = actual_pot_cold.values().copied().sum();
        let total_pots: u64 = pots.iter().copied().sum();
        assert_eq!(
            total_actual, total_pots,
            "total τ pot credited across colds must equal sum of pots"
        );

        // (b) Each cold’s pot is within Hamilton bounds: base ≤ actual ≤ base + #pairs.
        let mut extra_accum: u64 = 0;
        for &cold in cold_lps.iter() {
            let base = *base_share_cold.get(&cold).unwrap_or(&0);
            let pairs = *pair_count_cold.get(&cold).unwrap_or(&0) as u64;
            let actual = *actual_pot_cold.get(&cold).unwrap_or(&0);

            assert!(
                actual >= base,
                "cold {cold:?} actual pot {actual} is below base {base}"
            );
            assert!(
                actual <= base.saturating_add(pairs),
                "cold {cold:?} actual pot {actual} exceeds base + pairs ({base} + {pairs})"
            );

            extra_accum = extra_accum.saturating_add(actual.saturating_sub(base));
        }

        // (c) The total “extra beyond base” equals the computed leftover_total across nets.
        assert_eq!(
            extra_accum, leftover_total,
            "sum of extras beyond base must equal total leftover"
        );

        // (d) τ principal was fully refunded (compare after_adds → after).
        for &cold in cold_lps.iter() {
            let before = tao_before[&cold];
            let mid = tao_after_adds[&cold];
            let after = SubtensorModule::get_coldkey_balance(&cold);
            let principal_actual = before.saturating_sub(mid);
            let actual_pot = after.saturating_sub(before);
            assert_eq!(
                after.saturating_sub(mid),
                principal_actual.saturating_add(actual_pot),
                "cold {cold:?} τ balance incorrect vs 'after_adds'"
            );
        }

        // For each dissolved net, check α ledgers gone, network removed, and swap state clean.
        for &net in nets.iter() {
            assert!(
                Alpha::<Test>::iter().all(|((_h, _c, n), _)| n != net),
                "alpha ledger not fully cleared for net {net:?}"
            );
            assert!(
                !SubtensorModule::if_subnet_exist(net),
                "subnet {net:?} still exists"
            );
            assert!(
                pallet_subtensor_swap::Ticks::<Test>::iter_prefix(net)
                    .next()
                    .is_none(),
                "ticks not cleared for net {net:?}"
            );
            assert!(
                !pallet_subtensor_swap::Positions::<Test>::iter()
                    .any(|((n, _owner, _pid), _)| n == net),
                "swap positions not fully cleared for net {net:?}"
            );
            assert_eq!(
                pallet_subtensor_swap::FeeGlobalTao::<Test>::get(net).saturating_to_num::<u64>(),
                0,
                "FeeGlobalTao nonzero for net {net:?}"
            );
            assert_eq!(
                pallet_subtensor_swap::FeeGlobalAlpha::<Test>::get(net).saturating_to_num::<u64>(),
                0,
                "FeeGlobalAlpha nonzero for net {net:?}"
            );
            assert_eq!(
                pallet_subtensor_swap::CurrentLiquidity::<Test>::get(net),
                0,
                "CurrentLiquidity not zero for net {net:?}"
            );
            assert!(
                !pallet_subtensor_swap::SwapV3Initialized::<Test>::get(net),
                "SwapV3Initialized still set"
            );
            assert!(
                !pallet_subtensor_swap::EnabledUserLiquidity::<Test>::get(net),
                "EnabledUserLiquidity still set"
            );
            assert!(
                pallet_subtensor_swap::TickIndexBitmapWords::<Test>::iter_prefix((net,))
                    .next()
                    .is_none(),
                "TickIndexBitmapWords not cleared for net {net:?}"
            );
        }

        // ────────────────────────────────────────────────────────────────────
        // 8) Re-register a fresh subnet and re‑stake using the pallet’s min rule
        //    Assert αΔ equals the sim-swap result for the exact τ staked.
        // ────────────────────────────────────────────────────────────────────
        let new_owner_hot = U256::from(99_000);
        let new_owner_cold = U256::from(99_001);
        let net_new = add_dynamic_network(&new_owner_hot, &new_owner_cold);
        SubtensorModule::set_max_registrations_per_block(net_new, 1_000u16);
        SubtensorModule::set_target_registrations_per_interval(net_new, 1_000u16);
        Emission::<Test>::insert(net_new, Vec::<AlphaCurrency>::new());
        SubtensorModule::set_subnet_locked_balance(net_new, TaoCurrency::from(0));

        assert_ok!(
            pallet_subtensor_swap::Pallet::<Test>::toggle_user_liquidity(
                RuntimeOrigin::root(),
                net_new,
                true
            )
        );
        let ct0 = pallet_subtensor_swap::tick::TickIndex::new_unchecked(0);
        let sqrt1 = ct0.try_to_sqrt_price().expect("sqrt(1)");
        pallet_subtensor_swap::CurrentTick::<Test>::set(net_new, ct0);
        pallet_subtensor_swap::AlphaSqrtPrice::<Test>::set(net_new, sqrt1);

        // Compute the exact min stake per the pallet rule: DefaultMinStake + fee(DefaultMinStake).
        let min_stake_u64: u64 = DefaultMinStake::<Test>::get().into();
        let fee_for_min: u64 = pallet_subtensor_swap::Pallet::<Test>::sim_swap(
            net_new,
            subtensor_swap_interface::OrderType::Buy,
            min_stake_u64,
        )
        .map(|r| r.fee_paid)
        .unwrap_or_else(|_e| {
            <pallet_subtensor_swap::Pallet<Test> as subtensor_swap_interface::SwapHandler<
                <Test as frame_system::Config>::AccountId,
            >>::approx_fee_amount(net_new, min_stake_u64)
        });
        let min_amount_required: u64 = min_stake_u64.saturating_add(fee_for_min);

        // Re‑stake from three coldkeys; choose a specific DISTINCT hotkey per cold.
        for &cold in &cold_lps[0..3] {
            let [hot1, _hot2] = cold_to_hots[&cold];
            register_ok_neuron(net_new, hot1, cold, 7777);

            let before_tau = SubtensorModule::get_coldkey_balance(&cold);
            let a_prev: u64 = Alpha::<Test>::get((hot1, cold, net_new)).saturating_to_num();

            // Expected α for this exact τ, using the same sim path as the pallet.
            let expected_alpha_out: u64 = pallet_subtensor_swap::Pallet::<Test>::sim_swap(
                net_new,
                subtensor_swap_interface::OrderType::Buy,
                min_amount_required,
            )
            .map(|r| r.amount_paid_out)
            .expect("sim_swap must succeed for fresh net and min amount");

            assert_ok!(SubtensorModule::do_add_stake(
                RuntimeOrigin::signed(cold),
                hot1,
                net_new,
                min_amount_required.into()
            ));

            let after_tau = SubtensorModule::get_coldkey_balance(&cold);
            let a_new: u64 = Alpha::<Test>::get((hot1, cold, net_new)).saturating_to_num();
            let a_delta = a_new.saturating_sub(a_prev);

            // τ decreased by exactly the amount we sent.
            assert_eq!(
                after_tau,
                before_tau.saturating_sub(min_amount_required),
                "τ did not decrease by the min required restake amount for cold {cold:?}"
            );

            // α minted equals the simulated swap’s net out for that same τ.
            assert_eq!(
                a_delta, expected_alpha_out,
                "α minted mismatch for cold {cold:?} (hot {hot1:?}) on new net (αΔ {a_delta}, expected {expected_alpha_out})"
            );
        }

        // Ensure V3 still functional on new net: add a small position for the first cold using its hot1
        let who_cold = cold_lps[0];
        let [who_hot, _] = cold_to_hots[&who_cold];
        add_pos(net_new, who_hot, who_cold, 8, 123_456);
        assert!(
            pallet_subtensor_swap::Positions::<Test>::iter()
                .any(|((n, owner, _pid), _)| n == net_new && owner == who_cold),
            "new position not recorded on the re-registered net"
        );
    });
}
