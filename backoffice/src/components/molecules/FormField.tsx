import { get, useFieldArray, type Control, type FieldErrors, type UseFormRegister } from 'react-hook-form'
import { useTranslation } from 'react-i18next'
import type { FieldDef, FormValues } from '../../features/fields'
import type { LookupOption } from '../../features/lookupSlice'

interface Props { field: FieldDef; register: UseFormRegister<FormValues>; errors: FieldErrors<FormValues>; options?: LookupOption[]; disabled?: boolean; control?: Control<FormValues>; lookups?: Partial<Record<string, LookupOption[]>> }

function ListField({ field, register, errors, control, lookups }: Props) {
  const { t } = useTranslation()
  const { fields, append, remove } = useFieldArray({ control: control as Control<FormValues>, name: field.name as never })
  const listError = (get(errors, field.name) as { message?: string } | undefined)?.message
  return <fieldset className="list-field"><legend>{t(field.labelKey)}{field.required ? ' *' : ''}</legend>
    {fields.map((row, index) => <div className="list-row" key={row.id}>
      {(field.itemFields ?? []).map((sub) => <FormField key={sub.name} field={{ ...sub, name: `${field.name}.${index}.${sub.name}` }} register={register} errors={errors} options={sub.lookup ? lookups?.[sub.lookup] : undefined} />)}
      {!field.fixedRows && <button type="button" className="btn tertiary danger-text" onClick={() => remove(index)}>{t('removeRow')}</button>}</div>)}
    {listError && <small className="field-error">{t(`validation_${listError}`, { defaultValue: listError })}</small>}
    {!field.fixedRows && <button type="button" className="btn secondary" onClick={() => append(Object.fromEntries((field.itemFields ?? []).map((sub) => [sub.name, sub.defaultValue ?? (sub.type === 'checkbox' ? false : '')])) as never)}>{t('addRow')}</button>}</fieldset>
}

export function FormField(props: Props) {
  if (props.field.type === 'list') return <ListField {...props} />
  return <InputField {...props} />
}

function InputField({ field, register, errors, options, disabled }: Props) {
  const { t } = useTranslation()
  const error = (get(errors, field.name) as { message?: string } | undefined)?.message
  const common = { id: field.name, disabled: disabled || field.transient, 'aria-invalid': Boolean(error), ...register(field.name) }
  const label = `${t(field.labelKey)}${field.required ? ' *' : ''}`
  let control
  switch (field.type) {
    case 'hidden': return <input type="hidden" {...common} />
    case 'textarea': control = <textarea rows={3} {...common} />; break
    case 'checkbox': return <label className="check-field"><input type="checkbox" {...common} />{t(field.labelKey)}</label>
    case 'select': control = <select {...common}><option value="">—</option>{field.options?.map((value) => <option key={value} value={value}>{field.optionPrefix ? t(`${field.optionPrefix}${value}`) : value}</option>)}</select>; break
    case 'lookup': control = <select {...common}><option value="">—</option>{options?.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select>; break
    case 'money': control = <input type="number" step="0.01" min={field.min} {...common} />; break
    case 'number': control = <input type="number" step="any" min={field.min} {...common} />; break
    case 'datetime': control = <input type="datetime-local" {...common} />; break
    default: control = <input type={field.type === 'text' ? 'text' : field.type} {...common} />
  }
  return <label htmlFor={field.name}>{label}{control}{error && <small className="field-error">{t(`validation_${error}`, { defaultValue: error })}</small>}</label>
}
