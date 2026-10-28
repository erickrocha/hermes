import { useTranslation } from 'react-i18next'
import { addressLines, type AddressValues } from '../../features/address'

/// A postal address as one block, one line per part; nothing when every part is blank.
export function AddressBlock({ address }: { address: AddressValues }) {
  const { t } = useTranslation()
  const lines = addressLines(address, t)
  if (!lines.length) return null
  return <address className="address-block">{lines.map((line) => <span key={line}>{line}</span>)}</address>
}
