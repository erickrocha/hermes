import { useEffect, useMemo } from 'react'
import { useForm } from 'react-hook-form'
import { yupResolver } from '@hookform/resolvers/yup'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { buildSchema, defaultsFor, toPayload, type FieldDef, type FormValues } from '../../features/fields'
import { loadLookup, type LookupKind } from '../../features/lookupSlice'
import type { AppDispatch, RootState } from '../../store'
import { FormField } from '../molecules/FormField'

/// Os filtros de uma consulta: React Hook Form + Yup como os cadastros, mas o envio só aplica os parâmetros (nada é gravado).
export function FilterBar({ fields, onApply, applyKey = 'generate' }: { fields: FieldDef[]; onApply: (params: Record<string, unknown>) => void; applyKey?: string }) {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>()
  const schema = useMemo(() => buildSchema(fields), [fields])
  const { register, handleSubmit, formState: { errors } } = useForm<FormValues>({ resolver: yupResolver(schema) as never, defaultValues: defaultsFor(fields) })
  const lookups = useSelector((state: RootState) => state.lookup)
  const kinds = useMemo(() => [...new Set(fields.filter((f) => f.type === 'lookup').map((f) => f.lookup as LookupKind))], [fields])
  useEffect(() => { kinds.forEach((kind) => dispatch(loadLookup(kind))) }, [dispatch, kinds])
  return <form className="toolbar form-inline" onSubmit={handleSubmit((values) => onApply(toPayload(fields, values)))} noValidate>
    {fields.map((field) => <FormField key={field.name} field={field} register={register} errors={errors} options={field.lookup ? lookups[field.lookup] : undefined} />)}
    <button className="btn primary" type="submit">{t(applyKey)}</button>
  </form>
}
