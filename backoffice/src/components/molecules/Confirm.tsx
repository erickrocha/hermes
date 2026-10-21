import { AlertTriangle } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Modal } from '../atoms/Modal'

export function Confirm({ title, text, onCancel, onConfirm }: { title: string; text: string; onCancel: () => void; onConfirm: () => void }) { const { t } = useTranslation(); return <Modal title={title} onClose={onCancel}><div className="confirm-body"><span className="warning-icon"><AlertTriangle /></span><p>{text}</p></div><div className="modal-actions"><button className="btn secondary" onClick={onCancel}>{t('cancel')}</button><button className="btn danger" onClick={onConfirm}>{t('confirm')}</button></div></Modal> }
