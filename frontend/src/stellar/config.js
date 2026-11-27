import { Networks } from '@stellar/stellar-sdk'
import boveda from './boveda-id.json'

// Red de Stellar de esta publicación de la app: VITE_RED=mainnet para la red
// principal (dinero real); sin la variable, Testnet. Cada red tiene su propio
// contrato, RPC, Horizon y explorador.
export const ES_MAINNET = import.meta.env.VITE_RED === 'mainnet'
export const NOMBRE_RED = ES_MAINNET ? 'Mainnet' : 'Testnet'
export const RED = ES_MAINNET ? Networks.PUBLIC : Networks.TESTNET

const POR_RED = {
  testnet: {
    // Contrato TrustTix v2: comisiones, reventa con tope, cancelación y reembolso.
    contrato: 'CDKVPZI5HC25H6ILACGF24PNTTNYMUDQBSAJGMSUKEQEFUYVSAAKKRD4',
    rpc: 'https://soroban-testnet.stellar.org',
    horizon: 'https://horizon-testnet.stellar.org',
    explorador: 'https://stellar.expert/explorer/testnet',
  },
  mainnet: {
    // Contrato de contracts/trusttix-mainnet (contracts/scripts/contrato-mainnet.txt).
    contrato: 'CBEPJEJDV2C7HV3PO7V7Z4P5XIADQCXFES2Q2P6DGDVB3LKS2QSPXSJ5',
    rpc: 'https://mainnet.sorobanrpc.com',
    horizon: 'https://horizon.stellar.org',
    explorador: 'https://stellar.expert/explorer/public',
  },
}
const red = POR_RED[ES_MAINNET ? 'mainnet' : 'testnet']

export const CONTRATO_ID = import.meta.env.VITE_CONTRATO_ID || red.contrato

// Bóveda de códigos promocionales (solo Testnet). El ID lo escribe
// contracts/scripts/desplegar-boveda.ps1.
export const BOVEDA_ID = import.meta.env.VITE_BOVEDA_ID || (ES_MAINNET ? '' : boveda.contrato) || ''

export const RPC_URL = import.meta.env.VITE_RPC_URL || red.rpc
export const HORIZON_URL = import.meta.env.VITE_HORIZON_URL || red.horizon
// Friendbot regala XLM de prueba; en Mainnet no existe.
export const FRIENDBOT_URL = ES_MAINNET ? null : 'https://friendbot.stellar.org'

// Comisión de inclusión de cada transacción, en stroops: 0.001 XLM. La que usa
// el SDK por defecto (100) queda debajo del mínimo que pide Mainnet en horas de
// carga, y la red nunca toma la transacción. La de recursos de Soroban se suma aparte.
export const COMISION_INCLUSION = '10000'

// Unidad mínima de XLM: 1 XLM = 10,000,000 stroops.
export const STROOPS_POR_XLM = 10_000_000n

export const enlaceTx = (hash) => `${red.explorador}/tx/${hash}`
export const enlaceCuenta = (direccion) => `${red.explorador}/account/${direccion}`
export const enlaceContrato = (id = CONTRATO_ID) => `${red.explorador}/contract/${id}`
