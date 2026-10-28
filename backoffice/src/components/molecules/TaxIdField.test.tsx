import { fireEvent, render, screen } from '@testing-library/react'
import { useForm } from 'react-hook-form'
import { describe, expect, it, vi } from 'vitest'
import '../../i18n'
import { TaxIdField } from './TaxIdField'

function Harness({ countryCode, kind, onValues, error }: { countryCode: string; kind: 'company' | 'any'; onValues?: (values: { taxId: string }) => void; error?: string }) {
  const { control, handleSubmit } = useForm<{ taxId: string }>({ defaultValues: { taxId: '' } })
  return <form onSubmit={handleSubmit((values) => onValues?.(values))}><TaxIdField control={control} name="taxId" countryCode={countryCode} kind={kind} label="Doc" error={error} /><button type="submit">go</button></form>
}

describe('TaxIdField', () => {
  it('masks a Brazilian document as a CPF and then as a CNPJ while the person types, keeping the masked text in the form', async () => {
    const onValues = vi.fn()
    render(<Harness countryCode="BR" kind="any" onValues={onValues} />)
    const input = screen.getByLabelText(/^Doc/)
    fireEvent.change(input, { target: { value: '52998224725' } })
    expect(input).toHaveValue('529.982.247-25')
    fireEvent.change(input, { target: { value: '11222333000181' } })
    expect(input).toHaveValue('11.222.333/0001-81')
    fireEvent.click(screen.getByText('go'))
    await vi.waitFor(() => expect(onValues).toHaveBeenCalledWith({ taxId: '11.222.333/0001-81' }))
  })

  it('a company-only field keeps the CNPJ mask even for eleven digits, and the US field reads as an EIN', () => {
    const { unmount } = render(<Harness countryCode="BR" kind="company" />)
    fireEvent.change(screen.getByLabelText(/^Doc/), { target: { value: '52998224725' } })
    expect(screen.getByLabelText(/^Doc/)).toHaveValue('52.998.224/725')
    unmount()
    render(<Harness countryCode="US" kind="any" />)
    fireEvent.change(screen.getByLabelText(/^Doc/), { target: { value: '123456789' } })
    expect(screen.getByLabelText(/^Doc/)).toHaveValue('12-3456789')
    expect(screen.getByLabelText(/^Doc/)).toHaveAttribute('maxlength', '10')
  })

  it('announces the error it is given', () => {
    render(<Harness countryCode="BR" kind="any" error="Nope" />)
    expect(screen.getByRole('alert')).toHaveTextContent('Nope')
    expect(screen.getByLabelText(/^Doc/)).toHaveAttribute('aria-invalid', 'true')
  })
})
