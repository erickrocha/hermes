import { Check, ChevronDown, LoaderCircle, Search, X } from 'lucide-react'
import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from 'react'
import { useTranslation } from 'react-i18next'

export type ComboboxValue = string | number
export type ComboboxOption<T extends ComboboxValue> = { value: T; label: string; disabled?: boolean }

type CommonProps<T extends ComboboxValue> = {
  options: ComboboxOption<T>[]
  placeholder?: string
  filterable?: boolean
  disabled?: boolean
  loading?: boolean
  name?: string
  ariaLabel?: string
}

type SingleProps<T extends ComboboxValue> = CommonProps<T> & {
  multiple?: false
  value: T | null
  onChange: (value: T | null) => void
}

type MultiProps<T extends ComboboxValue> = CommonProps<T> & {
  multiple: true
  value: T[]
  onChange: (value: T[]) => void
}

export type ComboboxProps<T extends ComboboxValue> = SingleProps<T> | MultiProps<T>

export function Combobox<T extends ComboboxValue>(props: ComboboxProps<T>) {
  const { t } = useTranslation()
  const id = useId()
  const root = useRef<HTMLDivElement>(null)
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [activeIndex, setActiveIndex] = useState(-1)
  const selectedValues: T[] = props.multiple ? props.value : props.value == null ? [] : [props.value]
  const selected = props.options.filter((option) => selectedValues.includes(option.value))
  const filtered = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase()
    return normalized ? props.options.filter((option) => option.label.toLocaleLowerCase().includes(normalized)) : props.options
  }, [props.options, query])

  useEffect(() => {
    const close = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [])

  useEffect(() => {
    if (!open) setActiveIndex(-1)
  }, [open])

  useEffect(() => {
    setActiveIndex(-1)
  }, [query])

  const choose = (option: ComboboxOption<T>) => {
    if (option.disabled) return
    if (props.multiple) {
      props.onChange(props.value.includes(option.value) ? props.value.filter((value) => value !== option.value) : [...props.value, option.value])
    } else {
      props.onChange(option.value)
      setOpen(false)
    }
    setQuery('')
  }

  const remove = (value: T) => {
    if (props.multiple) props.onChange(props.value.filter((item) => item !== value))
    else props.onChange(null)
  }

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === 'Escape') { setOpen(false); return }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      setOpen(true)
      const direction = event.key === 'ArrowDown' ? 1 : -1
      setActiveIndex((index) => {
        if (index < 0) return direction > 0 ? 0 : filtered.length - 1
        return Math.max(0, Math.min(filtered.length - 1, index + direction))
      })
    }
    if (event.key === 'Enter') {
      const targetOption = filtered[activeIndex] || (activeIndex < 0 && filtered.length > 0 ? filtered[0] : undefined)
      if (targetOption) {
        event.preventDefault()
        choose(targetOption)
      }
    }
  }

  const display = selected[0]?.label || props.placeholder || t('selectOption')
  return <div className={`combobox ${props.disabled ? 'disabled' : ''}`} ref={root} onKeyDown={onKeyDown}>
    {props.name && selectedValues.map((value) => <input key={String(value)} type="hidden" name={props.name} value={value} />)}
    <div className="combobox-control" role="combobox" aria-expanded={open} aria-controls={`${id}-listbox`} aria-haspopup="listbox" aria-label={props.ariaLabel || props.placeholder || t('selectOption')}>
      {props.multiple && selected.map((option) => <span className="combobox-chip" key={option.value}>{option.label}<button type="button" aria-label={`${t('remove')} ${option.label}`} onClick={(event) => { event.stopPropagation(); remove(option.value) }}><X size={13} /></button></span>)}
      {props.filterable ? <><Search size={16} /><input value={query} disabled={props.disabled} placeholder={open && selected.length ? t('search') : display} onFocus={() => setOpen(true)} onChange={(event) => { setQuery(event.target.value); setOpen(true) }} /></> : <button type="button" className="combobox-trigger" disabled={props.disabled} onClick={() => setOpen(!open)}><span>{props.multiple ? (selected.length ? t('selectedCount', { count: selected.length }) : display) : display}</span></button>}
      {props.loading ? <LoaderCircle className="spin" size={17} /> : <ChevronDown size={17} />}
    </div>
    {open && !props.disabled && <div className="combobox-menu" id={`${id}-listbox`} role="listbox" aria-multiselectable={props.multiple || undefined}>
      {filtered.length ? filtered.map((option, index) => {
        const isSelected = selectedValues.includes(option.value)
        return <button type="button" role="option" aria-selected={isSelected} disabled={option.disabled} className={index === activeIndex ? 'active' : ''} key={option.value} onMouseEnter={() => setActiveIndex(index)} onClick={() => choose(option)}><span>{option.label}</span>{isSelected && <Check size={16} />}</button>
      }) : <p>{t('noOptions')}</p>}
    </div>}
  </div>
}
