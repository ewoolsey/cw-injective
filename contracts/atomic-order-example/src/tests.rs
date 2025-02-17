use std::str::FromStr;

use cosmwasm_std::testing::{mock_info, MockApi, MockStorage};
use cosmwasm_std::{
    coins, to_binary, BankMsg, Binary, ContractResult, CosmosMsg, OwnedDeps, QuerierResult, Reply,
    SubMsgResponse, SubMsgResult, SystemResult, Uint128,
};

use injective_cosmwasm::InjectiveMsg::CreateSpotMarketOrder;
use injective_cosmwasm::{
    inj_mock_deps, inj_mock_env, HandlesMarketIdQuery, InjectiveQueryWrapper, InjectiveRoute,
    MarketId, OrderInfo, OrderType, OwnedDepsExt, SpotMarket, SpotMarketResponse, SpotOrder,
    SubaccountId, WasmMockQuerier,
};
use injective_math::FPDecimal;

use crate::contract::{execute, instantiate, reply, ATOMIC_ORDER_REPLY_ID};
use crate::helpers::{get_message_data, i32_to_dec};
use crate::msg::{ExecuteMsg, InstantiateMsg};

fn test_deps() -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier, InjectiveQueryWrapper> {
    inj_mock_deps(|querier| {
        querier.spot_market_response_handler = Some(Box::new(create_spot_market_handler()))
    })
}

#[test]
fn proper_initialization() {
    let sender_addr = "inj1x2ck0ql2ngyxqtw8jteyc0tchwnwxv7npaungt";
    let msg = InstantiateMsg {
        market_id: MarketId::new(
            "0x78c2d3af98c517b164070a739681d4bd4d293101e7ffc3a30968945329b47ec6".to_string(),
        )
        .expect("failed to create market_id"),
    };
    let info = mock_info(sender_addr, &coins(1000, "earth"));

    // we can just call .unwrap() to assert this was a success
    let res = instantiate(test_deps().as_mut_deps(), inj_mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());
}

#[test]
fn test_swap() {
    let contract_addr = "inj14hj2tavq8fpesdwxxcu44rty3hh90vhujaxlnz";
    let sender_addr = "inj1x2ck0ql2ngyxqtw8jteyc0tchwnwxv7npaungt";
    let market_id = MarketId::new(
        "0x78c2d3af98c517b164070a739681d4bd4d293101e7ffc3a30968945329b47ec6".to_string(),
    )
    .expect("failed to create market_id");

    let msg = InstantiateMsg {
        market_id: market_id.clone(),
    };
    let info = mock_info(contract_addr, &coins(1000, "earth"));
    let mut deps = test_deps();
    let env = inj_mock_env();
    let _ = instantiate(deps.as_mut_deps(), env.clone(), info, msg);

    let info = mock_info(sender_addr, &coins(9000, "usdt"));
    let msg = ExecuteMsg::SwapSpot {
        quantity: i32_to_dec(8),
        price: i32_to_dec(1000),
    };
    let res = execute(deps.as_mut_deps(), env.clone(), info, msg).unwrap();

    let expected_atomic_order_message = CreateSpotMarketOrder {
        sender: env.contract.address.to_owned(),
        order: SpotOrder {
            market_id,
            order_info: OrderInfo {
                subaccount_id: SubaccountId::new(
                    "0xade4a5f5803a439835c636395a8d648dee57b2fc000000000000000000000000"
                        .to_string(),
                )
                .expect("failed to create subaccount_id"),
                fee_recipient: Some(env.contract.address),
                price: i32_to_dec(1000),
                quantity: i32_to_dec(8),
            },
            order_type: OrderType::BuyAtomic,
            trigger_price: None,
        },
    };

    let order_message = get_message_data(&res.messages, 0);
    assert_eq!(
        InjectiveRoute::Exchange,
        order_message.route,
        "route was incorrect"
    );
    assert_eq!(
        expected_atomic_order_message, order_message.msg_data,
        "spot create order had incorrect content"
    );

    let binary_response = Binary::from_base64("CkIweGRkNzI5MmY2ODcwMzIwOTc2YTUxYTUwODBiMGQ2NDU5M2NhZjE3OWViM2YxOTNjZWVlZGFiNGVhNWUxNDljZWISQwoTODAwMDAwMDAwMDAwMDAwMDAwMBIWMTAwMDAwMDAwMDAwMDAwMDAwMDAwMBoUMzYwMDAwMDAwMDAwMDAwMDAwMDA=").unwrap();
    let reply_msg = Reply {
        id: ATOMIC_ORDER_REPLY_ID,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(binary_response),
        }),
    };

    let transfers_response = reply(deps.as_mut_deps(), inj_mock_env(), reply_msg);
    let messages = transfers_response.unwrap().messages;
    assert_eq!(messages.len(), 1);

    if let CosmosMsg::Bank(BankMsg::Send { to_address, amount }) = &messages[0].msg {
        assert_eq!(to_address, sender_addr);
        assert_eq!(2, amount.len());
        assert_eq!(amount[0].denom, "INJ");
        assert_eq!(amount[0].amount, Uint128::from(8u128));
        assert_eq!(amount[1].denom, "USDT");
        assert_eq!(amount[1].amount, Uint128::from(9000u128 - 8036u128));
    } else {
        panic!("Wrong message type!");
    }
}

fn create_spot_market_handler() -> impl HandlesMarketIdQuery {
    struct Temp();
    impl HandlesMarketIdQuery for Temp {
        fn handle(&self, market_id: MarketId) -> QuerierResult {
            let response = SpotMarketResponse {
                market: Some(SpotMarket {
                    ticker: "INJ/USDT".to_string(),
                    base_denom: "INJ".to_string(),
                    quote_denom: "USDT".to_string(),
                    maker_fee_rate: FPDecimal::from_str("0.01").unwrap(),
                    taker_fee_rate: FPDecimal::from_str("0.1").unwrap(),
                    relayer_fee_share_rate: FPDecimal::from_str("0.4").unwrap(),
                    market_id,
                    status: 0,
                    min_price_tick_size: FPDecimal::from_str("0.000000000000001").unwrap(),
                    min_quantity_tick_size: FPDecimal::from_str("1000000000000000").unwrap(),
                }),
            };
            SystemResult::Ok(ContractResult::from(to_binary(&response)))
        }
    }
    Temp()
}
