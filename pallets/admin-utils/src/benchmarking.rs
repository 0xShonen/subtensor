//! Benchmarking setup
#![cfg(feature = "runtime-benchmarks")]
#![allow(clippy::arithmetic_side_effects)]

extern crate alloc;
use alloc::vec::Vec;

#[allow(unused)]
use crate::Pallet as AdminUtils;
use frame_benchmarking::v1::account;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pallet_subtensor::SubnetworkN;
use subtensor_runtime_common::NetUid;

use super::*;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn swap_authorities(a: Linear<0, 32>) {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);

        let mut value: BoundedVec<
            <T as pallet::Config>::AuthorityId,
            <T as pallet::Config>::MaxAuthorities,
        > = BoundedVec::new();

        for idx in 1..=a {
            let authority: <T as pallet::Config>::AuthorityId = account("Authority", idx, 0u32);
            let result = value.try_push(authority.clone());
            if result.is_err() {
                // Handle the error, perhaps by breaking the loop or logging an error message
            }
        }

        #[extrinsic_call]
        _(RawOrigin::Root, value);
    }

    #[benchmark]
    fn schedule_grandpa_change(a: Linear<0, 32>) {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        let next_authorities = (1..=a)
            .map(|idx| account("Authority", idx, 0u32))
            .collect::<Vec<(sp_consensus_grandpa::AuthorityId, u64)>>();
        let in_blocks = BlockNumberFor::<T>::from(42u32);

        #[extrinsic_call]
        _(RawOrigin::Root, next_authorities, in_blocks, None);
    }

    #[benchmark]
    fn sudo_set_default_take() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        #[extrinsic_call]
		_(RawOrigin::Root, 100u16/*default_take*/)/*sudo_set_default_take*/;
    }

    #[benchmark]
    fn sudo_set_serving_rate_limit() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 100u64/*serving_rate_limit*/)/*sudo_set_serving_rate_limit*/;
    }

    #[benchmark]
    fn sudo_set_max_difficulty() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 10000u64/*max_difficulty*/)/*sudo_set_max_difficulty*/;
    }

    #[benchmark]
    fn sudo_set_min_difficulty() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 1000u64/*min_difficulty*/)/*sudo_set_min_difficulty*/;
    }

    #[benchmark]
    fn sudo_set_weights_set_rate_limit() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 3u64/*rate_limit*/)/*sudo_set_weights_set_rate_limit*/;
    }

    #[benchmark]
    fn sudo_set_weights_version_key() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 1u64/*version_key*/)/*sudo_set_weights_version_key*/;
    }

    #[benchmark]
    fn sudo_set_bonds_moving_average() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 100u64/*bonds_moving_average*/)/*sudo_set_bonds_moving_average*/;
    }

    #[benchmark]
    fn sudo_set_bonds_penalty() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
        _(RawOrigin::Root, 1u16.into()/*netuid*/, 100u16/*bonds_penalty*/)/*sudo_set_bonds_penalty*/;
    }

    #[benchmark]
    fn sudo_set_max_allowed_validators() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 10u16/*max_allowed_validators*/)/*sudo_set_max_allowed_validators*/;
    }

    #[benchmark]
    fn sudo_set_difficulty() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 1200000u64/*difficulty*/)/*sudo_set_difficulty*/;
    }

    #[benchmark]
    fn sudo_set_adjustment_interval() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 12u16/*adjustment_interval*/)/*sudo_set_adjustment_interval*/;
    }

    #[benchmark]
    fn sudo_set_target_registrations_per_interval() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 300u16/*target_registrations*/)/*sudo_set_target_registrations_per_interval*/;
    }

    #[benchmark]
    fn sudo_set_activity_cutoff() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 361u16/*activity_cutoff*/)/*sudo_set_activity_cutoff*/;
    }

    #[benchmark]
    fn sudo_set_rho() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 300u16/*rho*/)/*sudo_set_rho*/;
    }

    #[benchmark]
    fn sudo_set_kappa() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*sudo_tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 3u16/*kappa*/)/*set_kappa*/;
    }

    #[benchmark]
    fn sudo_set_min_allowed_uids() {
        let netuid = NetUid::from(1);
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(netuid, 1u16 /*tempo*/);

        // Artificially set that some neurons are already registered
        SubnetworkN::<T>::set(netuid, 32);

        #[extrinsic_call]
		_(RawOrigin::Root, netuid, 16u16/*min_allowed_uids*/)/*sudo_set_min_allowed_uids*/;
    }

    #[benchmark]
    fn sudo_set_max_allowed_uids() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 4097u16/*max_allowed_uids*/)/*sudo_set_max_allowed_uids*/;
    }

    #[benchmark]
    fn sudo_set_min_allowed_weights() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 10u16/*max_allowed_uids*/)/*sudo_set_min_allowed_weights*/;
    }

    #[benchmark]
    fn sudo_set_immunity_period() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 100u16/*immunity_period*/)/*sudo_set_immunity_period*/;
    }

    #[benchmark]
    fn sudo_set_max_weight_limit() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 100u16/*max_weight_limit*/)/*sudo_set_max_weight_limit*/;
    }

    #[benchmark]
    fn sudo_set_max_registrations_per_block() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 100u16/*max_registrations*/)/*sudo_set_max_registrations_per_block*/;
    }

    #[benchmark]
    fn sudo_set_max_burn() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 2_000_000_000.into()/*max_burn*/)/*sudo_set_max_burn*/;
    }

    #[benchmark]
    fn sudo_set_min_burn() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 10.into()/*min_burn*/)/*sudo_set_min_burn*/;
    }

    #[benchmark]
    fn sudo_set_network_registration_allowed() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, true/*registration_allowed*/)/*sudo_set_network_registration_allowed*/;
    }

    /*
        benchmark_sudo_set_tempo {
        let netuid = NetUid::from(1);
        let tempo_default: u16 = 1; <------- unused?
        let tempo: u16 = 15;
        let modality: u16 = 0;

        pallet_subtensor::Pallet::<T>::init_new_network(netuid, tempo);

    }: sudo_set_tempo(RawOrigin::<AccountIdOf<T>>::Root, netuid, tempo)
    */
    #[benchmark]
    fn sudo_set_tempo() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 1u16/*tempo*/)/*sudo_set_tempo*/;
    }

    #[benchmark]
    fn sudo_set_commit_reveal_weights_interval() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*sudo_tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 3u64/*interval*/)/*sudo_set_commit_reveal_weights_interval()*/;
    }

    #[benchmark]
    fn sudo_set_commit_reveal_weights_enabled() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*sudo_tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, true/*enabled*/)/*set_commit_reveal_weights_enabled*/;
    }

    #[benchmark]
    fn sudo_set_commit_reveal_version() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*sudo_tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 5u16/*version*/)/*sudo_set_commit_reveal_version()*/;
    }

    #[benchmark]
    fn sudo_set_admin_freeze_window() {
        #[extrinsic_call]
		_(RawOrigin::Root, 5u16/*window*/)/*sudo_set_admin_freeze_window*/;
    }

    #[benchmark]
    fn sudo_set_owner_hparam_rate_limit() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        #[extrinsic_call]
		_(RawOrigin::Root, 2u16/*epochs*/)/*sudo_set_owner_hparam_rate_limit*/;
    }

    #[benchmark]
    fn sudo_set_owner_immune_neuron_limit() {
        // disable admin freeze window
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*sudo_tempo*/
        );

        #[extrinsic_call]
        _(RawOrigin::Root, 1u16.into()/*netuid*/, 5u16/*immune_neurons*/)/*sudo_set_owner_immune_neuron_limit()*/;
    }

    #[benchmark]
    fn sudo_trim_to_max_allowed_uids() {
        pallet_subtensor::Pallet::<T>::set_admin_freeze_window(0);
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*sudo_tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 256u16/*max_n*/)/*sudo_trim_to_max_allowed_uids()*/;
    }

    //impl_benchmark_test_suite!(AdminUtils, crate::mock::new_test_ext(), crate::mock::Test);
}
