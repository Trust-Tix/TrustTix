#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Env};

const XLM: i128 = 10_000_000;

struct Prueba<'a> {
    env: Env,
    tix: AbonosClient<'a>,
    token: TokenClient<'a>,
    org: Address,
}

fn preparar<'a>() -> Prueba<'a> {
    let env = Env::default();
    env.mock_all_auths();
    let emisor = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(emisor).address();
    let id = env.register(Abonos, (token_id.clone(),));
    let org = Address::generate(&env);
    Prueba {
        tix: AbonosClient::new(&env, &id),
        token: TokenClient::new(&env, &token_id),
        env,
        org,
    }
}

fn con_xlm(p: &Prueba, monto: i128) -> Address {
    let cuenta = Address::generate(&p.env);
    StellarAssetClient::new(&p.env, &p.token.address).mint(&cuenta, &monto);
    cuenta
}

/// Abono de 30 XLM con 3 entradas.
fn abono(p: &Prueba) -> u32 {
    p.tix.crear_abono(&p.org, &(30 * XLM), &3)
}

#[test]
fn comprar_abono_paga_al_organizador_y_da_las_entradas() {
    let p = preparar();
    let id = abono(&p);
    let fan = con_xlm(&p, 30 * XLM);
    assert_eq!(p.tix.comprar_abono(&fan, &id), 3);
    assert_eq!(p.env.auths()[0].0, fan);
    assert_eq!(p.token.balance(&p.org), 30 * XLM);
    assert_eq!(p.tix.entradas(&id, &fan), 3);
}

#[test]
fn comprar_abono_para_regala_las_entradas() {
    let p = preparar();
    let id = abono(&p);
    let papa = con_xlm(&p, 30 * XLM);
    let hija = Address::generate(&p.env);
    assert_eq!(p.tix.comprar_abono_para(&papa, &hija, &id), 3);
    assert_eq!(p.tix.entradas(&id, &papa), 0);
    assert_eq!(p.token.balance(&papa), 0);
    assert_eq!(
        p.tix.try_comprar_abono_para(&papa, &papa, &id),
        Err(Ok(Error::MismaCuenta))
    );
}

#[test]
fn renovar_abono_suma_entradas_a_quien_ya_lo_tiene() {
    let p = preparar();
    let id = abono(&p);
    let fan = con_xlm(&p, 60 * XLM);
    p.tix.comprar_abono(&fan, &id);
    assert_eq!(p.tix.renovar_abono(&fan, &id), 6);
    assert_eq!(p.token.balance(&p.org), 60 * XLM);
    let nuevo = con_xlm(&p, 30 * XLM);
    assert_eq!(
        p.tix.try_renovar_abono(&nuevo, &id),
        Err(Ok(Error::SinAbono))
    );
}

#[test]
fn usar_abono_descuenta_una_entrada_en_la_puerta() {
    let p = preparar();
    let id = abono(&p);
    let fan = con_xlm(&p, 30 * XLM);
    p.tix.comprar_abono(&fan, &id);
    assert_eq!(p.tix.usar_abono(&id, &fan), 2);
    assert_eq!(p.env.auths()[0].0, p.org);
    p.tix.usar_abono(&id, &fan);
    p.tix.usar_abono(&id, &fan);
    assert_eq!(p.tix.try_usar_abono(&id, &fan), Err(Ok(Error::SinUsos)));
}

#[test]
fn transferir_abono_pasa_las_entradas_que_quedan() {
    let p = preparar();
    let id = abono(&p);
    let fan = con_xlm(&p, 30 * XLM);
    let amigo = Address::generate(&p.env);
    p.tix.comprar_abono(&fan, &id);
    p.tix.usar_abono(&id, &fan);
    assert_eq!(p.tix.transferir_abono(&fan, &amigo, &id), 2);
    assert_eq!(p.tix.entradas(&id, &fan), 0);
    assert_eq!(
        p.tix.try_transferir_abono(&fan, &amigo, &id),
        Err(Ok(Error::SinAbono))
    );
}

