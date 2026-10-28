import { useEffect, useMemo } from 'react'
import { FormProvider, useForm } from 'react-hook-form'
import { yupResolver } from '@hookform/resolvers/yup'
import * as yup from 'yup'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import type { AppDispatch, RootState } from '../store'
import { useResourceState } from '../features/hooks'
import { customers } from '../features/resources'
import { ADDRESS_FIELDS } from '../features/address'
import { digitsOnly, isValidDocument, maskDocument, maskPostalCode } from '../features/masks'
import type { Customer } from '../types'
import { PageHeader } from '../components/UI'
import { StateView } from '../components/molecules/StateView'
import { TaxIdField } from '../components/molecules/TaxIdField'
import { AddressFields } from '../components/organisms/AddressFields'

// EPIC-SC-01-S05: the customer registry carries the identity (trade name, legal name, CPF/CNPJ) and the
// address. The document is checked by the customer's country -- CPF or CNPJ in Brazil, EIN in the US --
// and the server answers `InvalidCustomerTaxId` / `DuplicateCustomerTaxId`, which land on the field.

export interface CustomerFormValues {
  name: string; legalName: string; taxId: string; status: string; notes: string
  addressLine1: string; addressLine2: string; locality: string; administrativeArea: string; postalCode: string; countryCode: string
}
const STATUSES = ['Active', 'Inactive']
const TAX_ID_ERROR_KEYS = new Set(['InvalidCustomerTaxId', 'DuplicateCustomerTaxId'])

export const blankCustomer = (countryCode: string): CustomerFormValues => ({
  name: '', legalName: '', taxId: '', status: 'Active', notes: '',
  addressLine1: '', addressLine2: '', locality: '', administrativeArea: '', postalCode: '', countryCode,
})

/// From the record to the form: the stored digits-only document and postal code gain their masks; blanks become empty strings.
export const fromCustomer = (customer: Customer, fallbackCountry: string): CustomerFormValues => {
  const countryCode = customer.countryCode || fallbackCountry
  return {
    name: customer.name ?? '', legalName: customer.legalName ?? '', taxId: maskDocument(customer.taxId ?? '', countryCode), status: customer.status || 'Active', notes: customer.notes ?? '',
    addressLine1: customer.addressLine1 ?? '', addressLine2: customer.addressLine2 ?? '', locality: customer.locality ?? '', administrativeArea: customer.administrativeArea ?? '',
    postalCode: maskPostalCode(customer.postalCode ?? '', countryCode), countryCode,
  }
}

/// From the form to the request body: blanks vanish (as every registry form does), the document travels digits-only -- the shape the server stores and returns.
export const toCustomerPayload = (values: CustomerFormValues): Record<string, unknown> => {
  const body: Record<string, unknown> = { name: values.name.trim(), status: values.status }
  const optional: (keyof CustomerFormValues)[] = ['legalName', 'notes', ...ADDRESS_FIELDS]
  for (const key of optional) { const text = String(values[key] ?? '').trim(); if (text) body[key] = text }
  // Masks are presentation: the document and the postal code travel and are stored as digits only.
  const taxId = digitsOnly(values.taxId ?? '')
  if (taxId) body.taxId = taxId
  const postalCode = digitsOnly(values.postalCode ?? '')
  if (postalCode) body.postalCode = postalCode; else delete body.postalCode
  return body
}

