#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Env, Vec};

const XLM: i128 = 10_000_000;

struct Prueba<'a> {
    env: Env,
    tix: ArtistasClient<'a>,
    token: TokenClient<'a>,
    org: Address,
    banda: Address,
}

fn preparar<'a>() -> Prueba<'a> {
    let env = Env::default();
    env.mock_all_auths();
    let emisor = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(emisor).address();
    let id = env.register(Artistas, (token_id.clone(),));
    let org = Address::generate(&env);
    StellarAssetClient::new(&env, &token_id).mint(&org, &(100 * XLM));
    Prueba {
        tix: ArtistasClient::new(&env, &id),
        token: TokenClient::new(&env, &token_id),
        banda: Address::generate(&env),
        env,
        org,
    }
}

fn con_xlm(p: &Prueba, monto: i128) -> Address {
    let cuenta = Address::generate(&p.env);
    StellarAssetClient::new(&p.env, &p.token.address).mint(&cuenta, &monto);
    cuenta
}

#[test]
fn contratar_artista_deja_el_pago_en_garantia() {
    let p = preparar();
    let trato = p.tix.contratar_artista(&p.org, &p.banda, &(30 * XLM));
    assert_eq!(p.env.auths()[0].0, p.org);
    assert_eq!(p.token.balance(&p.tix.address), 30 * XLM);
    assert_eq!(p.tix.consultar_trato(&trato).deposito, 30 * XLM);
    assert_eq!(
        p.tix.try_contratar_artista(&p.org, &p.org, &XLM),
        Err(Ok(Error::MismaCuenta))
    );
}

#[test]
fn aumentar_pago_suma_a_la_garantia() {
    let p = preparar();
    let trato = p.tix.contratar_artista(&p.org, &p.banda, &(30 * XLM));
    assert_eq!(p.tix.aumentar_pago(&trato, &(10 * XLM)), 40 * XLM);
    assert_eq!(p.token.balance(&p.tix.address), 40 * XLM);
    assert_eq!(
        p.tix.try_aumentar_pago(&trato, &0),
        Err(Ok(Error::MontoInvalido))
    );
}

#[test]
fn pagar_anticipo_libera_una_parte_al_artista() {
    let p = preparar();
    let trato = p.tix.contratar_artista(&p.org, &p.banda, &(30 * XLM));
    assert_eq!(p.tix.pagar_anticipo(&trato, &(10 * XLM)), 20 * XLM);
    assert_eq!(p.token.balance(&p.banda), 10 * XLM);
    assert_eq!(
        p.tix.try_pagar_anticipo(&trato, &(21 * XLM)),
        Err(Ok(Error::SinFondos))
    );
}

#[test]
fn liquidar_artista_paga_el_resto_y_cierra() {
    let p = preparar();
    let trato = p.tix.contratar_artista(&p.org, &p.banda, &(30 * XLM));
    p.tix.pagar_anticipo(&trato, &(10 * XLM));
    assert_eq!(p.tix.liquidar_artista(&trato), 20 * XLM);
    assert_eq!(p.token.balance(&p.banda), 30 * XLM);
    assert!(p.tix.consultar_trato(&trato).cerrado);
    assert_eq!(p.tix.try_liquidar_artista(&trato), Err(Ok(Error::Cerrado)));
}

#[test]
fn liquidar_varios_paga_a_todo_el_cartel() {
    let p = preparar();
    let dj = Address::generate(&p.env);
    let a = p.tix.contratar_artista(&p.org, &p.banda, &(20 * XLM));
    let b = p.tix.contratar_artista(&p.org, &dj, &(10 * XLM));
    let tratos = Vec::from_array(&p.env, [a, b]);
    assert_eq!(p.tix.liquidar_varios(&p.org, &tratos), 30 * XLM);
    assert_eq!(p.token.balance(&p.banda), 20 * XLM);
    assert_eq!(p.token.balance(&dj), 10 * XLM);
    assert_eq!(
        p.tix.try_liquidar_varios(&p.org, &tratos),
        Err(Ok(Error::Cerrado))
    );
}

