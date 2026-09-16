import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { Provider } from 'react-redux'
import { BrowserRouter } from 'react-router-dom'
import { store } from './store'
import './i18n'
import './styles/tailwind.css'
import './styles/main.scss'
import App from './App'

createRoot(document.getElementById('root')!).render(
  <StrictMode><Provider store={store}><BrowserRouter><App /></BrowserRouter></Provider></StrictMode>,
)