export function CustomerFormPage() {
  const { uuid } = useParams(); const navigate = useNavigate(); const dispatch = useDispatch<AppDispatch>(); const { t } = useTranslation()
  const state = useResourceState(customers)
  const tenantCountry = useSelector((s: RootState) => s.data.currentTenant?.countryCode) || ''
  const fallbackCountry = tenantCountry || 'BR'
  const schema = useMemo(() => yup.object({
    name: yup.string().trim().required(t('validation_required')),
    status: yup.string().oneOf(STATUSES).required(t('validation_required')),
    countryCode: yup.string().oneOf(['BR', 'US']).required(t('validation_required')),
    taxId: yup.string().test('valid-document', t('invalidCpfCnpj'), (value, context) => {
      const country = String(context.parent.countryCode || fallbackCountry)
      if (!digitsOnly(value || '')) return true
      return isValidDocument(value || '', country) || context.createError({ message: t(country === 'BR' ? 'invalidCpfCnpj' : 'invalidTaxId') })
    }),
  }), [t, fallbackCountry])
  const form = useForm<CustomerFormValues>({ resolver: yupResolver(schema) as never, defaultValues: blankCustomer(fallbackCountry) })
  const { control, register, handleSubmit, reset, resetField, setError, watch, formState: { errors } } = form
  const countryCode = watch('countryCode') || ''
  const { current, error, errorKey, saving } = state
  const editing = Boolean(uuid)

  useEffect(() => { dispatch(customers.reset()); if (uuid) dispatch(customers.fetchOne(uuid)); return () => { dispatch(customers.reset()) } }, [dispatch, uuid])
  useEffect(() => { if (editing && current) reset(fromCustomer(current as unknown as Customer, fallbackCountry)) }, [editing, current, reset, fallbackCountry])
  // A new customer starts in the tenant's country; the tenant may arrive after the form did.
  useEffect(() => { if (!editing && tenantCountry) resetField('countryCode', { defaultValue: tenantCountry }) }, [editing, tenantCountry, resetField])
  useEffect(() => { if (errorKey && TAX_ID_ERROR_KEYS.has(errorKey)) setError('taxId', { type: 'server', message: error }) }, [error, errorKey, setError])

  const fieldError = Boolean(errorKey && TAX_ID_ERROR_KEYS.has(errorKey))
  const onSubmit = handleSubmit(async (values) => {
    const action = await dispatch(customers.save({ uuid, body: toCustomerPayload(values) }))
    if (customers.save.fulfilled.match(action)) navigate('/customers')
  })

  return <>
    <PageHeader title={t(editing ? 'editCustomer' : 'newCustomer')} subtitle={t('customersSummary')} />
    <StateView loading={editing && state.loading} error={editing && !current ? error : ''}>
      <FormProvider {...form}>
        <form className="page-form-card" onSubmit={onSubmit} noValidate>
          <div className="form-grid">
            <h2 className="form-section-title">{t('customerIdentity')}</h2>
            <div className="form-row customer-name-row">
              <label htmlFor="customer-name">{t('tradeName')} *<input id="customer-name" aria-invalid={Boolean(errors.name)} {...register('name')} />{errors.name?.message && <small className="field-error" role="alert">{errors.name.message}</small>}</label>
            </div>
            <div className="form-row customer-identity-row">
              <label htmlFor="customer-legal-name">{t('legalName')}<input id="customer-legal-name" {...register('legalName')} /></label>
              <TaxIdField control={control} name="taxId" id="customer-tax-id" kind="any" countryCode={countryCode} label={countryCode === 'US' ? t('taxIdEin') : t('taxIdCpfCnpj')} error={errors.taxId?.message} />
              <label htmlFor="customer-status">{t('status')} *<select id="customer-status" {...register('status')}>{STATUSES.map((value) => <option key={value} value={value}>{t(`customerStatus${value}`)}</option>)}</select></label>
            </div>
            <label className="span-2" htmlFor="customer-notes">{t('observation')}<textarea id="customer-notes" rows={3} {...register('notes')} /></label>
            <h2 className="form-section-title">{t('customerAddress')}</h2>
            <AddressFields />
          </div>
          {error && !fieldError && (!editing || current) && <p className="form-error" role="alert">{t(error, { defaultValue: error })}</p>}
          <div className="form-actions split">
            <Link className="btn secondary" to="/customers">{t('cancel')}</Link>
            <button className="btn primary" type="submit" disabled={saving}>{saving ? t('saving') : t('save')}</button>
          </div>
        </form>
      </FormProvider>
    </StateView>
  </>
}
