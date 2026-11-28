#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Env, Vec};

const XLM: i128 = 10_000_000;

struct Prueba<'a> {
    env: Env,
    tix: SegurosClient<'a>,
    token: TokenClient<'a>,
    org: Address,
}

fn preparar<'a>() -> Prueba<'a> {
    let env = Env::default();
    env.mock_all_auths();
    let emisor = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(emisor).address();
    let id = env.register(Seguros, (token_id.clone(),));
    let org = Address::generate(&env);
    StellarAssetClient::new(&env, &token_id).mint(&org, &(100 * XLM));
    Prueba {
        tix: SegurosClient::new(&env, &id),
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

/// Fondo de 50 XLM.
fn fondo(p: &Prueba) -> u32 {
    p.tix.fondear_seguro(&p.org, &(50 * XLM))
}

#[test]
fn fondear_seguro_deposita_la_reserva() {
    let p = preparar();
    let id = fondo(&p);
    assert_eq!(p.env.auths()[0].0, p.org);
    assert_eq!(p.tix.consultar_fondo(&id).saldo, 50 * XLM);
    assert_eq!(p.token.balance(&p.tix.address), 50 * XLM);
}

#[test]
fn aumentar_fondo_suma_reserva() {
    let p = preparar();
    let id = fondo(&p);
    assert_eq!(p.tix.aumentar_fondo(&id, &(10 * XLM)), 60 * XLM);
    assert_eq!(p.token.balance(&p.org), 40 * XLM);
}

#[test]
fn contratar_seguro_cobra_el_diez_por_ciento() {
    let p = preparar();
    let id = fondo(&p);
    let fan = con_xlm(&p, 2 * XLM);
    let poliza = p.tix.contratar_seguro(&fan, &id, &(20 * XLM));
    assert_eq!(p.token.balance(&fan), 0);
    let datos = p.tix.consultar_fondo(&id);
    assert_eq!(datos.saldo, 52 * XLM);
    assert_eq!(datos.comprometido, 20 * XLM);
    assert!(p.tix.consultar_poliza(&poliza).activa);
    // El fondo no respalda una cobertura mayor a lo libre.
    let otro = con_xlm(&p, 10 * XLM);
    assert_eq!(
        p.tix.try_contratar_seguro(&otro, &id, &(40 * XLM)),
        Err(Ok(Error::SinCobertura))
    );
}

#[test]
fn contratar_seguro_para_otra_persona() {
    let p = preparar();
    let id = fondo(&p);
    let mama = con_xlm(&p, XLM);
    let fan = Address::generate(&p.env);
    let poliza = p.tix.contratar_seguro_para(&mama, &fan, &id, &(10 * XLM));
    assert_eq!(p.tix.consultar_poliza(&poliza).asegurado, fan);
    assert_eq!(p.token.balance(&mama), 0);
}

#[test]
fn contratar_seguro_varios_asegura_al_grupo() {
    let p = preparar();
    let id = fondo(&p);
    let quien_paga = con_xlm(&p, 3 * XLM);
    let a = Address::generate(&p.env);
    let b = Address::generate(&p.env);
    let c = Address::generate(&p.env);
    let polizas = p.tix.contratar_seguro_varios(
        &quien_paga,
        &Vec::from_array(&p.env, [a, b, c]),
        &id,
        &(10 * XLM),
    );
    assert_eq!(polizas.len(), 3);
    assert_eq!(p.token.balance(&quien_paga), 0);
    assert_eq!(p.tix.consultar_fondo(&id).comprometido, 30 * XLM);
}

#[test]
fn pagar_siniestro_paga_la_cobertura_completa() {
    let p = preparar();
    let id = fondo(&p);
    let fan = con_xlm(&p, 2 * XLM);
    let poliza = p.tix.contratar_seguro(&fan, &id, &(20 * XLM));
    assert_eq!(p.tix.pagar_siniestro(&poliza), 20 * XLM);
    assert_eq!(p.token.balance(&fan), 20 * XLM);
    assert_eq!(p.tix.consultar_fondo(&id).comprometido, 0);
    assert_eq!(p.tix.try_pagar_siniestro(&poliza), Err(Ok(Error::Inactiva)));
}

#[test]
fn pagar_siniestros_paga_varias_polizas() {
    let p = preparar();
    let id = fondo(&p);
    let a = con_xlm(&p, XLM);
    let b = con_xlm(&p, XLM);
    let pa = p.tix.contratar_seguro(&a, &id, &(10 * XLM));
    let pb = p.tix.contratar_seguro(&b, &id, &(10 * XLM));
    assert_eq!(
        p.tix
            .pagar_siniestros(&id, &Vec::from_array(&p.env, [pa, pb])),
        20 * XLM
    );
    assert_eq!(p.token.balance(&a), 10 * XLM);
    assert_eq!(p.token.balance(&b), 10 * XLM);
}

#[test]
fn cancelar_poliza_devuelve_la_mitad_de_la_prima() {
    let p = preparar();
    let id = fondo(&p);
    let fan = con_xlm(&p, 2 * XLM);
    let poliza = p
        .tix
        .cancelar_poliza(&p.tix.contratar_seguro(&fan, &id, &(20 * XLM)));
    assert_eq!(poliza, XLM);
    assert_eq!(p.token.balance(&fan), XLM);
    assert_eq!(p.tix.consultar_fondo(&id).comprometido, 0);
}

#[test]
fn transferir_poliza_cambia_al_asegurado() {
    let p = preparar();
    let id = fondo(&p);
    let fan = con_xlm(&p, 2 * XLM);
    let amiga = Address::generate(&p.env);
    let poliza = p.tix.contratar_seguro(&fan, &id, &(20 * XLM));
    p.tix.transferir_poliza(&poliza, &amiga);
    assert_eq!(p.env.auths()[0].0, fan);
    p.tix.pagar_siniestro(&poliza);
    assert_eq!(p.token.balance(&amiga), 20 * XLM);
}

#[test]
fn retirar_fondo_solo_lo_no_comprometido() {
    let p = preparar();
    let id = fondo(&p);
    let fan = con_xlm(&p, 2 * XLM);
    p.tix.contratar_seguro(&fan, &id, &(20 * XLM));
    assert_eq!(p.tix.retirar_fondo(&id, &(30 * XLM)), 22 * XLM);
    assert_eq!(
        p.tix.try_retirar_fondo(&id, &(3 * XLM)),
        Err(Ok(Error::Comprometido))
    );
}

#[test]
fn cerrar_fondo_sin_polizas_activas_regresa_todo() {
    let p = preparar();
    let id = fondo(&p);
    let fan = con_xlm(&p, 2 * XLM);
    let poliza = p.tix.contratar_seguro(&fan, &id, &(20 * XLM));
    assert_eq!(p.tix.try_cerrar_fondo(&id), Err(Ok(Error::Comprometido)));
    p.tix.cancelar_poliza(&poliza);
    assert_eq!(p.tix.cerrar_fondo(&id), 51 * XLM);
    assert_eq!(p.token.balance(&p.org), 101 * XLM);
}

#[test]
fn dejar_deposito_guarda_la_garantia() {
    let p = preparar();
    let banda = con_xlm(&p, 10 * XLM);
    let id = p.tix.dejar_deposito(&banda, &p.org, &(10 * XLM));
    assert_eq!(p.token.balance(&p.tix.address), 10 * XLM);
    assert_eq!(p.tix.consultar_deposito(&id).monto, 10 * XLM);
}

#[test]
fn aumentar_deposito_suma_a_la_garantia() {
    let p = preparar();
    let banda = con_xlm(&p, 10 * XLM);
    let id = p.tix.dejar_deposito(&banda, &p.org, &(6 * XLM));
    assert_eq!(p.tix.aumentar_deposito(&id, &(4 * XLM)), 10 * XLM);
}

#[test]
fn devolver_deposito_regresa_todo() {
    let p = preparar();
    let banda = con_xlm(&p, 10 * XLM);
    let id = p.tix.dejar_deposito(&banda, &p.org, &(10 * XLM));
    assert_eq!(p.tix.devolver_deposito(&id), 10 * XLM);
    assert_eq!(p.env.auths()[0].0, p.org);
    assert_eq!(p.token.balance(&banda), 10 * XLM);
    assert_eq!(p.tix.try_devolver_deposito(&id), Err(Ok(Error::Cerrado)));
}

#[test]
fn devolver_depositos_regresa_varios() {
    let p = preparar();
    let a = con_xlm(&p, 5 * XLM);
    let b = con_xlm(&p, 5 * XLM);
    let da = p.tix.dejar_deposito(&a, &p.org, &(5 * XLM));
    let db = p.tix.dejar_deposito(&b, &p.org, &(5 * XLM));
    assert_eq!(
        p.tix
            .devolver_depositos(&p.org, &Vec::from_array(&p.env, [da, db])),
        10 * XLM
    );
    assert_eq!(p.token.balance(&b), 5 * XLM);
}

#[test]
fn cobrar_danos_y_regresar_el_resto() {
    let p = preparar();
    let banda = con_xlm(&p, 10 * XLM);
    let id = p.tix.dejar_deposito(&banda, &p.org, &(10 * XLM));
    assert_eq!(p.tix.cobrar_danos(&id, &(3 * XLM)), 7 * XLM);
    assert_eq!(p.token.balance(&banda), 7 * XLM);
    assert_eq!(p.token.balance(&p.org), 103 * XLM);
}

#[test]
fn reclamar_deposito_solo_despues_del_plazo() {
    let p = preparar();
    let banda = con_xlm(&p, 10 * XLM);
    let id = p.tix.dejar_deposito(&banda, &p.org, &(10 * XLM));
    assert_eq!(p.tix.try_reclamar_deposito(&id), Err(Ok(Error::NoVencido)));
    p.env
        .ledger()
        .set_timestamp(p.env.ledger().timestamp() + PLAZO_DEPOSITO + 1);
    assert_eq!(p.tix.reclamar_deposito(&id), 10 * XLM);
    assert_eq!(p.token.balance(&banda), 10 * XLM);
}
