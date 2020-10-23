pub mod harness;

mod tests {
    // NOTE: For some reason running these tests overflows the stack. In order to
    // mitigate this run them with:
    //
    //     RUST_MIN_STACK=100000000 cargo test

    use crate::{
        harness,
        harness::{
            init_bitcoind, init_test,
            node::{run_alice_until, run_bob_until},
        },
    };
    use futures::future;
    use monero_harness::Monero;
    use rand::rngs::OsRng;

    use crate::harness::storage::Database;
    use std::convert::TryInto;
    use testcontainers::clients::Cli;
    use tracing_subscriber::util::SubscriberInitExt;
    use xmr_btc::{
        alice, bitcoin,
        bitcoin::{Amount, TX_FEE},
        bob,
    };

    #[tokio::test]
    async fn happy_path() {
        let _guard = tracing_subscriber::fmt()
            .with_env_filter("info")
            .set_default();

        let cli = Cli::default();
        let (monero, _container) = Monero::new(&cli).unwrap();
        let bitcoind = init_bitcoind(&cli).await;

        let (
            alice_state0,
            bob_state0,
            mut alice_node,
            mut bob_node,
            initial_balances,
            swap_amounts,
        ) = init_test(&monero, &bitcoind, None, None).await;

        let (alice_state, bob_state) = future::try_join(
            run_alice_until(
                &mut alice_node,
                alice_state0.into(),
                harness::alice::is_state6,
                &mut OsRng,
            ),
            run_bob_until(
                &mut bob_node,
                bob_state0.into(),
                harness::bob::is_state5,
                &mut OsRng,
            ),
        )
        .await
        .unwrap();

        let alice_state6: alice::State6 = alice_state.try_into().unwrap();
        let bob_state5: bob::State5 = bob_state.try_into().unwrap();

        let alice_final_btc_balance = alice_node.bitcoin_wallet.balance().await.unwrap();
        let bob_final_btc_balance = bob_node.bitcoin_wallet.balance().await.unwrap();

        let lock_tx_bitcoin_fee = bob_node
            .bitcoin_wallet
            .transaction_fee(bob_state5.tx_lock_id())
            .await
            .unwrap();

        let alice_final_xmr_balance = alice_node.monero_wallet.get_balance().await.unwrap();

        monero.wait_for_bob_wallet_block_height().await.unwrap();

        let bob_final_xmr_balance = bob_node.monero_wallet.get_balance().await.unwrap();

        assert_eq!(
            alice_final_btc_balance,
            initial_balances.alice_btc + swap_amounts.btc
                - bitcoin::Amount::from_sat(bitcoin::TX_FEE)
        );
        assert_eq!(
            bob_final_btc_balance,
            initial_balances.bob_btc - swap_amounts.btc - lock_tx_bitcoin_fee
        );

        assert_eq!(
            alice_final_xmr_balance,
            initial_balances.alice_xmr - swap_amounts.xmr - alice_state6.lock_xmr_fee()
        );
        assert_eq!(
            bob_final_xmr_balance,
            initial_balances.bob_xmr + swap_amounts.xmr
        );
    }

    #[tokio::test]
    async fn both_refund() {
        let _guard = tracing_subscriber::fmt()
            .with_env_filter("info")
            .set_default();

        let cli = Cli::default();
        let (monero, _container) = Monero::new(&cli).unwrap();
        let bitcoind = init_bitcoind(&cli).await;

        let (
            alice_state0,
            bob_state0,
            mut alice_node,
            mut bob_node,
            initial_balances,
            swap_amounts,
        ) = init_test(&monero, &bitcoind, None, None).await;

        let (alice_state, bob_state) = future::try_join(
            run_alice_until(
                &mut alice_node,
                alice_state0.into(),
                harness::alice::is_state5,
                &mut OsRng,
            ),
            run_bob_until(
                &mut bob_node,
                bob_state0.into(),
                harness::bob::is_state3,
                &mut OsRng,
            ),
        )
        .await
        .unwrap();

        let alice_state5: alice::State5 = alice_state.try_into().unwrap();
        let bob_state3: bob::State3 = bob_state.try_into().unwrap();

        bob_state3
            .refund_btc(&bob_node.bitcoin_wallet)
            .await
            .unwrap();
        alice_state5
            .refund_xmr(&alice_node.bitcoin_wallet, &alice_node.monero_wallet)
            .await
            .unwrap();

        let alice_final_btc_balance = alice_node.bitcoin_wallet.balance().await.unwrap();
        let bob_final_btc_balance = bob_node.bitcoin_wallet.balance().await.unwrap();

        // lock_tx_bitcoin_fee is determined by the wallet, it is not necessarily equal
        // to TX_FEE
        let lock_tx_bitcoin_fee = bob_node
            .bitcoin_wallet
            .transaction_fee(bob_state3.tx_lock_id())
            .await
            .unwrap();

        monero.wait_for_alice_wallet_block_height().await.unwrap();
        let alice_final_xmr_balance = alice_node.monero_wallet.get_balance().await.unwrap();
        let bob_final_xmr_balance = bob_node.monero_wallet.get_balance().await.unwrap();

        assert_eq!(alice_final_btc_balance, initial_balances.alice_btc);
        assert_eq!(
            bob_final_btc_balance,
            // The 2 * TX_FEE corresponds to tx_refund and tx_cancel.
            initial_balances.bob_btc - Amount::from_sat(2 * TX_FEE) - lock_tx_bitcoin_fee
        );

        // Because we create a new wallet when claiming Monero, we can only assert on
        // this new wallet owning all of `xmr_amount` after refund
        assert_eq!(alice_final_xmr_balance, swap_amounts.xmr);
        assert_eq!(bob_final_xmr_balance, initial_balances.bob_xmr);
    }

