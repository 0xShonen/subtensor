use frame_support::{assert_noop, assert_ok};
use frame_system::Config;
use sp_core::U256;
use subtensor_runtime_common::NetUid;

use super::mock::*;
use crate::utils::rate_limiting::TransactionType;
use crate::{RateLimitKey, SubnetOwner, SubtokenEnabled};

#[test]
fn ensure_subnet_owner_returns_who_and_checks_ownership() {
    new_test_ext(1).execute_with(|| {
        let netuid = NetUid::from(1);
        add_network(netuid, 10, 0);

        let owner: U256 = U256::from(42);
        SubnetOwner::<Test>::insert(netuid, owner);

        // Non-owner signed should fail
        assert!(
            crate::Pallet::<Test>::ensure_subnet_owner(
                <<Test as Config>::RuntimeOrigin>::signed(U256::from(7)),
                netuid
            )
            .is_err()
        );

        // Owner signed returns who
        let who = crate::Pallet::<Test>::ensure_subnet_owner(
            <<Test as Config>::RuntimeOrigin>::signed(owner),
            netuid,
        )
        .expect("owner must pass");
        assert_eq!(who, owner);
    });
}

#[test]
fn ensure_subnet_owner_or_root_distinguishes_root_and_owner() {
    new_test_ext(1).execute_with(|| {
        let netuid = NetUid::from(2);
        add_network(netuid, 10, 0);
        let owner: U256 = U256::from(9);
        SubnetOwner::<Test>::insert(netuid, owner);

        // Root path returns None
        let root = crate::Pallet::<Test>::ensure_subnet_owner_or_root(
            <<Test as Config>::RuntimeOrigin>::root(),
            netuid,
        )
        .expect("root allowed");
        assert!(root.is_none());

        // Owner path returns Some(owner)
        let maybe_owner = crate::Pallet::<Test>::ensure_subnet_owner_or_root(
            <<Test as Config>::RuntimeOrigin>::signed(owner),
            netuid,
        )
        .expect("owner allowed");
        assert_eq!(maybe_owner, Some(owner));
    });
}

#[test]
fn ensure_root_with_rate_limit_blocks_in_freeze_window() {
    new_test_ext(1).execute_with(|| {
        let netuid = NetUid::from(1);
        let tempo: u16 = 10;
        add_network(netuid, 10, 0);

        // Set freeze window to 3
        let freeze_window: u16 = 3;
        crate::Pallet::<Test>::set_admin_freeze_window(freeze_window);

        run_to_block((tempo - freeze_window + 1).into());

        // Root is blocked in freeze window
        assert!(
            crate::Pallet::<Test>::ensure_root_with_rate_limit(
                <<Test as Config>::RuntimeOrigin>::root(),
                netuid
            )
            .is_err()
        );
    });
}

#[test]
fn ensure_owner_or_root_with_limits_checks_rl_and_freeze() {
    new_test_ext(1).execute_with(|| {
        let netuid = NetUid::from(1);
        let tempo = 10;
        add_network(netuid, 10, 0);
        SubtokenEnabled::<Test>::insert(netuid, true);
        let owner: U256 = U256::from(5);
        SubnetOwner::<Test>::insert(netuid, owner);
        // Set freeze window to 0 initially to avoid blocking when tempo is small
        let freeze_window = 3;
        crate::Pallet::<Test>::set_admin_freeze_window(0);

        // Set tempo to 1 so owner hyperparam RL = 2 blocks
        crate::Pallet::<Test>::set_tempo(netuid, 1);

        // Outside freeze window initially; should pass and return Some(owner)
        let res = crate::Pallet::<Test>::ensure_sn_owner_or_root_with_limits(
            <<Test as Config>::RuntimeOrigin>::signed(owner),
            netuid,
            &[TransactionType::OwnerSetKappa],
        )
        .expect("should pass");
        assert_eq!(res, Some(owner));

        // Simulate previous update at current block -> next call should fail due to rate limit
        let now = crate::Pallet::<Test>::get_current_block_as_u64();
        crate::Pallet::<Test>::set_last_transaction_block_on_subnet(
            &owner,
            netuid,
            &TransactionType::OwnerSetKappa,
            now,
        );
        assert_noop!(
            crate::Pallet::<Test>::ensure_sn_owner_or_root_with_limits(
                <<Test as Config>::RuntimeOrigin>::signed(owner),
                netuid,
                &[TransactionType::OwnerSetKappa],
            ),
            crate::Error::<Test>::TxRateLimitExceeded
        );

        // Advance beyond RL and ensure passes again
        run_to_block(now + 3);
        assert_ok!(crate::Pallet::<Test>::ensure_sn_owner_or_root_with_limits(
            <<Test as Config>::RuntimeOrigin>::signed(owner),
            netuid,
            &[TransactionType::OwnerSetKappa]
        ));

        // Now advance into the freeze window; ensure blocks
        // (using loop for clarity, because epoch calculation function uses netuid)
        // Restore tempo and configure freeze window for this part
        crate::Pallet::<Test>::set_tempo(netuid, tempo);
        crate::Pallet::<Test>::set_admin_freeze_window(freeze_window);
        let freeze_window = freeze_window as u64;
        loop {
            let cur = crate::Pallet::<Test>::get_current_block_as_u64();
            let rem = crate::Pallet::<Test>::blocks_until_next_epoch(netuid, tempo, cur);
            if rem < freeze_window {
                break;
            }
            run_to_block(cur + 1);
        }
        assert_noop!(
            crate::Pallet::<Test>::ensure_sn_owner_or_root_with_limits(
                <<Test as Config>::RuntimeOrigin>::signed(owner),
                netuid,
                &[TransactionType::OwnerSetKappa],
            ),
            crate::Error::<Test>::AdminActionProhibitedDuringWeightsWindow
        );
    });
}
