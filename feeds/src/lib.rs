#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;
use sp_std::prelude::*;

//#[cfg(test)]
//mod mock;

//#[cfg(test)]
//mod tests;

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
    use frame_system::Config as SystemConfig;
    use sp_arithmetic::traits::BaseArithmetic;
    use sp_runtime::traits::{CheckedAdd, One, Zero};

    // XCM
    use cumulus_pallet_xcm::{ensure_sibling_para, Origin as CumulusOrigin};
    use cumulus_primitives_core::ParaId;
    use xcm::latest::prelude::*;

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

        /// XCM
        /// The overarching call type; we assume sibling chains use the same type.
        type Call: From<Call<Self>> + Encode;

        type Origin: From<<Self as SystemConfig>::Origin>
            + Into<Result<CumulusOrigin, <Self as Config>::Origin>>;

        type XcmSender: SendXcm;

        /// This is the trusted account to submit XCM messages
        /// Depends on parachain ID, can be calculated here:
        /// https://substrate.stackexchange.com/questions/1200/how-to-calculate-sovereignaccount-for-parachain/1210
        type ParalinkSovereignAccount: Get<<Self as frame_system::Config>::AccountId>;
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

    /// Parachains interested in price feeds (used on the paralink side)
    #[pallet::storage]
    #[pallet::getter(fn registered_parachains)]
    pub(super) type RegisteredParachains<T: Config> =
        StorageMap<_, Twox64Concat, ParaId, Vec<T::FeedId>, OptionQuery>;

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
        FeedCreated(
            T::FeedId,
            FeedConfig<T::AccountId, BoundedVec<u8, T::StringLimit>>,
        ),
        /// A feed [\feed_id\] owner was updated [\new_owner\]
        OwnerUpdated(T::FeedId, T::AccountId),
        /// A feed [\feed_id\] value [\value\] was updated
        FeedValueUpdated(T::FeedId, T::Value),
        XcmReceivedNewFeedSubscription(
            ParaId,
            T::FeedId,
            FeedConfig<T::AccountId, BoundedVec<u8, T::StringLimit>>,
            Round<T::BlockNumber, T::Value>,
        ),
        // An XCM feed was registered on the Paralink chain
        XcmFeedRegistered(
            ParaId,
            T::FeedId,
            FeedConfig<T::AccountId, BoundedVec<u8, T::StringLimit>>,
        ),
        /// The XCM feed was registered on Paralink chain
        XcmFeedNotRegistered(
            SendError,
            ParaId,
            T::FeedId,
            FeedConfig<T::AccountId, BoundedVec<u8, T::StringLimit>>,
        ),
        XcmSendLatestValue(
            ParaId,
            Vec<(T::FeedId, RoundId, Round<T::BlockNumber, T::Value>)>,
        ),
        XcmErrorSendingLatestValue(
            SendError,
            ParaId,
            Vec<(T::FeedId, RoundId, Round<T::BlockNumber, T::Value>)>,
        ),
        // A feed value was received from XCM
        XcmReceiveLatestValue(ParaId, T::FeedId, RoundId, Round<T::BlockNumber, T::Value>),
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
        /// Register ParaId was not found
        ParaIdNotFound,
        /// The sending origin was not from the ParalinkSovereignAccount
        NotParalinkSovereignAccount,
    }

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

            let feed_config = FeedConfig {
                description,
                ipfs_hash,
                decimals,
                owner: owner.clone(),
                first_valid_round: None,
                latest_round: Zero::zero(),
            };
            Feeds::<T>::insert(id, feed_config.clone());

            let updated_at = frame_system::Pallet::<T>::block_number();
            Rounds::<T>::insert(
                id,
                RoundId::zero(),
                Round {
                    answer: Some(Zero::zero()),
                    updated_at: Some(updated_at),
                },
            );

            Self::deposit_event(Event::FeedCreated(id, feed_config));
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

        // XCM
        #[pallet::weight(T::WeightInfo::register_feed_to_paralink())]
        pub fn register_feed_to_paralink(
            origin: OriginFor<T>,
            para_id: ParaId,
            feed_id: T::FeedId,
        ) -> DispatchResult {
            let owner = ensure_signed(origin)?;
            ensure!(
                FeedCreators::<T>::contains_key(&owner),
                Error::<T>::NotFeedCreator
            );
            let feed_config = Feeds::<T>::get(feed_id).ok_or(Error::<T>::FeedNotFound)?;
            let round = Rounds::<T>::get(feed_id, feed_config.latest_round).unwrap_or_else(|| {
                debug_assert!(false, "The latest round should be available.");
                Round::default()
            });

            log::info!(
                "***** Paralink XCM register_feed_to_paralink to para_id: {:?},  feed: {:?}",
                para_id,
                feed_id,
            );
            match T::XcmSender::send_xcm(
                (1, Junction::Parachain(para_id.into())),
                Xcm(vec![Transact {
                    origin_type: OriginKind::Native,
                    require_weight_at_most: 2_000_000_000,
                    call: <T as Config>::Call::from(Call::<T>::receive_new_feed {
                        feed_id: feed_id.clone(),
                        feed_config: feed_config.clone(),
                        latest_round: round,
                    })
                    .encode()
                    .into(),
                }]),
            ) {
                Ok(()) => {
                    if RegisteredParachains::<T>::get(para_id).is_none() {
                        RegisteredParachains::<T>::insert(para_id, Vec::<T::FeedId>::new());
                    }

                    let mut vec = RegisteredParachains::<T>::get(para_id)
                        .ok_or(Error::<T>::ParaIdNotFound)?;
                    vec.push(feed_id);
                    RegisteredParachains::<T>::insert(para_id, vec);

                    Self::deposit_event(Event::XcmFeedRegistered(para_id, feed_id, feed_config));
                }
                Err(e) => Self::deposit_event(Event::XcmFeedNotRegistered(
                    e,
                    para_id,
                    feed_id,
                    feed_config,
                )),
            }

            log::info!("***** Paralink XCM send_latest_data_through_xcm exited");

            Ok(())
        }

        #[pallet::weight(T::WeightInfo::receive_new_feed())]
        pub fn receive_new_feed(
            origin: OriginFor<T>,
            feed_id: T::FeedId,
            feed_config: FeedConfig<T::AccountId, BoundedVec<u8, T::StringLimit>>,
            latest_round: Round<T::BlockNumber, T::Value>,
        ) -> DispatchResult {
            log::info!("***** Paralink XCM receive_new_feed called");
            let para_id = ensure_sibling_para(<T as Config>::Origin::from(origin.clone()))?;
            log::info!("***** Paralink XCM para called, para = {:?}", para_id);
            let signer = ensure_signed(origin.clone())?;

            log::info!("***** Paralink XCM para called, para = {:?}", para_id);

            log::info!(
                    "***** BEFORE Paralink XCM Received latest_feed_config = {:?}, round = {:?}, signer = {:?}, sovereign = {:?}",
                    feed_config.clone(),
                    latest_round.clone(),
                    signer,
                    T::ParalinkSovereignAccount::get()
            );

            ensure!(
                signer == T::ParalinkSovereignAccount::get(),
                Error::<T>::NotParalinkSovereignAccount
            );

            log::info!(
                "***** Paralink XCM Received latest_feed_config = {:?}, round = {:?}",
                feed_config.clone(),
                latest_round.clone()
            );

            Feeds::<T>::insert(feed_id, feed_config.clone());
            Rounds::<T>::insert(feed_id, feed_config.latest_round, latest_round.clone());

            // Register the feed, so we can retrieve it on the consumer chain
            if RegisteredParachains::<T>::get(para_id).is_none() {
                RegisteredParachains::<T>::insert(para_id, Vec::<T::FeedId>::new());
            }

            let mut vec =
                RegisteredParachains::<T>::get(para_id).ok_or(Error::<T>::ParaIdNotFound)?;
            vec.push(feed_id);
            RegisteredParachains::<T>::insert(para_id, vec);

            Self::deposit_event(Event::XcmReceivedNewFeedSubscription(
                para_id,
                feed_id,
                feed_config,
                latest_round,
            ));
            Ok(())
        }

        #[pallet::weight(T::WeightInfo::receive_latest_data())]
        pub fn receive_latest_data(
            origin: OriginFor<T>,
            data: Vec<(T::FeedId, RoundId, Round<T::BlockNumber, T::Value>)>,
        ) -> DispatchResult {
            log::info!("***** Paralink XCM receive_latest_data called");
            let para_id = ensure_sibling_para(<T as Config>::Origin::from(origin.clone()))?;
            let signer = ensure_signed(origin.clone())?;

            ensure!(
                signer == T::ParalinkSovereignAccount::get(),
                Error::<T>::NotParalinkSovereignAccount
            );

            log::info!(
                "***** Paralink XCM Received latest_data = {:?}",
                data.clone()
            );

            for (feed_id, round_id, round) in data.iter() {
                let feed = Feeds::<T>::get(feed_id.clone());

                if let Some(mut feed_config) = feed {
                    feed_config.latest_round = round_id.clone();

                    if feed_config.first_valid_round.is_none() {
                        feed_config.first_valid_round = Some(round_id.clone());
                    }

                    Feeds::<T>::insert(feed_id, feed_config.clone());
                    Rounds::<T>::insert(feed_id, feed_config.latest_round, round);

                    Self::deposit_event(Event::XcmReceiveLatestValue(
                        para_id,
                        feed_id.clone(),
                        round_id.clone(),
                        round.clone(),
                    ));
                } else {
                    log::info!(
                        "***** Paralink XCM receive_latest_data no feed found = {:?}",
                        data.clone()
                    );
                }
            }
            Ok(())
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

        fn send_latest_data_through_xcm(
            para_id: ParaId,
            data: Vec<(T::FeedId, RoundId, Round<T::BlockNumber, T::Value>)>,
        ) {
            log::info!(
                "***** Paralink XCM send_latest_data_through_xcm called para_id: {:?},  data {:?}",
                para_id,
                data
            );
            match T::XcmSender::send_xcm(
                (1, Junction::Parachain(para_id.into())),
                Xcm(vec![Transact {
                    origin_type: OriginKind::Native,
                    require_weight_at_most: 2_000_000_000,
                    call: <T as Config>::Call::from(Call::<T>::receive_latest_data {
                        data: data.clone(),
                    })
                    .encode()
                    .into(),
                }]),
            ) {
                Ok(()) => Self::deposit_event(Event::XcmSendLatestValue(para_id, data)),
                Err(e) => Self::deposit_event(Event::XcmErrorSendingLatestValue(e, para_id, data)),
            }

            log::info!("***** Paralink XCM send_latest_data_through_xcm exited");
        }
    }

    // XCM
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_n: T::BlockNumber) {
            for (para_id, feeds) in RegisteredParachains::<T>::iter() {
                let mut data: Vec<(T::FeedId, RoundId, Round<T::BlockNumber, T::Value>)> =
                    Vec::new();
                for feed_id in feeds.iter() {
                    let feed_config = Feeds::<T>::get(feed_id.clone());
                    if feed_config.is_none() {
                        log::error!(
                            "***** Paralink XCM on_finalize - no feed config found for para_id/feed_id: {:?}, {:?}",
                            para_id,
                            feed_id
                        );
                        continue;
                    }
                    if T::Currency::free_balance(&feed_config.clone().unwrap().owner)
                        < T::FeedStakingBalance::get()
                    {
                        log::error!(
                            "***** Paralink XCM on_finalize - not enough staking balance found for para_id/feed_id: {:?}, {:?}, available balance: {:?},/{:?}",
                            para_id,
                            feed_id,
                            T::Currency::free_balance(&feed_config.clone().unwrap().owner),
                            T::FeedStakingBalance::get()
                        );
                        continue;
                    }

                    let feed_round = Self::latest_value(feed_id.clone());
                    match feed_round {
                        Err(e) => {
                            log::error!(
                            "***** Paralink XCM on_finalize - no feed round found for para_id/feed_id: {:?}, {:?}, error: {:?}",
                            para_id,
                            feed_id,
                            e
                        );
                            continue;
                        }
                        Ok(round) => {
                            if let Some(feed) = feed_config {
                                data.push((feed_id.clone(), feed.latest_round, round));
                            }
                        }
                    }
                }
                if !data.is_empty() {
                    Self::send_latest_data_through_xcm(para_id, data);
                }
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
        fn receive_latest_data() -> Weight;
        fn receive_new_feed() -> Weight;
        fn register_feed_to_paralink() -> Weight;
    }
}