#[test]
fn reembolsar_abono_devuelve_la_parte_no_usada() {
    let p = preparar();
    let id = abono(&p);
    let fan = con_xlm(&p, 30 * XLM);
    p.tix.comprar_abono(&fan, &id);
    p.tix.usar_abono(&id, &fan);
    // Quedan 2 de 3 entradas: se devuelven 20 de 30 XLM.
    assert_eq!(p.tix.reembolsar_abono(&id, &fan), 20 * XLM);
    assert_eq!(p.token.balance(&fan), 20 * XLM);
    assert_eq!(p.tix.entradas(&id, &fan), 0);
}

#[test]
fn iniciar_plan_cobra_la_primera_cuota() {
    let p = preparar();
    let fan = con_xlm(&p, 30 * XLM);
    let plan = p.tix.iniciar_plan(&fan, &p.org, &(30 * XLM), &3);
    assert_eq!(p.token.balance(&p.org), 10 * XLM);
    assert_eq!(p.tix.consultar_plan(&plan).pagado, 10 * XLM);
    assert_eq!(
        p.tix.try_iniciar_plan(&fan, &p.org, &(30 * XLM), &1),
        Err(Ok(Error::CuotasInvalidas))
    );
}

#[test]
fn pagar_cuota_avanza_el_plan_hasta_completarlo() {
    let p = preparar();
    let fan = con_xlm(&p, 30 * XLM);
    let plan = p.tix.iniciar_plan(&fan, &p.org, &(30 * XLM), &3);
    assert_eq!(p.tix.pagar_cuota(&plan), 10 * XLM);
    assert_eq!(p.tix.pagar_cuota(&plan), 0);
    assert_eq!(p.tix.try_pagar_cuota(&plan), Err(Ok(Error::PlanCompleto)));
}

#[test]
fn pagar_cuota_por_otra_persona() {
    let p = preparar();
    let fan = con_xlm(&p, 10 * XLM);
    let mama = con_xlm(&p, 10 * XLM);
    let plan = p.tix.iniciar_plan(&fan, &p.org, &(20 * XLM), &2);
    assert_eq!(p.tix.pagar_cuota_por(&mama, &plan), 0);
    assert_eq!(p.env.auths()[0].0, mama);
    assert_eq!(p.token.balance(&mama), 0);
    assert_eq!(p.token.balance(&p.org), 20 * XLM);
}

#[test]
fn pagar_plan_completo_liquida_lo_que_falta() {
    let p = preparar();
    let fan = con_xlm(&p, 40 * XLM);
    let plan = p.tix.iniciar_plan(&fan, &p.org, &(40 * XLM), &4);
    assert_eq!(p.tix.pagar_plan_completo(&plan), 30 * XLM);
    assert_eq!(p.token.balance(&p.org), 40 * XLM);
    assert_eq!(
        p.tix.try_pagar_plan_completo(&plan),
        Err(Ok(Error::PlanCompleto))
    );
}

#[test]
fn cancelar_plan_devuelve_lo_pagado() {
    let p = preparar();
    let fan = con_xlm(&p, 30 * XLM);
    let plan = p.tix.iniciar_plan(&fan, &p.org, &(30 * XLM), &3);
    p.tix.pagar_cuota(&plan);
    assert_eq!(p.tix.cancelar_plan(&plan), 20 * XLM);
    assert_eq!(p.env.auths()[0].0, p.org);
    assert_eq!(p.token.balance(&fan), 30 * XLM);
    assert_eq!(p.tix.try_pagar_cuota(&plan), Err(Ok(Error::PlanCompleto)));
}

#[test]
fn abrir_alcancia_guarda_el_primer_deposito() {
    let p = preparar();
    let fan = con_xlm(&p, 10 * XLM);
    let id = p.tix.abrir_alcancia(&fan, &(50 * XLM), &(4 * XLM));
    assert_eq!(p.tix.consultar_alcancia(&id).saldo, 4 * XLM);
    assert_eq!(p.token.balance(&p.tix.address), 4 * XLM);
}

