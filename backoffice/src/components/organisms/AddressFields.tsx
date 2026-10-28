import { useEffect } from 'react'
import { Controller, useFormContext } from 'react-hook-form'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import type { AppDispatch, RootState } from '../../store'
import { clearCities, loadCities, loadProvinces } from '../../store'
import type { AddressValues } from '../../features/address'
import { maskPostalCode } from '../../features/masks'
import { Combobox } from '../Combobox'

/// The postal address block of a registry form: street, complement, postal code, then the country (BR/US)
/// with the province and city pickers fed by the reference data. Reads the surrounding `FormProvider`,
/// loads the provinces of the chosen country and the cities of the chosen province, and clears the
/// dependents when a parent changes.
export function AddressFields() {
  const { t } = useTranslation()
  const dispatch = useDispatch<AppDispatch>()
  const { provinces, cities } = useSelector((s: RootState) => s.data)
  const { control, register, setValue, watch } = useFormContext<AddressValues>()
  const countryCode = watch('countryCode') || ''
  const administrativeArea = watch('administrativeArea') || ''

  useEffect(() => {
    if (countryCode) dispatch(loadProvinces(countryCode))
  }, [dispatch, countryCode])

  useEffect(() => {
    if (!administrativeArea) {
      dispatch(clearCities())
      return
    }
    const province = provinces.find((item) => item.acronym === administrativeArea || item.name === administrativeArea)
    if (province) dispatch(loadCities(province.id))
  }, [dispatch, administrativeArea, provinces])

  const handleCountryChange = () => {
    setValue('administrativeArea', '')
    setValue('locality', '')
    dispatch(clearCities())
  }

  return (
    <>
      <div className="form-row address-row">
        <label>{t('address')}<input autoComplete="address-line1" {...register('addressLine1')} /></label>
      </div>
      <div className="form-row address-complement-row">
        <label>{t('address2')}<input autoComplete="address-line2" {...register('addressLine2')} /></label>
        <label>{t('postalCode')}<Controller control={control} name="postalCode" render={({ field }) => <input autoComplete="postal-code" inputMode="numeric" maxLength={countryCode === 'BR' ? 9 : 10} value={maskPostalCode(field.value || '', countryCode)} onBlur={field.onBlur} onChange={(event) => field.onChange(maskPostalCode(event.target.value, countryCode))} ref={field.ref} />} /></label>
      </div>
      <div className="form-row address-location-row">
        <div className="address-country-field">
          <span className="form-field-label">{t('country')}</span>
          <div className="radio-group" role="radiogroup" aria-label={t('country')}>
            <label className="radio-option"><input type="radio" value="BR" aria-label={t('countryBrazil')} {...register('countryCode', { onChange: handleCountryChange })} /><span aria-hidden="true">🇧🇷</span><span>{t('countryBrazil')}</span></label>
            <label className="radio-option"><input type="radio" value="US" aria-label={t('countryUnitedStates')} {...register('countryCode', { onChange: handleCountryChange })} /><span aria-hidden="true">🇺🇸</span><span>{t('countryUnitedStates')}</span></label>
          </div>
        </div>
        <label>{t('province')}<Controller control={control} name="administrativeArea" render={({ field }) => <Combobox filterable disabled={!countryCode || !provinces.length} ariaLabel={t('province')} value={field.value || null} options={provinces.map((province) => ({ value: province.acronym, label: `${province.name} (${province.acronym})` }))} onChange={(value) => { field.onChange(value || ''); setValue('locality', ''); if (!value) dispatch(clearCities()) }} />} /></label>
        <label>{t('city')}<Controller control={control} name="locality" render={({ field }) => <Combobox filterable disabled={!administrativeArea || !cities.length} ariaLabel={t('city')} value={field.value || null} options={cities.map((city) => ({ value: city.name, label: city.name }))} onChange={(value) => field.onChange(value || '')} />} /></label>
      </div>
    </>
  )
}
