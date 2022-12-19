use super::*;
use crate::{mock::*, Error, Event};
use frame_support::{assert_noop, assert_ok, traits::Currency};
use sp_runtime::DispatchError;

const IPFS_HASH: &str = "Qme3LWCbRwtEp51ENPdVupj9rDEgMqpnfSxLec7tnGjTk7";

fn events() -> Vec<mock::Event> {
    let evt = System::events()
        .into_iter()
        .map(|evt| evt.event)
        .collect::<Vec<_>>();
    System::reset_events();

    evt
}

#[test]
fn test_ext_builder() {
    new_test_ext().execute_with(|| {
        // check that alice has injected funds
        let alice = 1 as u64;
        Balances::make_free_balance_be(&alice, 10_000);

        assert_eq!(Balances::free_balance(&alice), 10_000);
    });
}

#[test]
fn admin_transfer_should_work() {
    new_test_ext().execute_with(|| {
        let current_admin_value = ParalinkFeedPallet::pallet_admin();
        assert_eq!(current_admin_value, Some(6));

        if let Some(current_admin) = current_admin_value {
            let new_admin = 2;

            assert_noop!(
                ParalinkFeedPallet::set_pallet_admin(Origin::signed(current_admin), new_admin),
                DispatchError::BadOrigin
            );
            assert_ok!(ParalinkFeedPallet::set_pallet_admin(
                Origin::root(),
                new_admin
            ));
            assert_eq!(ParalinkFeedPallet::pallet_admin(), Some(new_admin));
        }
    });
}

#[test]
fn create_feed_works() {
    new_test_ext().execute_with(|| {
        let alice = 1;
        assert_eq!(ParalinkFeedPallet::feed_counter(), 0);

        let description: &str = "BTC/USD";
        let decimals = 8;
        assert_ok!(ParalinkFeedPallet::create_feed(
            Origin::signed(alice),
            IPFS_HASH.into(),
            description.into(),
            decimals
        ));

        System::assert_last_event(mock::Event::ParalinkFeedPallet(Event::FeedCreated(
            0, alice,
        )));

        // New feed on ID = 0 should match given data
        assert_eq!(
            ParalinkFeedPallet::feed_config(0),
            Some(FeedConfig {
                description: description.as_bytes().to_vec().try_into().unwrap(),
                ipfs_hash: IPFS_HASH.as_bytes().to_vec().try_into().unwrap(),
                decimals,
                first_valid_round: None,
                latest_round: 0,
                owner: alice
            })
        );

        // A Round object should also be inserted
        assert_eq!(
            ParalinkFeedPallet::round(0, 0),
            Some(Round {
                answer: Some(0),
                updated_at: Some(1)
            })
        );

        // Make sure the IPFS hash can be converted back to str
        assert_eq!(
            std::str::from_utf8(
                ParalinkFeedPallet::feed_config(0)
                    .unwrap()
                    .ipfs_hash
                    .as_slice()
            )
            .unwrap(),
            IPFS_HASH
        );
    });
}

#[test]
fn create_feed_not_feed_creator_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            ParalinkFeedPallet::create_feed(
                Origin::signed(2),
                IPFS_HASH.into(),
                "BTC/USD".into(),
                8
            ),
            Error::<Test>::NotFeedCreator
        );
    });
}

#[test]
fn create_feed_input_failures() {
    new_test_ext().execute_with(|| {
        let alice = 1;
        assert_noop!(
            ParalinkFeedPallet::create_feed(
                Origin::signed(alice),
                IPFS_HASH.into(),
                "a string longer than 15 chars should cause error".into(),
                8
            ),
            Error::<Test>::DescriptionTooLong
        );

        assert_noop!(
            ParalinkFeedPallet::create_feed(
                Origin::signed(alice),
                "an IPFS_HASH string longer than 60 characters will not work and cause an error"
                    .into(),
                "BTC/USD".into(),
                8
            ),
            Error::<Test>::IpfsHashTooLong
        );

        Balances::make_free_balance_be(&alice, 9999);
        assert_noop!(
            ParalinkFeedPallet::create_feed(
                Origin::signed(alice),
                IPFS_HASH.into(),
                "BTC/USD".into(),
                8
            ),
            Error::<Test>::NotEnoughBalance
        );
    });
}

