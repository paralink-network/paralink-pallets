use crate as pallet_paralink_feeds;

use frame_support::{
    parameter_types,
    traits::{ConstU64, Everything},
    weights::{IdentityFee, Weight},
};
use frame_system as system;

use sp_core::{
    offchain::{testing, OffchainWorkerExt, TransactionPoolExt},
    sr25519::Signature,
    H256,
};

use sp_runtime::{
    testing::{Header, TestXt},
    traits::{BlakeTwo256, Extrinsic as ExtrinsicT, IdentifyAccount, IdentityLookup, Verify},
};
use xcm::latest::{prelude::*, Junction, OriginKind, SendXcm, Xcm};
use xcm_builder::{
    AllowUnpaidExecutionFrom, EnsureXcmOrigin, FixedWeightBounds, LocationInverter,
    SignedToAccountId32,
};
use xcm_executor::{
    traits::{InvertLocation, TransactAsset, WeightTrader},
    Assets, XcmExecutor,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        XcmPallet: pallet_xcm::{Pallet, Call, Event<T>, Origin, Config} = 31,
        CumulusXcm: cumulus_pallet_xcm::{Pallet, Event<T>, Origin} = 32,
        ParalinkFeedPallet: pallet_paralink_feeds::{Pallet, Call, Storage, Event<T>},
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const SS58Prefix: u8 = 42;
    pub const StringLimit: u32 = 20;
    pub const FeedStakingBalance: u64 = 10_000;

    pub static AdvertisedXcmVersion: xcm::prelude::XcmVersion = 2;
    pub const UnitWeightCost: Weight = 10;
    pub const MaxInstructions: u32 = 100;
    pub const RelayNetwork: NetworkId = NetworkId::Any;

    pub Ancestry: MultiLocation = Parachain(2001).into();
}

impl system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u64>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_balances::Config for Test {
    type Balance = u64;
    type DustRemoval = ();
    type Event = Event;
    type ExistentialDeposit = ConstU64<1>;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
}

pub struct DummyWeightTrader;
impl WeightTrader for DummyWeightTrader {
    fn new() -> Self {
        DummyWeightTrader
    }

    fn buy_weight(&mut self, _weight: Weight, _payment: Assets) -> Result<Assets, XcmError> {
        Ok(Assets::default())
    }
}
pub struct DummyAssetTransactor;
impl TransactAsset for DummyAssetTransactor {
    fn deposit_asset(_what: &MultiAsset, _who: &MultiLocation) -> XcmResult {
        Ok(())
    }

    fn withdraw_asset(_what: &MultiAsset, _who: &MultiLocation) -> Result<Assets, XcmError> {
        let asset: MultiAsset = (Parent, 100_000).into();
        Ok(asset.into())
    }
}
pub struct XcmConfig;
pub struct DoNothingRouter;
type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

pub type LocalOriginToLocation = SignedToAccountId32<Origin, AccountId, RelayNetwork>;
pub type Barrier = AllowUnpaidExecutionFrom<Everything>;

impl SendXcm for DoNothingRouter {
    fn send_xcm(_dest: impl Into<MultiLocation>, _msg: Xcm<()>) -> SendResult {
        Ok(())
    }
}

impl xcm_executor::Config for XcmConfig {
    type Call = Call;
    type XcmSender = DoNothingRouter;
    type AssetTransactor = DummyAssetTransactor;
    type OriginConverter = pallet_xcm::XcmPassthrough<Origin>;
    type IsReserve = ();
    type IsTeleporter = ();
    type LocationInverter = LocationInverter<Ancestry>;
    type Barrier = Barrier;
    type Weigher = FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
    type Trader = DummyWeightTrader;
    type ResponseHandler = ();
    type AssetTrap = XcmPallet;
    type AssetClaims = XcmPallet;
    type SubscriptionService = XcmPallet;
}

impl pallet_xcm::Config for Test {
    type Event = Event;
    type SendXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
    type XcmRouter = DoNothingRouter;
    type LocationInverter = LocationInverter<Ancestry>;
    type ExecuteXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
    type XcmExecuteFilter = Everything;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type XcmTeleportFilter = Everything;
    type Weigher = FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
    type XcmReserveTransferFilter = Everything;
    type Origin = Origin;
    type Call = Call;
    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
    type AdvertisedXcmVersion = AdvertisedXcmVersion;
}

impl cumulus_pallet_xcm::Config for Test {
    type Event = Event;
    type XcmExecutor = XcmExecutor<XcmConfig>;
}

impl pallet_paralink_feeds::Config for Test {
    type FeedId = u32;
    type Value = u128;
    type Event = Event;
    type Currency = Balances;
    type StringLimit = StringLimit;
    type FeedStakingBalance = FeedStakingBalance;
    type WeightInfo = ();

    type Call = Call;
    type Origin = Origin;
    type XcmSender = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    // inject test balances
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (0, 10_000), // root
            (1, 10_000), // alice
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();
    // set PalletAdmin for feeds
    pallet_paralink_feeds::GenesisConfig::<Test> {
        // make Ferdie the pallet admin
        pallet_admin: Some(6),
        feed_creators: vec![1],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    //t.into()
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
