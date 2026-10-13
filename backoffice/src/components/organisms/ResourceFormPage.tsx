import { useEffect, useMemo, useRef } from 'react'
import { useForm } from 'react-hook-form'
import { yupResolver } from '@hookform/resolvers/yup'
import { Link, useNavigate } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { buildSchema, defaultsFor, fromServer, toPayload, type FieldDef, type FormValues } from '../../features/fields'
import { loadLookup, type LookupKind } from '../../features/lookupSlice'
import type { AppDispatch, RootState } from '../../store'
import { PageHeader } from '../UI'
import { FormField } from '../molecules/FormField'
import { StateView } from '../molecules/StateView'

export interface FormController {
  loading: boolean; saving: boolean; error: string; saved: boolean
  current: Record<string, unknown> | null
  load?: () => void
  submit: (body: Record<string, unknown>) => unknown
  reset: () => void
}
interface Props {
  titleKey: string; subtitleKey?: string; fields: FieldDef[]; backTo: string; controller: FormController
  /** Presente na edição: os campos `createOnly` ficam de fora e o formulário nasce do registro carregado. */
  editing?: boolean
}

/// A página de cadastro/edição: React Hook Form + Yup, navegação em vez de modal, cancelar à esquerda e salvar à direita.
export function ResourceFormPage({ titleKey, subtitleKey, fields, backTo, controller, editing = false }: Props) {
  const { t } = useTranslation(); const navigate = useNavigate(); const dispatch = useDispatch<AppDispatch>()
  const visible = useMemo(() => fields.filter((field) => !(editing && field.createOnly)), [fields, editing])
  const schema = useMemo(() => buildSchema(visible), [visible])
  const { register, handleSubmit, reset, control, formState: { errors } } = useForm<FormValues>({ resolver: yupResolver(schema) as never, defaultValues: defaultsFor(visible) })
  const lookups = useSelector((state: RootState) => state.lookup)
  const kinds = useMemo(() => [...new Set(visible.flatMap((f) => (f.type === 'list' ? f.itemFields ?? [] : [f])).filter((f) => f.type === 'lookup').map((f) => f.lookup as LookupKind))], [visible])
  const { load, reset: clear, saved, current } = controller
  useEffect(() => { kinds.forEach((kind) => dispatch(loadLookup(kind))) }, [dispatch, kinds])
  useEffect(() => { clear(); load?.(); return () => { clear() } }, [load, clear])
  useEffect(() => { if (current) reset(fromServer(visible, current)) }, [current, visible, reset])
  // `saved` pode vir de uma ação anterior da mesma slice (sincronizar, excluir…): só vale depois do envio desta página.
  const submitted = useRef(false)
  useEffect(() => { if (saved && submitted.current) navigate(backTo) }, [saved, navigate, backTo])
  const onSubmit = handleSubmit((values) => { submitted.current = true; controller.submit(toPayload(visible, values, editing)) })
  return <>
    <PageHeader title={t(titleKey)} subtitle={subtitleKey ? t(subtitleKey) : undefined} />
    <StateView loading={controller.loading} error={editing && !current ? controller.error : ''}>
      <form className="page-form-card" onSubmit={onSubmit} noValidate>
        <div className="form-grid">{visible.map((field) => <FormField key={field.name} field={field} register={register} errors={errors} control={control} lookups={lookups} options={field.lookup ? lookups[field.lookup] : undefined} />)}</div>
        {controller.error && (!editing || current) && <p className="form-error" role="alert">{t(controller.error, { defaultValue: controller.error })}</p>}
        <div className="form-actions split">
          <Link className="btn secondary" to={backTo}>{t('cancel')}</Link>
          <button className="btn primary" type="submit" disabled={controller.saving}>{controller.saving ? t('saving') : t('save')}</button>
        </div>
      </form>
    </StateView>
  </>
}
