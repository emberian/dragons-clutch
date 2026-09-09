//! Actual Custody ELF entrypoint, invoked by the real caller's PDA signature.
//! ProgramData substitutions are fixture evidence, not native Loader upgrades.
use super::*;
use dclutch_custody_sbf::CustodySbfError;
use solana_sdk::{instruction::InstructionError, transaction::TransactionError};

#[tokio::test]
async fn real_elf_calling_release_requires_live_loader_continuity() {
    for mutation in ["accepted", "authority"] {
        let (mut test, fixture) = fixture_with_caller_authority(Profile::Legacy, Some([0x71; 32]));
        // Substitute before the bank starts. ProgramTest's set_account can
        // replace a loaded same-slot cache entry and panic independently of
        // the protocol; initial metadata gives the runtime one coherent image.
        let mut data = immutable_programdata(&artifacts().caller);
        *data.get_mut(12).expect("authority option") = 1;
        data.get_mut(13..45).expect("authority").fill(0x71);
        match mutation {
            "authority" => data.get_mut(13..45).expect("authority").fill(0x72),
            _ => {}
        }
        test.add_account(
            programdata_address(CALLER_PROGRAM_ID),
            Account {
                lamports: Rent::default().minimum_balance(data.len()),
                data,
                owner: bpf_loader_upgradeable::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
        let mut context = test.start_with_context().await;
        let payer = fixture.payer.pubkey();
        let instruction =
            wrapper_instruction(&fixture, initialize_request(&fixture, payer), payer, false);
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, &fixture.payer],
            context.last_blockhash,
        );
        let outcome = context
            .banks_client
            .process_transaction_with_metadata(transaction)
            .await
            .expect("entrypoint transaction");
        let metadata = outcome.metadata.expect("executed entrypoint metadata");
        let expected = match mutation {
            "authority" => Err(TransactionError::InstructionError(
                0,
                InstructionError::Custom(CustodySbfError::Release as u32),
            )),
            _ => Ok(()),
        };
        println!(
            "calling-release {mutation}: result={:?} CU={} logs={:?}",
            outcome.result, metadata.compute_units_consumed, metadata.log_messages
        );
        assert!(
            metadata
                .log_messages
                .iter()
                .any(|line| line == &format!("Program {CUSTODY_PROGRAM_ID} invoke [2]")),
            "must enter actual Custody ELF"
        );
        assert_eq!(outcome.result, expected, "{mutation}");
        let replay = context
            .banks_client
            .get_account(fixture.replay)
            .await
            .expect("replay poststate");
        if mutation == "accepted" {
            assert_eq!(replay.expect("created replay").owner, CUSTODY_PROGRAM_ID);
        } else {
            assert!(replay.is_none(), "refusal must not allocate replay");
        }
    }
}
