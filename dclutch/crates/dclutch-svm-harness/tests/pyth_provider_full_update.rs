// Real-SVM evidence that the provenance-pinned Pyth Router and Receiver
// accept the complete caller-owned lifecycle used by flagship resolution.

#[path = "support/pyth_provider.rs"]
#[allow(dead_code)]
mod pyth_provider;

use solana_program_test::ProgramTest;

#[tokio::test]
async fn pinned_real_provider_executes_verify_post_and_reclaim() {
    pyth_provider::assert_all_fixture_hashes();
    let provider = pyth_provider::ProviderAddresses::pinned();
    let mut test = ProgramTest::default();
    test.set_compute_max_units(1_400_000);
    pyth_provider::add_upgraded_provider_programs(&mut test, provider);
    let mut context = test.start_with_context().await;

    let encoded_vaa = pyth_provider::initialize_real_providers(&mut context, provider).await;
    pyth_provider::set_fixture_clock(&mut context).await;
    pyth_provider::prove_full_provider_update(&mut context, provider, encoded_vaa).await;
}
