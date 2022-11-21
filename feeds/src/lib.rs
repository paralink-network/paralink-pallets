#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        dispatch::{DispatchResultWithPostInfo, HasCompact},
        pallet_prelude::{OptionQuery, *},
        traits::{Currency, ReservableCurrency},
    };

    use frame_system::pallet_prelude::*;

    use sp_arithmetic::traits::BaseArithmetic;

    use sp_runtime::traits::{CheckedAdd, One, Zero};

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Type for feed indexing.
        type FeedId: Member + Parameter + Default + Copy + HasCompact + BaseArithmetic;

        /// Oracle feed values.
        type Value: Member
            + Parameter
            + Default
            + Copy
            + HasCompact
            + PartialEq
            + BaseArithmetic
            + MaybeSerializeDeserialize;
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// Maximum allowed string length.
        type StringLimit: Get<u32>;

        /// Given account has to have at least this amount of PARA to be able to create
        /// and send feed through XCM
        type FeedStakingBalance: Get<BalanceOf<Self>>;

        /// Information on runtime weights.
        type WeightInfo: WeightInfo;
    }

    pub type RoundId = u64;
    pub type IpfsHashLimit = ConstU32<60>;

    #[derive(Clone, Encode, Decode, Default, Eq, PartialEq, TypeInfo, RuntimeDebug)]
    pub struct FeedConfig<AccountId: Parameter, BoundedStringVec> {
        /// Description
        pub description: BoundedStringVec,
        /// IPFS pointer to the PQL query, the size can vary
        pub ipfs_hash: BoundedVec<u8, IpfsHashLimit>,
        /// Number of decimals
        pub decimals: u8,
        /// Id of the first valid round
        pub first_valid_round: Option<RoundId>,
        /// The id of the latest round
        pub latest_round: RoundId,
        /// Owner of this feed
        pub owner: AccountId,
    }

    #[derive(Clone, Encode, Decode, Default, Eq, PartialEq, TypeInfo, RuntimeDebug)]
    pub struct Round<BlockNumber, Value> {
        pub answer: Option<Value>,
        pub updated_at: Option<BlockNumber>,
    }

    #[pallet::storage]
    #[pallet::getter(fn pallet_admin)]
    /// Account responsible for managing feed creators
    pub type PalletAdmin<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    #[pallet::storage]
    /// Accounts allowed to create feeds.
    pub type FeedCreators<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (), OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn feed_config)]
    pub type Feeds<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::FeedId,
        FeedConfig<T::AccountId, BoundedVec<u8, T::StringLimit>>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn feed_counter)]
    /// A running counter used internally to determine the next feed id.
    pub type FeedCounter<T: Config> = StorageValue<_, T::FeedId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn round)]
    /// User-facing round data.
    pub type Rounds<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        T::FeedId,
        Twox64Concat,
        RoundId,
        Round<T::BlockNumber, T::Value>,
        OptionQuery,
    >;

    // Pallets use events to inform users when important changes are made.
    // https://docs.substrate.io/v3/runtime/events-and-errors
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The pallet admin change was executed. \[new_admin\]
        PalletAdminUpdated(T::AccountId),
        /// A feed creator was added. \[new_creator\]
        FeedCreatorAdded(T::AccountId),
        /// A feed creator was removed. \[creator\]
        FeedCreatorRemoved(T::AccountId),
        /// A feed was created with [\feed_id\] and [\creator\]
        FeedCreated(T::FeedId, T::AccountId),
        /// A feed [\feed_id\] owner was updated [\new_owner\]
        OwnerUpdated(T::FeedId, T::AccountId),
        /// A feed [\feed_id\] value [\value\] was updated
        FeedValueUpdated(T::FeedId, T::Value),
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        /// Only the pallet admin account can call this extrinsic.
        NotPalletAdmin,
        /// Only the verified feed creator can create feeds
        NotFeedCreator,
        /// Only the owner of feed can modify feed
        NotFeedOwner,
        /// Description was too long
        DescriptionTooLong,
        /// IpfsHash was too long
        IpfsHashTooLong,
        /// Math overflow
        Overflow,
        /// No such feed was found
        FeedNotFound,
        /// There is no value, since it was not submitted yet
        NoValue,
        /// The account does not hold enough balance to create/submit to feed
        NotEnoughBalance,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(T::WeightInfo::create_feed())]
        pub fn create_feed(
            origin: OriginFor<T>,
            ipfs_hash: Vec<u8>,
            description: Vec<u8>,
            decimals: u8,
        ) -> DispatchResultWithPostInfo {
            let owner = ensure_signed(origin)?;

            ensure!(
                FeedCreators::<T>::contains_key(&owner),
                Error::<T>::NotFeedCreator
            );

            ensure!(
                T::Currency::free_balance(&owner) >= T::FeedStakingBalance::get(),
                Error::<T>::NotEnoughBalance
            );

            let description: BoundedVec<u8, T::StringLimit> = description
                .try_into()
                .map_err(|_| Error::<T>::DescriptionTooLong)?;

            let ipfs_hash: BoundedVec<u8, IpfsHashLimit> = ipfs_hash
                .try_into()
                .map_err(|_| Error::<T>::IpfsHashTooLong)?;

            let id: T::FeedId = FeedCounter::<T>::get();
            let new_id = id.checked_add(&One::one()).ok_or(Error::<T>::Overflow)?;
            FeedCounter::<T>::put(new_id);

            Feeds::<T>::insert(
                id,
                FeedConfig {
                    description,
                    ipfs_hash,
                    decimals,
                    owner: owner.clone(),
                    first_valid_round: None,
                    latest_round: Zero::zero(),
                },
            );

            let updated_at = frame_system::Pallet::<T>::block_number();
            Rounds::<T>::insert(
                id,
                RoundId::zero(),
                Round {
                    answer: Some(Zero::zero()),
                    updated_at: Some(updated_at),
                },
            );

            Self::deposit_event(Event::FeedCreated(id, owner));
            Ok(().into())
        }

        /// Submit a new value to the given feed and round.
        ///
        /// Limited to the owner of a feed.
        #[pallet::weight(T::WeightInfo::submit())]
        pub fn submit(
            origin: OriginFor<T>,
            #[pallet::compact] feed_id: T::FeedId,
            #[pallet::compact] value: T::Value,
        ) -> DispatchResultWithPostInfo {
            let owner = ensure_signed(origin)?;

            let mut feed = Self::feed_config(feed_id).ok_or(Error::<T>::FeedNotFound)?;
            ensure!(feed.owner == owner, Error::<T>::NotFeedOwner);

            ensure!(
                T::Currency::free_balance(&owner) >= T::FeedStakingBalance::get(),
                Error::<T>::NotEnoughBalance
            );

            let round_id = feed
                .latest_round
                .checked_add(One::one())
                .ok_or(Error::<T>::Overflow)?;

            if feed.first_valid_round.is_none() {
                feed.first_valid_round = Some(round_id);
            }
            feed.latest_round = round_id;

            let updated_at = frame_system::Pallet::<T>::block_number();

            Feeds::<T>::insert(feed_id, feed);
            Rounds::<T>::insert(
                feed_id,
                round_id,
                Round {
                    answer: Some(value),
                    updated_at: Some(updated_at),
                },
            );

            Self::deposit_event(Event::FeedValueUpdated(feed_id, value));

            Ok(().into())
        }

        /// Sets the pallet admin account.
        ///
        /// The dispatch origin for this call must be _Root_.
        ///
        /// Unlike `transfer_pallet_admin`, the `new_pallet_admin` is not
        /// required to accept the transfer, instead the `PalletAdmin` is
        /// forcibly set and the eventual pending transfer is aborted.
        #[pallet::weight(T::DbWeight::get().writes(2))]
        pub fn set_pallet_admin(
            origin: OriginFor<T>,
            new_pallet_admin: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            PalletAdmin::<T>::put(new_pallet_admin.clone());
            Self::deposit_event(Event::PalletAdminUpdated(new_pallet_admin));

            Ok(())
        }

        /// Allow the given account to create oracle feeds.
        /// Limited to the pallet admin account.
        #[pallet::weight(T::WeightInfo::add_feed_creator())]
        pub fn add_feed_creator(
            origin: OriginFor<T>,
            new_creator: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let admin = ensure_signed(origin)?;
            ensure!(
                Self::pallet_admin() == Some(admin),
                Error::<T>::NotPalletAdmin
            );

            FeedCreators::<T>::insert(&new_creator, ());

            Self::deposit_event(Event::FeedCreatorAdded(new_creator));

            Ok(().into())
        }

        /// Disallow the given account to create oracle feeds.
        /// Limited to the pallet admin account.
        #[pallet::weight(T::WeightInfo::remove_feed_creator())]
        pub fn remove_feed_creator(
            origin: OriginFor<T>,
            creator: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let admin = ensure_signed(origin)?;
            ensure!(
                Self::pallet_admin() == Some(admin),
                Error::<T>::NotPalletAdmin
            );

            FeedCreators::<T>::remove(&creator);

            Self::deposit_event(Event::FeedCreatorRemoved(creator));

            Ok(().into())
        }

        /// Initiate the transfer of the feed to `new_owner`.
        /// Limited to the current owner of the feed.
        ///
        /// This is a noop if the requested `new_owner` is the sender itself
        /// and the sender is already the owner.
        #[pallet::weight(T::WeightInfo::transfer_ownership())]
        pub fn transfer_ownership(
            origin: OriginFor<T>,
            feed_id: T::FeedId,
            new_owner: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let old_owner = ensure_signed(origin)?;
            let mut feed = Self::feed_config(feed_id).ok_or(Error::<T>::FeedNotFound)?;
            ensure!(feed.owner == old_owner, Error::<T>::NotFeedOwner);
            if old_owner == new_owner {
                // nothing to transfer
                return Ok(().into());
            }

            feed.owner = new_owner.clone();
            Feeds::<T>::insert(feed_id, feed);

            Self::deposit_event(Event::OwnerUpdated(feed_id, new_owner));

            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Gets the latest data for the feed_id in a single call
        pub fn latest_value(
            feed_id: T::FeedId,
        ) -> Result<Round<T::BlockNumber, T::Value>, Error<T>> {
            let feed = Self::feed_config(feed_id).ok_or(Error::<T>::FeedNotFound)?;

            if feed.first_valid_round.is_none() {
                Err(Error::NoValue)
            } else {
                let round = Rounds::<T>::get(feed_id, feed.latest_round).unwrap_or_else(|| {
                    debug_assert!(false, "The latest round should be available.");
                    Round::default()
                });
                Ok(round)
            }
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub pallet_admin: Option<T::AccountId>,
        pub feed_creators: Vec<T::AccountId>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                pallet_admin: Default::default(),
                feed_creators: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            if let Some(ref admin) = self.pallet_admin {
                PalletAdmin::<T>::put(admin);
            }
            for creator in &self.feed_creators {
                FeedCreators::<T>::insert(creator, ());
            }
        }
    }

    #[cfg(feature = "std")]
    impl<T: Config> GenesisConfig<T> {
        /// Direct implementation of `GenesisBuild::build_storage`.
        ///
        /// Kept in order not to break dependency.
        pub fn build_storage(&self) -> Result<frame_support::sp_runtime::Storage, String> {
            <Self as GenesisBuild<T>>::build_storage(self)
        }

        /// Direct implementation of `GenesisBuild::assimilate_storage`.
        ///
        /// Kept in order not to break dependency.
        pub fn assimilate_storage(
            &self,
            storage: &mut frame_support::sp_runtime::Storage,
        ) -> Result<(), String> {
            <Self as GenesisBuild<T>>::assimilate_storage(self, storage)
        }
    }

    /// Trait for the paralink pallet extrinsic weights
    /// (generated automatically with benchmark in weights.rs)
    pub trait WeightInfo {
        fn create_feed() -> Weight;
        fn submit() -> Weight;
        fn transfer_ownership() -> Weight;
        fn transfer_pallet_admin() -> Weight;
        fn add_feed_creator() -> Weight;
        fn remove_feed_creator() -> Weight;
    }
}
