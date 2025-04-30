use super::*;
use crate::AccountIdOf;
use alloc::collections::BTreeMap;
use frame_support::{
    pallet_prelude::{Blake2_128Concat, ValueQuery},
    storage_alias,
    traits::Get,
    weights::Weight,
};
pub use frame_system::pallet_prelude::BlockNumberFor;
use scale_info::prelude::string::String;
/// Module containing deprecated storage format for LoadedEmission
pub mod deprecated_coldkey_swap_scheduled_format {
    use super::*;

    #[storage_alias]
    pub(super) type ColdkeySwapScheduled<T: Config> =
        StorageMap<Pallet<T>, Blake2_128Concat, AccountIdOf<T>, (), ValueQuery>;
}

/// Migrate the ColdkeySwapScheduled map to the new storage format
pub fn migrate_coldkey_swap_scheduled<T: Config>() -> Weight {
    use deprecated_coldkey_swap_scheduled_format as old;

    let migration_name = b"migrate_coldkey_swap_scheduled".to_vec();
    let mut weight = T::DbWeight::get().reads(1);

    if HasMigrationRun::<T>::get(&migration_name) {
        log::info!(
            "Migration '{:?}' has already run. Skipping.",
            migration_name
        );
        return weight;
    }

    log::info!(
        "Running migration '{}'",
        String::from_utf8_lossy(&migration_name)
    );

    // ------------------------------
    // Step 1: Migrate ColdkeySwapScheduled map
    // ------------------------------
    let mut scheduled_map: BTreeMap<AccountIdOf<T>, (BlockNumberFor<T>, AccountIdOf<T>)> =
        BTreeMap::new();

    // for (block, scheduled_tasks) in old::ColdkeySwapScheduled::iter() {
    // for task in old::ColdkeySwapScheduled::<T>::iter() {

    //     //scheduled_map.insert(task.to, (block, new_coldkey));
    // }
    // }

    let curr_keys: Vec<AccountIdOf<T>> = old::ColdkeySwapScheduled::<T>::iter_keys().collect();

    // Remove any undecodable entries
    for coldkey in curr_keys {
        weight.saturating_accrue(T::DbWeight::get().reads(1));
        if old::ColdkeySwapScheduled::<T>::try_get(&coldkey).is_err() {
            weight.saturating_accrue(T::DbWeight::get().writes(1));
            old::ColdkeySwapScheduled::<T>::remove(&coldkey);
            log::warn!(
                "Was unable to decode old coldkey_swap_scheduled for coldkey {:?}",
                &coldkey
            );
        }
    }

    let default_value = DefaultColdkeySwapScheduled::<T>::get();
    ColdkeySwapScheduled::<T>::translate::<(), _>(|coldkey: AccountIdOf<T>, _: ()| {
        let (when, new_coldkey) = scheduled_map.get(&coldkey).unwrap_or(&default_value);

        Some((*when, new_coldkey.clone()))
    });

    // ------------------------------
    // Step 2: Mark Migration as Completed
    // ------------------------------

    HasMigrationRun::<T>::insert(&migration_name, true);
    weight = weight.saturating_add(T::DbWeight::get().writes(1));

    log::info!(
        "Migration '{:?}' completed successfully.",
        String::from_utf8_lossy(&migration_name)
    );

    weight
}