#[test]
fn renunciar_artista_regresa_la_garantia_al_organizador() {
    let p = preparar();
    let trato = p.tix.contratar_artista(&p.org, &p.banda, &(30 * XLM));
    assert_eq!(p.tix.renunciar_artista(&trato), 30 * XLM);
    assert_eq!(p.env.auths()[0].0, p.banda);
    assert_eq!(p.token.balance(&p.org), 100 * XLM);
    assert_eq!(p.token.balance(&p.banda), 0);
}

#[test]
fn dar_bono_paga_directo_y_suma_a_lo_pagado() {
    let p = preparar();
    let trato = p.tix.contratar_artista(&p.org, &p.banda, &(30 * XLM));
    p.tix.liquidar_artista(&trato);
    assert_eq!(p.tix.dar_bono(&trato, &(5 * XLM)), 35 * XLM);
    assert_eq!(p.token.balance(&p.banda), 35 * XLM);
    assert_eq!(
        p.tix.try_dar_bono(&trato, &0),
        Err(Ok(Error::MontoInvalido))
    );
}

#[test]
fn pagar_artistas_paga_a_cada_uno_su_monto() {
    let p = preparar();
    let dj = Address::generate(&p.env);
    let artistas = Vec::from_array(&p.env, [p.banda.clone(), dj.clone()]);
    let montos = Vec::from_array(&p.env, [7 * XLM, 3 * XLM]);
    assert_eq!(p.tix.pagar_artistas(&p.org, &artistas, &montos), 10 * XLM);
    assert_eq!(p.token.balance(&dj), 3 * XLM);
    assert_eq!(
        p.tix
            .try_pagar_artistas(&p.org, &artistas, &Vec::from_array(&p.env, [XLM])),
        Err(Ok(Error::CantidadInvalida))
    );
}

#[test]
fn patrocinar_deja_el_dinero_en_garantia() {
    let p = preparar();
    let marca = con_xlm(&p, 50 * XLM);
    let id = p.tix.patrocinar(&marca, &p.org, &(40 * XLM));
    assert_eq!(p.env.auths()[0].0, marca);
    assert_eq!(p.tix.consultar_patrocinio(&id).saldo, 40 * XLM);
    assert_eq!(p.token.balance(&marca), 10 * XLM);
}

#[test]
fn aumentar_patrocinio_suma_al_saldo() {
    let p = preparar();
    let marca = con_xlm(&p, 50 * XLM);
    let id = p.tix.patrocinar(&marca, &p.org, &(20 * XLM));
    assert_eq!(p.tix.aumentar_patrocinio(&id, &(10 * XLM)), 30 * XLM);
    assert_eq!(p.token.balance(&marca), 20 * XLM);
}

#[test]
fn liberar_patrocinio_parcial_paga_una_parte() {
    let p = preparar();
    let marca = con_xlm(&p, 50 * XLM);
    let id = p.tix.patrocinar(&marca, &p.org, &(40 * XLM));
    assert_eq!(p.tix.liberar_patrocinio_parcial(&id, &(15 * XLM)), 25 * XLM);
    assert_eq!(p.token.balance(&p.org), 115 * XLM);
    assert_eq!(
        p.tix.try_liberar_patrocinio_parcial(&id, &(26 * XLM)),
        Err(Ok(Error::SinFondos))
    );
}

#[test]
fn liberar_patrocinio_paga_todo_y_cierra() {
    let p = preparar();
    let marca = con_xlm(&p, 50 * XLM);
    let id = p.tix.patrocinar(&marca, &p.org, &(40 * XLM));
    assert_eq!(p.tix.liberar_patrocinio(&id), 40 * XLM);
    assert_eq!(p.token.balance(&p.org), 140 * XLM);
    assert_eq!(p.tix.try_retirar_patrocinio(&id), Err(Ok(Error::Cerrado)));
}

#[test]
fn retirar_patrocinio_regresa_lo_no_liberado() {
    let p = preparar();
    let marca = con_xlm(&p, 50 * XLM);
    let id = p.tix.patrocinar(&marca, &p.org, &(40 * XLM));
    p.tix.liberar_patrocinio_parcial(&id, &(10 * XLM));
    assert_eq!(p.tix.retirar_patrocinio(&id), 30 * XLM);
    assert_eq!(p.token.balance(&marca), 40 * XLM);
}

