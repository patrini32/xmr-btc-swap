use crate::api::request::{Method, Request};
use crate::api::Context;
use crate::bitcoin::bitcoin_address;
use crate::monero::monero_address;
use crate::{bitcoin, monero};
use anyhow::Result;
use jsonrpsee::server::RpcModule;
use jsonrpsee::types::Params;
use libp2p::core::Multiaddr;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

pub fn register_modules(context: Arc<Context>) -> RpcModule<Arc<Context>> {
    let mut module = RpcModule::new(context);

    module
        .register_async_method("suspend_current_swap", |params, context| async move {
            execute_request(params, Method::SuspendCurrentSwap, &context).await
        })
        .unwrap();

    module
        .register_async_method("get_swap_info", |params_raw, context| async move {
            let params: HashMap<String, Uuid> = params_raw.parse()?;

            let swap_id = params.get("swap_id").ok_or_else(|| {
                jsonrpsee_core::Error::Custom("Does not contain swap_id".to_string())
            })?;

            execute_request(
                params_raw,
                Method::GetSwapInfo { swap_id: *swap_id },
                &context,
            )
            .await
        })
        .unwrap();

    module
        .register_async_method("get_bitcoin_balance", |params, context| async move {
            execute_request(params, Method::Balance, &context).await
        })
        .unwrap();

    module
        .register_async_method("get_history", |params, context| async move {
            execute_request(params, Method::History, &context).await
        })
        .unwrap();

    module
        .register_async_method("get_raw_history", |params, context| async move {
            execute_request(params, Method::GetRawStates, &context).await
        })
        .unwrap();

    module
        .register_async_method("resume_swap", |params_raw, context| async move {
            let params: HashMap<String, Uuid> = params_raw.parse()?;

            let swap_id = params.get("swap_id").ok_or_else(|| {
                jsonrpsee_core::Error::Custom("Does not contain swap_id".to_string())
            })?;

            execute_request(params_raw, Method::Resume { swap_id: *swap_id }, &context).await
        })
        .unwrap();

    module
        .register_async_method("cancel_refund_swap", |params_raw, context| async move {
            let params: HashMap<String, Uuid> = params_raw.parse()?;

            let swap_id = params.get("swap_id").ok_or_else(|| {
                jsonrpsee_core::Error::Custom("Does not contain swap_id".to_string())
            })?;

            execute_request(
                params_raw,
                Method::CancelAndRefund { swap_id: *swap_id },
                &context,
            )
            .await
        })
        .unwrap();

    module
        .register_async_method(
            "get_monero_recovery_info",
            |params_raw, context| async move {
                let params: HashMap<String, Uuid> = params_raw.parse()?;

                let swap_id = params.get("swap_id").ok_or_else(|| {
                    jsonrpsee_core::Error::Custom("Does not contain swap_id".to_string())
                })?;

                execute_request(
                    params_raw,
                    Method::MoneroRecovery { swap_id: *swap_id },
                    &context,
                )
                .await
            },
        )
        .unwrap();

    module
        .register_async_method("withdraw_btc", |params_raw, context| async move {
            let params: HashMap<String, String> = params_raw.parse()?;

            let amount = if let Some(amount_str) = params.get("amount") {
                Some(
                    ::bitcoin::Amount::from_str_in(amount_str, ::bitcoin::Denomination::Bitcoin)
                        .map_err(|_| {
                            jsonrpsee_core::Error::Custom("Unable to parse amount".to_string())
                        })?,
                )
            } else {
                None
            };

            let withdraw_address =
                bitcoin::Address::from_str(params.get("address").ok_or_else(|| {
                    jsonrpsee_core::Error::Custom("Does not contain address".to_string())
                })?)
                .map_err(|err| jsonrpsee_core::Error::Custom(err.to_string()))?;
            let withdraw_address =
                bitcoin_address::validate(withdraw_address, context.config.is_testnet)?;

            execute_request(
                params_raw,
                Method::WithdrawBtc {
                    amount,
                    address: withdraw_address,
                },
                &context,
            )
            .await
        })
        .unwrap();

    module
        .register_async_method("buy_xmr", |params_raw, context| async move {
            let params: HashMap<String, String> = params_raw.parse()?;

            let bitcoin_change_address = bitcoin::Address::from_str(
                params.get("bitcoin_change_address").ok_or_else(|| {
                    jsonrpsee_core::Error::Custom(
                        "Does not contain bitcoin_change_address".to_string(),
                    )
                })?,
            )
            .map_err(|err| jsonrpsee_core::Error::Custom(err.to_string()))?;

            let bitcoin_change_address =
                bitcoin_address::validate(bitcoin_change_address, context.config.is_testnet)?;

            let monero_receive_address = monero::Address::from_str(
                params.get("monero_receive_address").ok_or_else(|| {
                    jsonrpsee_core::Error::Custom(
                        "Does not contain monero_receiveaddress".to_string(),
                    )
                })?,
            )
            .map_err(|err| jsonrpsee_core::Error::Custom(err.to_string()))?;

            let monero_receive_address =
                monero_address::validate(monero_receive_address, context.config.is_testnet)?;

            let seller = Multiaddr::from_str(params.get("seller").ok_or_else(|| {
                jsonrpsee_core::Error::Custom("Does not contain seller".to_string())
            })?)
            .map_err(|err| jsonrpsee_core::Error::Custom(err.to_string()))?;

            execute_request(
                params_raw,
                Method::BuyXmr {
                    bitcoin_change_address,
                    monero_receive_address,
                    seller,
                    swap_id: Uuid::new_v4(),
                },
                &context,
            )
            .await
        })
        .unwrap();

    module
        .register_async_method("list_sellers", |params_raw, context| async move {
            let params: HashMap<String, Multiaddr> = params_raw.parse()?;
            let rendezvous_point = params.get("rendezvous_point").ok_or_else(|| {
                jsonrpsee_core::Error::Custom("Does not contain rendezvous_point".to_string())
            })?;

            execute_request(
                params_raw,
                Method::ListSellers {
                    rendezvous_point: rendezvous_point.clone(),
                },
                &context,
            )
            .await
        })
        .unwrap();

    module
        .register_async_method("get_current_swap", |params, context| async move {
            execute_request(params, Method::GetCurrentSwap, &context).await
        })
        .unwrap();

    module
}

async fn execute_request(
    params: Params<'static>,
    cmd: Method,
    context: &Arc<Context>,
) -> Result<serde_json::Value, jsonrpsee_core::Error> {
    let params_parsed = params
        .parse::<HashMap<String, String>>()
        .map_err(|err| jsonrpsee_core::Error::Custom(err.to_string()))?;

    let reference_id = params_parsed.get("log_reference_id");

    let request = Request::with_id(cmd, reference_id.map(|s| s.clone()));
    request
        .call(Arc::clone(context))
        .await
        .map_err(|err| jsonrpsee_core::Error::Custom(err.to_string()))
}