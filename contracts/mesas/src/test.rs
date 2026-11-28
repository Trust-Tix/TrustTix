#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Env};

const XLM: i128 = 10_000_000;

struct Prueba<'a> {
    env: Env,
    tix: MesasClient<'a>,
    token: TokenClient<'a>,
    recinto: Address,
}

fn preparar<'a>() -> Prueba<'a> {
    let env = Env::default();
    env.mock_all_auths();
    let emisor = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(emisor).address();
    let id = env.register(Mesas, (token_id.clone(),));
    Prueba {
        tix: MesasClient::new(&env, &id),
        token: TokenClient::new(&env, &token_id),
        recinto: Address::generate(&env),
        env,
    }
}

fn con_xlm(p: &Prueba, monto: i128) -> Address {
    let cuenta = Address::generate(&p.env);
    StellarAssetClient::new(&p.env, &p.token.address).mint(&cuenta, &monto);
    cuenta
}

#[test]
fn reservar_mesa_paga_el_consumo_minimo_en_garantia() {
    let p = preparar();
    let cliente = con_xlm(&p, 20 * XLM);
    let mesa = p.tix.reservar_mesa(&cliente, &p.recinto, &(20 * XLM));
    assert_eq!(p.token.balance(&p.tix.address), 20 * XLM);
    assert_eq!(p.tix.consultar_mesa(&mesa).minimo, 20 * XLM);
}

#[test]
fn cargar_consumo_cobra_de_la_garantia() {
    let p = preparar();
    let cliente = con_xlm(&p, 20 * XLM);
    let mesa = p.tix.reservar_mesa(&cliente, &p.recinto, &(20 * XLM));
    assert_eq!(p.tix.cargar_consumo(&mesa, &(8 * XLM)), 8 * XLM);
    assert_eq!(p.env.auths()[0].0, p.recinto);
    assert_eq!(p.token.balance(&p.recinto), 8 * XLM);
    assert_eq!(
        p.tix.try_cargar_consumo(&mesa, &(13 * XLM)),
        Err(Ok(Error::SinSaldo))
    );
}

#[test]
fn cerrar_mesa_cobra_lo_que_falta_del_minimo() {
    let p = preparar();
    let cliente = con_xlm(&p, 20 * XLM);
    let mesa = p.tix.reservar_mesa(&cliente, &p.recinto, &(20 * XLM));
    p.tix.cargar_consumo(&mesa, &(8 * XLM));
    assert_eq!(p.tix.cerrar_mesa(&mesa), 12 * XLM);
    assert_eq!(p.token.balance(&p.recinto), 20 * XLM);
    assert_eq!(
        p.tix.try_cargar_consumo(&mesa, &XLM),
        Err(Ok(Error::Cerrado))
    );
}

#[test]
fn programar_regalo_guarda_el_dinero_en_el_contrato() {
    let p = preparar();
    let tia = con_xlm(&p, 10 * XLM);
    let sobrina = Address::generate(&p.env);
    p.tix.programar_regalo(&tia, &sobrina, &(10 * XLM), &5_000);
    assert_eq!(p.token.balance(&tia), 0);
    assert_eq!(p.token.balance(&p.tix.address), 10 * XLM);
}

#[test]
fn reclamar_regalo_solo_desde_su_fecha() {
    let p = preparar();
    let tia = con_xlm(&p, 10 * XLM);
    let sobrina = Address::generate(&p.env);
    let regalo = p.tix.programar_regalo(&tia, &sobrina, &(10 * XLM), &5_000);
    assert_eq!(
        p.tix.try_reclamar_regalo(&regalo),
        Err(Ok(Error::MuyPronto))
    );
    p.env.ledger().set_timestamp(5_000);
    assert_eq!(p.tix.reclamar_regalo(&regalo), 10 * XLM);
    assert_eq!(p.env.auths()[0].0, sobrina);
    assert_eq!(p.token.balance(&sobrina), 10 * XLM);
}

#[test]
fn revocar_regalo_solo_antes_de_su_fecha() {
    let p = preparar();
    let tia = con_xlm(&p, 10 * XLM);
    let sobrina = Address::generate(&p.env);
    let regalo = p.tix.programar_regalo(&tia, &sobrina, &(10 * XLM), &5_000);
    assert_eq!(p.tix.revocar_regalo(&regalo), 10 * XLM);
    assert_eq!(p.token.balance(&tia), 10 * XLM);
    let otro = p.tix.programar_regalo(&tia, &sobrina, &(10 * XLM), &5_000);
    p.env.ledger().set_timestamp(5_000);
    assert_eq!(
        p.tix.try_revocar_regalo(&otro),
        Err(Ok(Error::YaDisponible))
    );
}

#[test]
fn pagar_con_arbitro_deja_el_pago_en_garantia() {
    let p = preparar();
    let comprador = con_xlm(&p, 10 * XLM);
    let vendedor = Address::generate(&p.env);
    let arbitro = Address::generate(&p.env);
    p.tix
        .pagar_con_arbitro(&comprador, &vendedor, &arbitro, &(10 * XLM));
    assert_eq!(p.token.balance(&p.tix.address), 10 * XLM);
    assert_eq!(
        p.tix
            .try_pagar_con_arbitro(&comprador, &vendedor, &vendedor, &XLM),
        Err(Ok(Error::MismaCuenta))
    );
}

#[test]
fn aceptar_compra_libera_el_pago_al_vendedor() {
    let p = preparar();
    let comprador = con_xlm(&p, 10 * XLM);
    let vendedor = Address::generate(&p.env);
    let arbitro = Address::generate(&p.env);
    let compra = p
        .tix
        .pagar_con_arbitro(&comprador, &vendedor, &arbitro, &(10 * XLM));
    assert_eq!(p.tix.aceptar_compra(&compra), 10 * XLM);
    assert_eq!(p.env.auths()[0].0, comprador);
    assert_eq!(p.token.balance(&vendedor), 10 * XLM);
}

#[test]
fn resolver_disputa_reparte_segun_el_arbitro() {
    let p = preparar();
    let comprador = con_xlm(&p, 10 * XLM);
    let vendedor = Address::generate(&p.env);
    let arbitro = Address::generate(&p.env);
    let compra = p
        .tix
        .pagar_con_arbitro(&comprador, &vendedor, &arbitro, &(10 * XLM));
    assert_eq!(p.tix.resolver_disputa(&compra, &3_000), 3 * XLM);
    assert_eq!(p.env.auths()[0].0, arbitro);
    assert_eq!(p.token.balance(&vendedor), 3 * XLM);
    assert_eq!(p.token.balance(&comprador), 7 * XLM);
    assert_eq!(p.tix.try_aceptar_compra(&compra), Err(Ok(Error::Cerrado)));
}