#[test]
fn repartir_paga_segun_las_partes() {
    let p = preparar();
    let dj = Address::generate(&p.env);
    let cuentas = Vec::from_array(&p.env, [p.banda.clone(), dj.clone()]);
    let partes = Vec::from_array(&p.env, [7_000u32, 3_000]);
    assert_eq!(
        p.tix.repartir(&p.org, &cuentas, &partes, &(10 * XLM)),
        10 * XLM
    );
    assert_eq!(p.token.balance(&p.banda), 7 * XLM);
    assert_eq!(p.token.balance(&dj), 3 * XLM);
    let malas = Vec::from_array(&p.env, [7_000u32, 2_000]);
    assert_eq!(
        p.tix.try_repartir(&p.org, &cuentas, &malas, &XLM),
        Err(Ok(Error::PartesInvalidas))
    );
}

#[test]
fn repartir_igual_divide_en_partes_iguales() {
    let p = preparar();
    let dj = Address::generate(&p.env);
    let vj = Address::generate(&p.env);
    let cuentas = Vec::from_array(&p.env, [p.banda.clone(), dj.clone(), vj.clone()]);
    assert_eq!(p.tix.repartir_igual(&p.org, &cuentas, &(9 * XLM)), 9 * XLM);
    // 3,334 / 3,333 / 3,333 puntos: lo que sobra va a la primera cuenta.
    assert_eq!(p.token.balance(&dj), 29_997_000);
    assert_eq!(p.token.balance(&p.banda), 9 * XLM - 2 * 29_997_000);
}

#[test]
fn crear_bolsa_abre_el_fondo_de_premios() {
    let p = preparar();
    let bolsa = p.tix.crear_bolsa(&p.org, &(20 * XLM));
    assert_eq!(p.tix.consultar_bolsa(&bolsa).saldo, 20 * XLM);
    assert_eq!(p.token.balance(&p.org), 80 * XLM);
}

#[test]
fn aportar_bolsa_suma_dinero_de_fans() {
    let p = preparar();
    let fan = con_xlm(&p, 5 * XLM);
    let bolsa = p.tix.crear_bolsa(&p.org, &(20 * XLM));
    assert_eq!(p.tix.aportar_bolsa(&fan, &bolsa, &(5 * XLM)), 25 * XLM);
    assert_eq!(p.token.balance(&fan), 0);
}

#[test]
fn pagar_desde_bolsa_entrega_un_premio() {
    let p = preparar();
    let bolsa = p.tix.crear_bolsa(&p.org, &(20 * XLM));
    assert_eq!(
        p.tix.pagar_desde_bolsa(&bolsa, &p.banda, &(8 * XLM)),
        12 * XLM
    );
    assert_eq!(p.token.balance(&p.banda), 8 * XLM);
    assert_eq!(
        p.tix.try_pagar_desde_bolsa(&bolsa, &p.banda, &(13 * XLM)),
        Err(Ok(Error::SinFondos))
    );
}

#[test]
fn repartir_bolsa_divide_entre_ganadores_y_cierra() {
    let p = preparar();
    let dj = Address::generate(&p.env);
    let bolsa = p.tix.crear_bolsa(&p.org, &(20 * XLM));
    let ganadores = Vec::from_array(&p.env, [p.banda.clone(), dj.clone()]);
    assert_eq!(p.tix.repartir_bolsa(&bolsa, &ganadores), 20 * XLM);
    assert_eq!(p.token.balance(&dj), 10 * XLM);
    assert!(p.tix.consultar_bolsa(&bolsa).cerrada);
}

#[test]
fn cerrar_bolsa_regresa_lo_que_sobra() {
    let p = preparar();
    let bolsa = p.tix.crear_bolsa(&p.org, &(20 * XLM));
    p.tix.pagar_desde_bolsa(&bolsa, &p.banda, &(5 * XLM));
    assert_eq!(p.tix.cerrar_bolsa(&bolsa), 15 * XLM);
    assert_eq!(p.token.balance(&p.org), 95 * XLM);
    assert_eq!(p.tix.try_cerrar_bolsa(&bolsa), Err(Ok(Error::Cerrado)));
}