#[test]
fn add_feed_creator_fails_without_pallet_admin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            ParalinkFeedPallet::add_feed_creator(Origin::signed(1), 2),
            Error::<Test>::NotPalletAdmin
        );
    });
}

#[test]
fn remove_feed_creator_fails_without_pallet_admin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            ParalinkFeedPallet::remove_feed_creator(Origin::signed(1), 1),
            Error::<Test>::NotPalletAdmin
        );
    });
}

#[test]
fn add_feed_creator_works() {
    new_test_ext().execute_with(|| {
        let new_creator = 2;
        assert_noop!(
            ParalinkFeedPallet::create_feed(
                Origin::signed(new_creator),
                IPFS_HASH.into(),
                "BTC/USD".into(),
                8
            ),
            Error::<Test>::NotFeedCreator
        );

        Balances::make_free_balance_be(&new_creator, 10_000);
        assert_ok!(ParalinkFeedPallet::add_feed_creator(
            Origin::signed(6),
            new_creator
        ));

        System::assert_last_event(mock::Event::ParalinkFeedPallet(Event::FeedCreatorAdded(
            new_creator,
        )));

        // the new_creator can now create a feed
        assert_ok!(ParalinkFeedPallet::create_feed(
            Origin::signed(new_creator),
            IPFS_HASH.into(),
            "BTC/USD".into(),
            8
        ));
    });
}

#[test]
fn remove_feed_creator_works() {
    new_test_ext().execute_with(|| {
        let old_creator = 1;
        assert_ok!(ParalinkFeedPallet::create_feed(
            Origin::signed(old_creator),
            IPFS_HASH.into(),
            "BTC/USD".into(),
            8
        ));

        assert_ok!(ParalinkFeedPallet::remove_feed_creator(
            Origin::signed(6),
            old_creator
        ));

        System::assert_last_event(mock::Event::ParalinkFeedPallet(Event::FeedCreatorRemoved(
            old_creator,
        )));

        // the old_creator can no longer create a feed
        assert_noop!(
            ParalinkFeedPallet::create_feed(
                Origin::signed(old_creator),
                IPFS_HASH.into(),
                "BTC/USD".into(),
                8
            ),
            Error::<Test>::NotFeedCreator
        );
    });
}

#[test]
fn transfer_ownership_works() {
    new_test_ext().execute_with(|| {
        let old_owner = 1;
        let new_owner = 2;
        let feed_id = 0;

        assert_ok!(ParalinkFeedPallet::create_feed(
            Origin::signed(old_owner),
            IPFS_HASH.into(),
            "BTC/USD".into(),
            8
        ));

        assert_ok!(ParalinkFeedPallet::transfer_ownership(
            Origin::signed(old_owner),
            feed_id,
            new_owner
        ));

        System::assert_last_event(mock::Event::ParalinkFeedPallet(Event::OwnerUpdated(
            feed_id, new_owner,
        )));

        assert!(ParalinkFeedPallet::feed_config(feed_id).unwrap().owner == new_owner);

        // Old owner has no permission to transfer ownership
        assert_noop!(
            ParalinkFeedPallet::transfer_ownership(Origin::signed(old_owner), feed_id, new_owner),
            Error::<Test>::NotFeedOwner
        );

        // But the new_owner can
        assert_ok!(ParalinkFeedPallet::transfer_ownership(
            Origin::signed(new_owner),
            feed_id,
            old_owner
        ));

        System::assert_last_event(mock::Event::ParalinkFeedPallet(Event::OwnerUpdated(
            feed_id, old_owner,
        )));
    });
}

