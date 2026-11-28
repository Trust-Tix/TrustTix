#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Env};

const XLM: i128 = 10_000_000;

struct Prueba<'a> {
    env: Env,
    tix: PagosClient<'a>,
    token: TokenClient<'a>,
    org: Address,
}

fn preparar<'a>() -> Prueba<'a> {
    let env = Env::default();
    env.mock_all_auths();
    let emisor = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(emisor).address();
    let id = env.register(Pagos, (token_id.clone(),));
    Prueba {
        tix: PagosClient::new(&env, &id),
        token: TokenClient::new(&env, &token_id),
        org: Address::generate(&env),
        env,
    }
}

fn con_xlm(p: &Prueba, monto: i128) -> Address {
    let cuenta = Address::generate(&p.env);
    StellarAssetClient::new(&p.env, &p.token.address).mint(&cuenta, &monto);
    cuenta
}

#[test]
fn suscribirse_paga_el_primer_periodo_y_autoriza_los_demas() {
    let p = preparar();
    let fan = con_xlm(&p, 30 * XLM);
    let id = p
        .tix
        .suscribirse(&fan, &p.org, &(10 * XLM), &100, &3, &1_000);
    assert_eq!(p.token.balance(&p.org), 10 * XLM);
    assert_eq!(p.token.allowance(&fan, &p.tix.address), 20 * XLM);
    assert_eq!(p.tix.consultar_suscripcion(&id).restantes, 2);
}

#[test]
fn cobrar_suscripcion_solo_cuando_vence_el_periodo() {
    let p = preparar();
    let fan = con_xlm(&p, 30 * XLM);
    p.env.ledger().set_timestamp(1_000);
    let id = p
        .tix
        .suscribirse(&fan, &p.org, &(10 * XLM), &100, &3, &1_000);
    assert_eq!(p.tix.try_cobrar_suscripcion(&id), Err(Ok(Error::MuyPronto)));
    p.env.ledger().set_timestamp(1_100);
    assert_eq!(p.tix.cobrar_suscripcion(&id), 1);
    assert_eq!(p.env.auths()[0].0, p.org);
    assert_eq!(p.token.balance(&p.org), 20 * XLM);
    p.env.ledger().set_timestamp(1_200);
    assert_eq!(p.tix.cobrar_suscripcion(&id), 0);
    p.env.ledger().set_timestamp(1_300);
    assert_eq!(
        p.tix.try_cobrar_suscripcion(&id),
        Err(Ok(Error::SinPeriodos))
    );
    assert_eq!(p.token.balance(&fan), 0);
}

#[test]
fn crear_pago_programado_deposita_el_total() {
    let p = preparar();
    StellarAssetClient::new(&p.env, &p.token.address).mint(&p.org, &(100 * XLM));
    let staff = Address::generate(&p.env);
    p.tix
        .crear_pago_programado(&p.org, &staff, &(100 * XLM), &1_000);
    assert_eq!(p.token.balance(&p.tix.address), 100 * XLM);
}

#[test]
fn cobrar_devengado_paga_lo_liberado_por_el_tiempo() {
    let p = preparar();
    StellarAssetClient::new(&p.env, &p.token.address).mint(&p.org, &(100 * XLM));
    let staff = Address::generate(&p.env);
    p.env.ledger().set_timestamp(1_000);
    let id = p
        .tix
        .crear_pago_programado(&p.org, &staff, &(100 * XLM), &1_000);
    p.env.ledger().set_timestamp(1_250);
    assert_eq!(p.tix.cobrar_devengado(&id), 25 * XLM);
    assert_eq!(p.env.auths()[0].0, staff);
    assert_eq!(
        p.tix.try_cobrar_devengado(&id),
        Err(Ok(Error::NadaQueCobrar))
    );
    p.env.ledger().set_timestamp(3_000);
    assert_eq!(p.tix.cobrar_devengado(&id), 75 * XLM);
    assert_eq!(p.token.balance(&staff), 100 * XLM);
}

#[test]
fn terminar_pago_programado_reparte_lo_ganado_y_lo_restante() {
    let p = preparar();
    StellarAssetClient::new(&p.env, &p.token.address).mint(&p.org, &(100 * XLM));
    let staff = Address::generate(&p.env);
    p.env.ledger().set_timestamp(1_000);
    let id = p
        .tix
        .crear_pago_programado(&p.org, &staff, &(100 * XLM), &1_000);
    p.env.ledger().set_timestamp(1_400);
    assert_eq!(p.tix.terminar_pago_programado(&id), 60 * XLM);
    assert_eq!(p.token.balance(&staff), 40 * XLM);
    assert_eq!(p.token.balance(&p.org), 60 * XLM);
    assert_eq!(p.tix.try_cobrar_devengado(&id), Err(Ok(Error::Cerrado)));
}

#[test]
fn fondear_cashback_suma_a_la_reserva_del_comercio() {
    let p = preparar();
    let barra = con_xlm(&p, 10 * XLM);
    assert_eq!(p.tix.fondear_cashback(&barra, &(10 * XLM)), 10 * XLM);
    assert_eq!(p.tix.reserva_de(&barra), 10 * XLM);
    assert_eq!(p.token.balance(&p.tix.address), 10 * XLM);
}

#[test]
fn pagar_con_cashback_regresa_el_cinco_por_ciento() {
    let p = preparar();
    let barra = con_xlm(&p, XLM);
    p.tix.fondear_cashback(&barra, &XLM);
    let fan = con_xlm(&p, 20 * XLM);
    assert_eq!(p.tix.pagar_con_cashback(&fan, &barra, &(20 * XLM)), XLM);
    assert_eq!(p.token.balance(&barra), 20 * XLM);
    assert_eq!(p.token.balance(&fan), XLM);
    assert_eq!(p.tix.reserva_de(&barra), 0);
    let otro = con_xlm(&p, 20 * XLM);
    assert_eq!(
        p.tix.try_pagar_con_cashback(&otro, &barra, &(20 * XLM)),
        Err(Ok(Error::SinReserva))
    );
}
