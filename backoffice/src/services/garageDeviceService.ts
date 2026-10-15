import { api } from '../api'

export interface GarageDevice {
  uuid: string
  name: string
  paired: boolean
  revoked: boolean
  pairedAt?: string | null
  revokedAt?: string | null
}

export interface PairingCode {
  deviceUuid: string
  deviceName: string
  code: string
  expiresAt: string
}

export const listGarageDevices = async (): Promise<GarageDevice[]> =>
  (await api.get<GarageDevice[]>('/garage-device')).data

export const issueGarageDeviceCode = async (name: string): Promise<PairingCode> =>
  (await api.post<PairingCode>('/garage-device/pairing-code', { name })).data

export const reissueGarageDeviceCode = async (uuid: string): Promise<PairingCode> =>
  (await api.post<PairingCode>(`/garage-device/uuid/${uuid}/pairing-code`)).data

export const revokeGarageDevice = async (uuid: string): Promise<void> => {
  await api.delete(`/garage-device/uuid/${uuid}`)
}