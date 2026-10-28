import { Controller, type Control, type FieldValues, type Path } from 'react-hook-form'
import { maskDocument, type TaxIdKind } from '../../features/masks'

interface Props<T extends FieldValues> {
  control: Control<T>
  name: Path<T>
  countryCode: string
  label: string
  /** `company`: CNPJ (BR) or EIN (US) only; `any`: a person's CPF as well. */
  kind?: TaxIdKind
  required?: boolean
  id?: string
  error?: string
}

/// A fiscal document input masked by country: the form state holds the masked text, exactly as typed.
export function TaxIdField<T extends FieldValues>({ control, name, countryCode, label, kind = 'company', required = false, id, error }: Props<T>) {
  const maxLength = countryCode === 'BR' ? 18 : 10
  return (
    <label>
      {label}
      <Controller control={control} name={name} render={({ field }) => (
        <input id={id} required={required} inputMode="numeric" autoComplete="off" maxLength={maxLength} aria-invalid={Boolean(error)}
          value={maskDocument(String(field.value ?? ''), countryCode, kind)} onBlur={field.onBlur}
          onChange={(event) => field.onChange(maskDocument(event.target.value, countryCode, kind))} ref={field.ref} />
      )} />
      {error && <small className="field-error" role="alert">{error}</small>}
    </label>
  )
}
