import { Component, type ErrorInfo, type ReactNode } from 'react'
import { withTranslation, type WithTranslation } from 'react-i18next'

// U-1 / DEF-BO-03: a render error used to unmount the whole console and leave
// a white page -- the operator's only clue was a stack trace in a console
// they never open. A boundary turns that into a page that says something went
// wrong and offers a way back, and keeps the failure contained to the route
// that caused it rather than taking the shell down with it.
type Props = WithTranslation & { children: ReactNode }

class Boundary extends Component<Props, { failed: boolean }> {
  state = { failed: false }

  static getDerivedStateFromError() {
    return { failed: true }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('[hermes] unhandled render error', error, info.componentStack)
  }

  render() {
    if (!this.state.failed) return this.props.children
    const { t } = this.props
    return <section className="state-box" role="alert">
      <h2>{t('unexpectedErrorTitle')}</h2>
      <p>{t('unexpectedErrorText')}</p>
      <button className="btn primary" onClick={() => { this.setState({ failed: false }); window.location.assign('/') }}>{t('backToDashboard')}</button>
    </section>
  }
}

export const ErrorBoundary = withTranslation()(Boundary)