#[test]
fn ahorrar_suma_a_la_alcancia() {
    let p = preparar();
    let fan = con_xlm(&p, 10 * XLM);
    let id = p.tix.abrir_alcancia(&fan, &(50 * XLM), &(4 * XLM));
    assert_eq!(p.tix.ahorrar(&id, &(6 * XLM)), 10 * XLM);
    assert_eq!(p.tix.try_ahorrar(&id, &0), Err(Ok(Error::MontoInvalido)));
}

#[test]
fn ahorrar_para_deposita_en_la_alcancia_de_otro() {
    let p = preparar();
    let fan = con_xlm(&p, 10 * XLM);
    let abuela = con_xlm(&p, 20 * XLM);
    let id = p.tix.abrir_alcancia(&fan, &(50 * XLM), &(4 * XLM));
    assert_eq!(p.tix.ahorrar_para(&abuela, &id, &(20 * XLM)), 24 * XLM);
    assert_eq!(p.env.auths()[0].0, abuela);
}

#[test]
fn retirar_alcancia_regresa_todo_al_dueno() {
    let p = preparar();
    let fan = con_xlm(&p, 10 * XLM);
    let id = p.tix.abrir_alcancia(&fan, &(50 * XLM), &(10 * XLM));
    assert_eq!(p.tix.retirar_alcancia(&id), 10 * XLM);
    assert_eq!(p.token.balance(&fan), 10 * XLM);
    assert_eq!(p.tix.try_retirar_alcancia(&id), Err(Ok(Error::SinFondos)));
}

#[test]
fn pagar_con_alcancia_paga_al_organizador() {
    let p = preparar();
    let fan = con_xlm(&p, 10 * XLM);
    let id = p.tix.abrir_alcancia(&fan, &(50 * XLM), &(10 * XLM));
    assert_eq!(p.tix.pagar_con_alcancia(&id, &p.org, &(7 * XLM)), 3 * XLM);
    assert_eq!(p.token.balance(&p.org), 7 * XLM);
    assert_eq!(
        p.tix.try_pagar_con_alcancia(&id, &p.org, &(4 * XLM)),
        Err(Ok(Error::SinFondos))
    );
}

#[test]
fn unirse_club_cobra_la_cuota_y_da_30_dias() {
    let p = preparar();
    let club = p.tix.crear_club(&p.org, &(2 * XLM));
    let fan = con_xlm(&p, 2 * XLM);
    p.env.ledger().set_timestamp(1_000);
    assert_eq!(p.tix.unirse_club(&fan, &club), 1_000 + MES);
    assert_eq!(p.token.balance(&p.org), 2 * XLM);
}

#[test]
fn renovar_membresia_suma_30_dias_mas() {
    let p = preparar();
    let club = p.tix.crear_club(&p.org, &(2 * XLM));
    let fan = con_xlm(&p, 4 * XLM);
    p.env.ledger().set_timestamp(1_000);
    p.tix.unirse_club(&fan, &club);
    assert_eq!(p.tix.renovar_membresia(&fan, &club), 1_000 + 2 * MES);
    let nuevo = con_xlm(&p, 2 * XLM);
    assert_eq!(
        p.tix.try_renovar_membresia(&nuevo, &club),
        Err(Ok(Error::NoExiste))
    );
}

#[test]
fn regalar_membresia_paga_uno_y_el_mes_es_del_otro() {
    let p = preparar();
    let club = p.tix.crear_club(&p.org, &(2 * XLM));
    let amigo = con_xlm(&p, 2 * XLM);
    let fan = Address::generate(&p.env);
    p.env.ledger().set_timestamp(1_000);
    assert_eq!(p.tix.regalar_membresia(&amigo, &fan, &club), 1_000 + MES);
    assert_eq!(p.tix.membresia_vence(&club, &fan), 1_000 + MES);
    assert_eq!(p.tix.membresia_vence(&club, &amigo), 0);
    assert_eq!(p.token.balance(&amigo), 0);
}
