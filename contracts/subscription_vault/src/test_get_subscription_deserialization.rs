//! Regression tests for issue #598 (#462): `get_subscription` must return
//! `Error::NotFound` rather than panicking when a `DataKey::Sub(id)` entry
//! contains bytes that fail to deserialize into `Subscription`.

use crate::types::DataKey;
use crate::{Error, SubscriptionVault, SubscriptionVaultClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{contracttype, Address, Env, String, Symbol};

#[contracttype]
#[derive(Clone, Debug)]
struct LegacyIncompatibleSubscription {
    id: u32,
    legacy_label: Symbol,
}

fn setup() -> (Env, Address, SubscriptionVaultClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SubscriptionVault, ());
    let client = SubscriptionVaultClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    client.init(&token, &6, &admin, &1i128, &(7 * 24 * 60 * 60));

    let merchant = Address::generate(&env);
    (env, contract_id, client, merchant)
}

#[test]
fn get_subscription_returns_not_found_when_missing() {
    let (_env, _contract_id, client, _merchant) = setup();
    let res = client.try_get_subscription(&999);
    assert!(matches!(res, Err(Ok(Error::NotFound))));
}

#[test]
fn get_subscription_returns_not_found_on_incompatible_schema_payload() {
    let (env, contract_id, client, _merchant) = setup();

    // Store an incompatible struct directly at DataKey::Sub(42) to simulate a
    // pre-migration or corrupted persistent entry.
    env.as_contract(&contract_id, || {
        let corrupted = LegacyIncompatibleSubscription {
            id: 42,
            legacy_label: Symbol::new(&env, "v1_stale"),
        };
        env.storage().persistent().set(&DataKey::Sub(42), &corrupted);
    });

    // Must return Err(Error::NotFound) instead of trapping with a host panic.
    let res = client.try_get_subscription(&42);
    assert!(matches!(res, Err(Ok(Error::NotFound))));
}

#[test]
fn get_subscription_returns_not_found_on_primitive_payload() {
    let (env, contract_id, client, _merchant) = setup();

    env.as_contract(&contract_id, || {
        let corrupted = String::from_str(&env, "not-a-subscription");
        env.storage().persistent().set(&DataKey::Sub(7), &corrupted);
    });

    let res = client.try_get_subscription(&7);
    assert!(matches!(res, Err(Ok(Error::NotFound))));
}

#[test]
fn read_subscription_returns_none_on_deserialization_failure() {
    let (env, contract_id, _client, _merchant) = setup();

    env.as_contract(&contract_id, || {
        env.storage().persistent().set(&DataKey::Sub(11), &12345u64);
        assert!(crate::subscription::read_subscription(&env, 11).is_none());
        assert!(matches!(
            crate::subscription::try_get_subscription(&env, 11),
            Err(Error::NotFound)
        ));
    });
}
