import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Empty, Loading } from '../UI'

/// Os estados de toda página que busca dados: carregando, erro, vazio e (com os dados) sucesso.
export function StateView({ loading, error, empty, children }: { loading: boolean; error?: string; empty?: boolean; children: ReactNode }) {
  const { t } = useTranslation()
  if (loading) return <Loading />
  if (error) return <p className="form-error" role="alert">{t(error, { defaultValue: error })}</p>
  if (empty) return <Empty />
  return <>{children}</>
}
