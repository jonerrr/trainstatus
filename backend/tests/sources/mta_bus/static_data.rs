use crate::common::{contracts, mta_bus_dataset};

#[test]
fn static_fixture_satisfies_source_contract() {
    contracts::assert_static_dataset_contract(&mta_bus_dataset());
}
