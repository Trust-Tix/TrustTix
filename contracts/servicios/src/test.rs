#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Env, Vec};

const XLM: i128 = 10_000_000;

struct Prueba<'a> {
    env: Env,
    tix: ServiciosClient<'a>,
    token: TokenClient<'a>,
    org: Address,
}

fn preparar<'a>() -> Prueba<'a> {
    let env = Env::default();
    env.mock_all_auths();
    let emisor = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(emisor).address();
    let id = env.register(Servicios, (token_id.clone(),));
    Prueba {
        tix: ServiciosClient::new(&env, &id),
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
fn preordenar_deja_el_pago_en_garantia_y_baja_existencias() {
    let p = preparar();
    let playera = p.tix.crear_producto(&p.org, &(5 * XLM), &1);
    let fan = con_xlm(&p, 10 * XLM);
    let pedido = p.tix.preordenar(&fan, &playera);
    assert_eq!(p.token.balance(&p.tix.address), 5 * XLM);
    assert_eq!(p.tix.consultar_pedido(&pedido).estado, 0);
    assert_eq!(
        p.tix.try_preordenar(&fan, &playera),
        Err(Ok(Error::Agotado))
    );
}

#[test]
fn confirmar_entrega_libera_el_pago_al_vendedor() {
    let p = preparar();
    let playera = p.tix.crear_producto(&p.org, &(5 * XLM), &3);
    let fan = con_xlm(&p, 5 * XLM);
    let pedido = p.tix.preordenar(&fan, &playera);
    assert_eq!(p.tix.confirmar_entrega(&pedido), 5 * XLM);
    assert_eq!(p.env.auths()[0].0, fan);
    assert_eq!(p.token.balance(&p.org), 5 * XLM);
    assert_eq!(
        p.tix.try_cancelar_pedido(&pedido),
        Err(Ok(Error::NoPendiente))
    );
}

#[test]
fn cancelar_pedido_regresa_el_dinero_y_la_pieza() {
    let p = preparar();
    let playera = p.tix.crear_producto(&p.org, &(5 * XLM), &1);
    let fan = con_xlm(&p, 5 * XLM);
    let pedido = p.tix.cancelar_pedido(&p.tix.preordenar(&fan, &playera));
    assert_eq!(pedido, 5 * XLM);
    assert_eq!(p.token.balance(&fan), 5 * XLM);
    // La pieza vuelve a estar disponible.
    let otro = con_xlm(&p, 5 * XLM);
    p.tix.preordenar(&otro, &playera);
}

#[test]
fn entrar_estacionamiento_deja_el_maximo_en_garantia() {
    let p = preparar();
    let est = p.tix.crear_estacionamiento(&p.org, &XLM, &5);
    let fan = con_xlm(&p, 5 * XLM);
    assert_eq!(p.tix.entrar_estacionamiento(&fan, &est), 5 * XLM);
    assert_eq!(p.token.balance(&fan), 0);
    assert_eq!(
        p.tix.try_entrar_estacionamiento(&fan, &est),
        Err(Ok(Error::YaDentro))
    );
}

#[test]
fn salir_estacionamiento_cobra_solo_las_horas_usadas() {
    let p = preparar();
    let est = p.tix.crear_estacionamiento(&p.org, &XLM, &5);
    let fan = con_xlm(&p, 5 * XLM);
    p.env.ledger().set_timestamp(1_000);
    p.tix.entrar_estacionamiento(&fan, &est);
    // Dos horas y un minuto: se cobran 3 horas.
    p.env.ledger().set_timestamp(1_000 + 2 * HORA + 60);
    assert_eq!(p.tix.salir_estacionamiento(&fan, &est), 3 * XLM);
    assert_eq!(p.token.balance(&p.org), 3 * XLM);
    assert_eq!(p.token.balance(&fan), 2 * XLM);
    assert_eq!(
        p.tix.try_salir_estacionamiento(&fan, &est),
        Err(Ok(Error::NoDentro))
    );
}

#[test]
fn apartar_asiento_deja_el_pago_en_garantia() {
    let p = preparar();
    let viaje = p.tix.ofrecer_viaje(&p.org, &(2 * XLM), &1);
    let a = con_xlm(&p, 2 * XLM);
    let b = con_xlm(&p, 2 * XLM);
    assert_eq!(p.tix.apartar_asiento(&a, &viaje), 0);
    assert_eq!(p.token.balance(&p.tix.address), 2 * XLM);
    assert_eq!(
        p.tix.try_apartar_asiento(&b, &viaje),
        Err(Ok(Error::SinAsiento))
    );
}

#[test]
fn confirmar_llegada_paga_al_conductor_una_sola_vez() {
    let p = preparar();
    let viaje = p.tix.ofrecer_viaje(&p.org, &(2 * XLM), &3);
    let a = con_xlm(&p, 2 * XLM);
    p.tix.apartar_asiento(&a, &viaje);
    assert_eq!(p.tix.confirmar_llegada(&a, &viaje), 2 * XLM);
    assert_eq!(p.token.balance(&p.org), 2 * XLM);
    assert_eq!(
        p.tix.try_confirmar_llegada(&a, &viaje),
        Err(Ok(Error::YaPagado))
    );
}

#[test]
fn cancelar_viaje_reembolsa_a_quien_no_ha_llegado() {
    let p = preparar();
    let viaje = p.tix.ofrecer_viaje(&p.org, &(2 * XLM), &3);
    let a = con_xlm(&p, 2 * XLM);
    let b = con_xlm(&p, 2 * XLM);
    p.tix.apartar_asiento(&a, &viaje);
    p.tix.apartar_asiento(&b, &viaje);
    p.tix.confirmar_llegada(&a, &viaje);
    assert_eq!(p.tix.cancelar_viaje(&viaje), 2 * XLM);
    assert_eq!(p.token.balance(&b), 2 * XLM);
    assert_eq!(p.token.balance(&a), 0);
}

#[test]
fn votar_con_xlm_suma_votos_a_la_opcion() {
    let p = preparar();
    let banda = Address::generate(&p.env);
    let dj = Address::generate(&p.env);
    let votacion = p
        .tix
        .crear_votacion(&p.org, &Vec::from_array(&p.env, [banda, dj]));
    let fan = con_xlm(&p, 5 * XLM);
    assert_eq!(
        p.tix.votar_con_xlm(&fan, &votacion, &1, &(3 * XLM)),
        3 * XLM
    );
    assert_eq!(p.token.balance(&p.tix.address), 3 * XLM);
    assert_eq!(
        p.tix.try_votar_con_xlm(&fan, &votacion, &2, &XLM),
        Err(Ok(Error::OpcionInvalida))
    );
}

#[test]
fn premiar_ganador_entrega_toda_la_bolsa() {
    let p = preparar();
    let banda = Address::generate(&p.env);
    let dj = Address::generate(&p.env);
    let votacion = p.tix.crear_votacion(
        &p.org,
        &Vec::from_array(&p.env, [banda.clone(), dj.clone()]),
    );
    let a = con_xlm(&p, 5 * XLM);
    let b = con_xlm(&p, 5 * XLM);
    p.tix.votar_con_xlm(&a, &votacion, &0, &(2 * XLM));
    p.tix.votar_con_xlm(&b, &votacion, &1, &(4 * XLM));
    assert_eq!(p.tix.premiar_ganador(&votacion), dj);
    assert_eq!(p.token.balance(&dj), 6 * XLM);
    assert_eq!(
        p.tix.try_premiar_ganador(&votacion),
        Err(Ok(Error::Cerrado))
    );
}
