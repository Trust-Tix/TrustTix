#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Env};

const XLM: i128 = 10_000_000;

struct Prueba<'a> {
    env: Env,
    tix: SubastasClient<'a>,
    token: TokenClient<'a>,
    org: Address,
}

fn preparar<'a>() -> Prueba<'a> {
    let env = Env::default();
    env.mock_all_auths();
    let emisor = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(emisor).address();
    let id = env.register(Subastas, (token_id.clone(),));
    Prueba {
        tix: SubastasClient::new(&env, &id),
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
fn pujar_deja_la_puja_en_garantia_y_regresa_la_anterior() {
    let p = preparar();
    let id = p.tix.crear_subasta(&p.org, &(10 * XLM), &0);
    let a = con_xlm(&p, 20 * XLM);
    let b = con_xlm(&p, 20 * XLM);
    p.tix.pujar(&a, &id, &(12 * XLM));
    assert_eq!(p.token.balance(&a), 8 * XLM);
    p.tix.pujar(&b, &id, &(15 * XLM));
    assert_eq!(p.token.balance(&a), 20 * XLM);
    assert_eq!(p.token.balance(&p.tix.address), 15 * XLM);
    assert_eq!(
        p.tix.try_pujar(&a, &id, &(15 * XLM)),
        Err(Ok(Error::PujaBaja))
    );
}

#[test]
fn cerrar_subasta_paga_la_mejor_puja_al_vendedor() {
    let p = preparar();
    let id = p.tix.crear_subasta(&p.org, &(10 * XLM), &0);
    let a = con_xlm(&p, 20 * XLM);
    p.tix.pujar(&a, &id, &(12 * XLM));
    assert_eq!(p.tix.cerrar_subasta(&id), 12 * XLM);
    assert_eq!(p.env.auths()[0].0, p.org);
    assert_eq!(p.token.balance(&p.org), 12 * XLM);
    assert_eq!(p.tix.consultar_subasta(&id).ganador, Some(a));
    assert_eq!(p.tix.try_cerrar_subasta(&id), Err(Ok(Error::Cerrada)));
}

#[test]
fn cancelar_subasta_regresa_la_mejor_puja() {
    let p = preparar();
    let id = p.tix.crear_subasta(&p.org, &(10 * XLM), &0);
    let a = con_xlm(&p, 20 * XLM);
    p.tix.pujar(&a, &id, &(12 * XLM));
    assert_eq!(p.tix.cancelar_subasta(&id), 12 * XLM);
    assert_eq!(p.token.balance(&a), 20 * XLM);
    assert_eq!(p.token.balance(&p.org), 0);
}

#[test]
fn comprar_ya_gana_de_inmediato_y_regresa_la_puja() {
    let p = preparar();
    let id = p.tix.crear_subasta(&p.org, &(10 * XLM), &(30 * XLM));
    let a = con_xlm(&p, 20 * XLM);
    let b = con_xlm(&p, 30 * XLM);
    p.tix.pujar(&a, &id, &(12 * XLM));
    assert_eq!(p.tix.comprar_ya(&b, &id), 30 * XLM);
    assert_eq!(p.token.balance(&p.org), 30 * XLM);
    assert_eq!(p.token.balance(&a), 20 * XLM);
    assert_eq!(p.tix.consultar_subasta(&id).ganador, Some(b));
    let sin = p.tix.crear_subasta(&p.org, &XLM, &0);
    assert_eq!(p.tix.try_comprar_ya(&a, &sin), Err(Ok(Error::SinCompraYa)));
}

#[test]
fn comprar_numero_cobra_el_precio_de_la_rifa() {
    let p = preparar();
    let rifa = p.tix.crear_rifa(&p.org, &(2 * XLM));
    let fan = con_xlm(&p, 4 * XLM);
    assert_eq!(p.tix.comprar_numero(&fan, &rifa), 0);
    assert_eq!(p.tix.comprar_numero(&fan, &rifa), 1);
    assert_eq!(p.token.balance(&fan), 0);
    assert_eq!(p.token.balance(&p.tix.address), 4 * XLM);
}

#[test]
fn sortear_rifa_paga_la_mitad_al_ganador() {
    let p = preparar();
    let rifa = p.tix.crear_rifa(&p.org, &(2 * XLM));
    let a = con_xlm(&p, 2 * XLM);
    let b = con_xlm(&p, 2 * XLM);
    p.tix.comprar_numero(&a, &rifa);
    p.tix.comprar_numero(&b, &rifa);
    let ganador = p.tix.sortear_rifa(&rifa);
    assert!(ganador == a || ganador == b);
    assert_eq!(p.token.balance(&ganador), 2 * XLM);
    assert_eq!(p.token.balance(&p.org), 2 * XLM);
    let vacia = p.tix.crear_rifa(&p.org, &XLM);
    assert_eq!(
        p.tix.try_sortear_rifa(&vacia),
        Err(Ok(Error::SinParticipantes))
    );
}

#[test]
fn cancelar_rifa_reembolsa_cada_numero() {
    let p = preparar();
    let rifa = p.tix.crear_rifa(&p.org, &(2 * XLM));
    let a = con_xlm(&p, 2 * XLM);
    let b = con_xlm(&p, 2 * XLM);
    p.tix.comprar_numero(&a, &rifa);
    p.tix.comprar_numero(&b, &rifa);
    assert_eq!(p.tix.cancelar_rifa(&rifa), 4 * XLM);
    assert_eq!(p.token.balance(&a), 2 * XLM);
    assert_eq!(p.token.balance(&b), 2 * XLM);
}

#[test]
fn apoyar_campana_deja_el_dinero_en_garantia() {
    let p = preparar();
    let campana = p.tix.crear_campana(&p.org, &(10 * XLM), &3_600);
    let fan = con_xlm(&p, 6 * XLM);
    assert_eq!(p.tix.apoyar_campana(&fan, &campana, &(6 * XLM)), 6 * XLM);
    assert_eq!(p.token.balance(&p.tix.address), 6 * XLM);
    p.env
        .ledger()
        .set_timestamp(p.env.ledger().timestamp() + 3_601);
    assert_eq!(
        p.tix.try_apoyar_campana(&fan, &campana, &XLM),
        Err(Ok(Error::CampanaTerminada))
    );
}

#[test]
fn cobrar_campana_solo_si_llego_a_la_meta() {
    let p = preparar();
    let campana = p.tix.crear_campana(&p.org, &(10 * XLM), &3_600);
    let fan = con_xlm(&p, 10 * XLM);
    p.tix.apoyar_campana(&fan, &campana, &(4 * XLM));
    assert_eq!(
        p.tix.try_cobrar_campana(&campana),
        Err(Ok(Error::MetaNoAlcanzada))
    );
    p.tix.apoyar_campana(&fan, &campana, &(6 * XLM));
    assert_eq!(p.tix.cobrar_campana(&campana), 10 * XLM);
    assert_eq!(p.token.balance(&p.org), 10 * XLM);
}

#[test]
fn recuperar_apoyo_si_no_se_llego_a_la_meta() {
    let p = preparar();
    let campana = p.tix.crear_campana(&p.org, &(10 * XLM), &3_600);
    let fan = con_xlm(&p, 4 * XLM);
    p.tix.apoyar_campana(&fan, &campana, &(4 * XLM));
    assert_eq!(
        p.tix.try_recuperar_apoyo(&fan, &campana),
        Err(Ok(Error::CampanaVigente))
    );
    p.env
        .ledger()
        .set_timestamp(p.env.ledger().timestamp() + 3_601);
    assert_eq!(p.tix.recuperar_apoyo(&fan, &campana), 4 * XLM);
    assert_eq!(p.token.balance(&fan), 4 * XLM);
}

#[test]
fn reservar_con_garantia_deja_el_deposito() {
    let p = preparar();
    let evento = p.tix.crear_evento_gratis(&p.org, &(3 * XLM));
    let fan = con_xlm(&p, 3 * XLM);
    assert_eq!(p.tix.reservar_con_garantia(&fan, &evento), 3 * XLM);
    assert_eq!(p.token.balance(&fan), 0);
    assert_eq!(
        p.tix.try_reservar_con_garantia(&fan, &evento),
        Err(Ok(Error::YaReservado))
    );
}

#[test]
fn confirmar_asistencia_regresa_la_garantia() {
    let p = preparar();
    let evento = p.tix.crear_evento_gratis(&p.org, &(3 * XLM));
    let fan = con_xlm(&p, 3 * XLM);
    p.tix.reservar_con_garantia(&fan, &evento);
    assert_eq!(p.tix.confirmar_asistencia(&evento, &fan), 3 * XLM);
    assert_eq!(p.env.auths()[0].0, p.org);
    assert_eq!(p.token.balance(&fan), 3 * XLM);
}

#[test]
fn penalizar_inasistencia_da_la_garantia_al_organizador() {
    let p = preparar();
    let evento = p.tix.crear_evento_gratis(&p.org, &(3 * XLM));
    let fan = con_xlm(&p, 3 * XLM);
    p.tix.reservar_con_garantia(&fan, &evento);
    assert_eq!(p.tix.penalizar_inasistencia(&evento, &fan), 3 * XLM);
    assert_eq!(p.token.balance(&p.org), 3 * XLM);
    assert_eq!(
        p.tix.try_confirmar_asistencia(&evento, &fan),
        Err(Ok(Error::SinReserva))
    );
}
