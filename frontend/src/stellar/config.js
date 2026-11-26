import { Networks } from '@stellar/stellar-sdk'

// Contrato TrustTix v2 en Stellar Testnet: comisiones, reventa con tope,
// cancelación y reembolso.
export const CONTRATO_ID =
  import.meta.env.VITE_CONTRATO_ID ?? 'CDKVPZI5HC25H6ILACGF24PNTTNYMUDQBSAJGMSUKEQEFUYVSAAKKRD4'

export const RPC_URL = import.meta.env.VITE_RPC_URL ?? 'https://soroban-testnet.stellar.org'
export const HORIZON_URL = import.meta.env.VITE_HORIZON_URL ?? 'https://horizon-testnet.stellar.org'
export const FRIENDBOT_URL = 'https://friendbot.stellar.org'
export const RED = Networks.TESTNET

// Unidad mínima de XLM: 1 XLM = 10,000,000 stroops.
export const STROOPS_POR_XLM = 10_000_000n

const EXPLORADOR = 'https://stellar.expert/explorer/testnet'
export const enlaceTx = (hash) => `${EXPLORADOR}/tx/${hash}`
export const enlaceCuenta = (direccion) => `${EXPLORADOR}/account/${direccion}`
export const enlaceContrato = () => `${EXPLORADOR}/contract/${CONTRATO_ID}`