    #[tokio::test]
    async fn alice_punishes() {
        let _guard = tracing_subscriber::fmt()
            .with_env_filter("info")
            .set_default();

        let cli = Cli::default();
        let (monero, _container) = Monero::new(&cli).unwrap();
        let bitcoind = init_bitcoind(&cli).await;

        let (
            alice_state0,
            bob_state0,
            mut alice_node,
            mut bob_node,
            initial_balances,
            swap_amounts,
        ) = init_test(&monero, &bitcoind, None, None).await;

        let (alice_state, bob_state) = future::try_join(
            run_alice_until(
                &mut alice_node,
                alice_state0.into(),
                harness::alice::is_state4,
                &mut OsRng,
            ),
            run_bob_until(
                &mut bob_node,
                bob_state0.into(),
                harness::bob::is_state3,
                &mut OsRng,
            ),
        )
        .await
        .unwrap();

        let alice_state4: alice::State4 = alice_state.try_into().unwrap();
        let bob_state3: bob::State3 = bob_state.try_into().unwrap();

        alice_state4
            .punish(&alice_node.bitcoin_wallet)
            .await
            .unwrap();

        let alice_final_btc_balance = alice_node.bitcoin_wallet.balance().await.unwrap();
        let bob_final_btc_balance = bob_node.bitcoin_wallet.balance().await.unwrap();

        // lock_tx_bitcoin_fee is determined by the wallet, it is not necessarily equal
        // to TX_FEE
        let lock_tx_bitcoin_fee = bob_node
            .bitcoin_wallet
            .transaction_fee(bob_state3.tx_lock_id())
            .await
            .unwrap();

        assert_eq!(
            alice_final_btc_balance,
            initial_balances.alice_btc + swap_amounts.btc - Amount::from_sat(2 * TX_FEE)
        );
        assert_eq!(
            bob_final_btc_balance,
            initial_balances.bob_btc - swap_amounts.btc - lock_tx_bitcoin_fee
        );
    }

    #[tokio::test]
    async fn recover_protocol_state_from_db() {
        let _guard = tracing_subscriber::fmt()
            .with_env_filter("info")
            .set_default();

        let cli = Cli::default();
        let (monero, _container) = Monero::new(&cli).unwrap();
        let bitcoind = init_bitcoind(&cli).await;

        let alice_db_dir = tempfile::tempdir().unwrap();
        let alice_db: Database<alice::State> =
            harness::storage::Database::open(alice_db_dir.path()).unwrap();
        let bob_db_dir = tempfile::tempdir().unwrap();
        let bob_db: Database<bob::State> =
            harness::storage::Database::open(bob_db_dir.path()).unwrap();

        let (
            alice_state0,
            bob_state0,
            mut alice_node,
            mut bob_node,
            initial_balances,
            swap_amounts,
        ) = init_test(&monero, &bitcoind, None, None).await;

        {
            let (alice_state, bob_state) = future::try_join(
                run_alice_until(
                    &mut alice_node,
                    alice_state0.into(),
                    harness::alice::is_state5,
                    &mut OsRng,
                ),
                run_bob_until(
                    &mut bob_node,
                    bob_state0.into(),
                    harness::bob::is_state3,
                    &mut OsRng,
                ),
            )
            .await
            .unwrap();

            // save state to db
            alice_db.insert_latest_state(&alice_state).await.unwrap();
            bob_db.insert_latest_state(&bob_state).await.unwrap();
        };

        let (alice_state6, bob_state5) = {
            // recover state from db
            let alice_state = alice_db.get_latest_state().unwrap();
            let bob_state = bob_db.get_latest_state().unwrap();

            let (alice_state, bob_state) = future::try_join(
                run_alice_until(
                    &mut alice_node,
                    alice_state,
                    harness::alice::is_state6,
                    &mut OsRng,
                ),
                run_bob_until(
                    &mut bob_node,
                    bob_state,
                    harness::bob::is_state5,
                    &mut OsRng,
                ),
            )
            .await
            .unwrap();

            let alice_state: alice::State6 = alice_state.try_into().unwrap();
            let bob_state: bob::State5 = bob_state.try_into().unwrap();

            (alice_state, bob_state)
        };

        let alice_final_btc_balance = alice_node.bitcoin_wallet.balance().await.unwrap();
        let bob_final_btc_balance = bob_node.bitcoin_wallet.balance().await.unwrap();

        let lock_tx_bitcoin_fee = bob_node
            .bitcoin_wallet
            .transaction_fee(bob_state5.tx_lock_id())
            .await
            .unwrap();

        let alice_final_xmr_balance = alice_node.monero_wallet.0.get_balance(0).await.unwrap();

        monero.wait_for_bob_wallet_block_height().await.unwrap();

        let bob_final_xmr_balance = bob_node.monero_wallet.0.get_balance(0).await.unwrap();

        assert_eq!(
            alice_final_btc_balance,
            initial_balances.alice_btc + swap_amounts.btc
                - bitcoin::Amount::from_sat(bitcoin::TX_FEE)
        );
        assert_eq!(
            bob_final_btc_balance,
            initial_balances.bob_btc - swap_amounts.btc - lock_tx_bitcoin_fee
        );

        assert_eq!(
            alice_final_xmr_balance,
            initial_balances.alice_xmr.as_piconero()
                - swap_amounts.xmr.as_piconero()
                - alice_state6.lock_xmr_fee().as_piconero()
        );
        assert_eq!(
            bob_final_xmr_balance,
            initial_balances.bob_xmr.as_piconero() + swap_amounts.xmr.as_piconero()
        );
    }
}