#[test]
fn transfer_ownership_fails_with_non_existing_feed() {
    new_test_ext().execute_with(|| {
        let old_owner = 1;
        let new_owner = 2;
        let feed_id = 1337;

        assert_noop!(
            ParalinkFeedPallet::transfer_ownership(Origin::signed(old_owner), feed_id, new_owner),
            Error::<Test>::FeedNotFound
        );
    });
}

#[test]
fn submit_works() {
    new_test_ext().execute_with(|| {
        let alice = 1;
        let feed_id = 0;
        let value = 15;

        let description: &str = "BTC/USD";
        let decimals = 8;
        assert_ok!(ParalinkFeedPallet::create_feed(
            Origin::signed(alice),
            IPFS_HASH.into(),
            "BTC/USD".into(),
            8
        ));

        assert_ok!(ParalinkFeedPallet::submit(
            Origin::signed(alice),
            feed_id,
            value
        ),);

        System::assert_last_event(mock::Event::ParalinkFeedPallet(Event::FeedValueUpdated(
            feed_id, value,
        )));

        // New feed on ID = 0 should match given data
        assert_eq!(
            ParalinkFeedPallet::feed_config(feed_id),
            Some(FeedConfig {
                description: description.as_bytes().to_vec().try_into().unwrap(),
                ipfs_hash: IPFS_HASH.as_bytes().to_vec().try_into().unwrap(),
                decimals,
                first_valid_round: Some(1),
                latest_round: 1,
                owner: alice
            })
        );

        // A Round object should also be inserted
        assert_eq!(
            ParalinkFeedPallet::round(feed_id, 1),
            Some(Round {
                answer: Some(value),
                updated_at: Some(1)
            })
        );

        System::set_block_number(2);

        // Submit once more to see a new value
        let value = 36;
        assert_ok!(ParalinkFeedPallet::submit(
            Origin::signed(alice),
            feed_id,
            value
        ));

        System::assert_last_event(mock::Event::ParalinkFeedPallet(Event::FeedValueUpdated(
            feed_id, value,
        )));

        // New feed on ID = 0 should match given data
        assert_eq!(
            ParalinkFeedPallet::feed_config(feed_id),
            Some(FeedConfig {
                description: description.as_bytes().to_vec().try_into().unwrap(),
                ipfs_hash: IPFS_HASH.as_bytes().to_vec().try_into().unwrap(),
                decimals,
                first_valid_round: Some(1),
                latest_round: 2,
                owner: alice
            })
        );

        // A Round object should also be inserted
        assert_eq!(
            ParalinkFeedPallet::round(feed_id, 2),
            Some(Round {
                answer: Some(value),
                updated_at: Some(2)
            })
        );

        // latest value should be the last value value
        assert_eq!(
            ParalinkFeedPallet::latest_value(feed_id).unwrap(),
            Round {
                answer: Some(value),
                updated_at: Some(2)
            }
        );

        // we can still query the old value
        assert_eq!(
            ParalinkFeedPallet::round(feed_id, 1).unwrap(),
            Round {
                answer: Some(15),
                updated_at: Some(1)
            }
        );
    });
}

#[test]
fn submit_input_failures() {
    new_test_ext().execute_with(|| {
        let alice = 1;
        let feed_id = 0;
        let value = 15;

        assert_ok!(ParalinkFeedPallet::create_feed(
            Origin::signed(alice),
            IPFS_HASH.into(),
            "BTC/USD".into(),
            8
        ));

        assert_noop!(
            ParalinkFeedPallet::submit(Origin::signed(alice), 123, value),
            Error::<Test>::FeedNotFound
        );

        assert_noop!(
            ParalinkFeedPallet::submit(Origin::signed(2), feed_id, value),
            Error::<Test>::NotFeedOwner
        );

        Balances::make_free_balance_be(&alice, 9999);
        assert_noop!(
            ParalinkFeedPallet::submit(Origin::signed(alice), feed_id, value),
            Error::<Test>::NotEnoughBalance
        );
    });
}
