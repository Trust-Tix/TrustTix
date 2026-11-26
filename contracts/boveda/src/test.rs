#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Bytes, Env, String};

const MONTO: i128 = 55_550_000_000; // 5555 XLM

struct Prueba<'a> {
    env: Env,
    boveda: BovedaClient<'a>,
    token: TokenClient<'a>,
    admin: Address,
}

/// Bóveda con el código «Ship It» y fondos para `reclamos` cuentas.
fn preparar<'a>(reclamos: i128) -> Prueba<'a> {
    let env = Env::default();
    env.mock_all_auths();

    let emisor = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(emisor).address();
    let admin = Address::generate(&env);
    let hash = env
        .crypto()
        .sha256(&Bytes::from_slice(&env, b"ship it"))
        .to_bytes();
    let id = env.register(Boveda, (admin.clone(), token_id.clone(), hash, MONTO));
    let boveda = BovedaClient::new(&env, &id);
    let token = TokenClient::new(&env, &token_id);

    if reclamos > 0 {
        StellarAssetClient::new(&env, &token_id).mint(&admin, &(MONTO * reclamos));
        boveda.depositar(&admin, &(MONTO * reclamos));
    }
    Prueba {
        env,
        boveda,
        token,
        admin,
    }
}

fn texto(env: &Env, t: &str) -> String {
    String::from_str(env, t)
}

#[test]
fn depositar_guarda_los_xlm_en_la_boveda() {
    let p = preparar(0);
    StellarAssetClient::new(&p.env, &p.token.address).mint(&p.admin, &(MONTO * 3));

    let disponible = p.boveda.depositar(&p.admin, &(MONTO * 3));
    // Lo firma quien deposita.
    assert_eq!(p.env.auths()[0].0, p.admin);

    assert_eq!(disponible, MONTO * 3);
    assert_eq!(p.boveda.disponible(), MONTO * 3);
    assert_eq!(p.token.balance(&p.admin), 0);
    assert_eq!(
        p.boveda.try_depositar(&p.admin, &0),
        Err(Ok(Error::MontoInvalido))
    );
}

#[test]
fn con_el_codigo_correcto_se_reclama_el_monto() {
    let p = preparar(2);
    let ganador = Address::generate(&p.env);

    let recibido = p.boveda.reclamar(&ganador, &texto(&p.env, "Ship It"));
    // El reclamo lo firma quien recibe.
    assert_eq!(p.env.auths()[0].0, ganador);

    assert_eq!(recibido, MONTO);
    assert_eq!(p.token.balance(&ganador), MONTO);
    assert_eq!(p.boveda.disponible(), MONTO);
    assert_eq!(p.boveda.reclamos(), 1);
    assert!(p.boveda.ya_reclamo(&ganador));
}

#[test]
fn el_codigo_no_distingue_mayusculas_ni_espacios_a_los_lados() {
    let p = preparar(3);
    for (i, codigo) in ["ship it", "  SHIP IT ", "Ship It"].iter().enumerate() {
        let cuenta = Address::generate(&p.env);
        assert_eq!(p.boveda.reclamar(&cuenta, &texto(&p.env, codigo)), MONTO);
        assert_eq!(p.boveda.reclamos(), i as u32 + 1);
    }
}

#[test]
fn un_codigo_incorrecto_no_paga_nada() {
    let p = preparar(1);
    let cuenta = Address::generate(&p.env);
    for codigo in ["Shipit", "Ship  It", "", "Ship It!"] {
        assert_eq!(
            p.boveda.try_reclamar(&cuenta, &texto(&p.env, codigo)),
            Err(Ok(Error::CodigoIncorrecto))
        );
    }
    // 65 caracteres: más largo de lo que acepta la bóveda.
    let largo = "ship it ship it ship it ship it ship it ship it ship it ship it s";
    assert_eq!(largo.len(), 65);
    assert_eq!(
        p.boveda.try_reclamar(&cuenta, &texto(&p.env, largo)),
        Err(Ok(Error::CodigoIncorrecto))
    );
    assert_eq!(p.token.balance(&cuenta), 0);
    assert_eq!(p.boveda.disponible(), MONTO);
}

#[test]
fn cada_cuenta_reclama_una_sola_vez() {
    let p = preparar(2);
    let cuenta = Address::generate(&p.env);
    p.boveda.reclamar(&cuenta, &texto(&p.env, "Ship It"));

    assert_eq!(
        p.boveda.try_reclamar(&cuenta, &texto(&p.env, "Ship It")),
        Err(Ok(Error::YaReclamo))
    );
    assert_eq!(p.token.balance(&cuenta), MONTO);
    assert_eq!(p.boveda.disponible(), MONTO);
}

#[test]
fn sin_fondos_suficientes_no_se_paga_a_medias() {
    let p = preparar(1);
    let primero = Address::generate(&p.env);
    let segundo = Address::generate(&p.env);
    p.boveda.reclamar(&primero, &texto(&p.env, "Ship It"));

    assert_eq!(
        p.boveda.try_reclamar(&segundo, &texto(&p.env, "Ship It")),
        Err(Ok(Error::SinFondos))
    );
    assert_eq!(p.token.balance(&segundo), 0);
    assert!(!p.boveda.ya_reclamo(&segundo));
}

#[test]
fn solo_el_admin_retira_el_sobrante() {
    let p = preparar(2);
    let cuenta = Address::generate(&p.env);
    p.boveda.reclamar(&cuenta, &texto(&p.env, "Ship It"));

    let retirado = p.boveda.retirar_sobrante();
    assert_eq!(p.env.auths()[0].0, p.admin);

    assert_eq!(retirado, MONTO);
    assert_eq!(p.token.balance(&p.admin), MONTO);
    assert_eq!(p.boveda.disponible(), 0);
}

#[test]
#[should_panic]
fn no_se_despliega_con_monto_cero() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let hash = env
        .crypto()
        .sha256(&Bytes::from_slice(&env, b"ship it"))
        .to_bytes();
    env.register(Boveda, (admin, token, hash, 0i128));
}
