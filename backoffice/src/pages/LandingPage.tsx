import { useEffect } from 'react'
import { Link } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { ArrowRight, Building2, CalendarClock, ClipboardCheck, Coins, Fuel, Mail, Radio, ShieldCheck, Warehouse, Wrench, type LucideIcon } from 'lucide-react'
import type { AppDispatch, RootState } from '../store'
import { fetchPublicPlans } from '../features/publicPlanSlice'
import { LanguagePicker } from '../components/LanguagePicker'
import { applyTheme } from '../theme'
import { CONTACT_EMAIL } from '../config'
import type { PublicPlan } from '../types'

const money = (cents: number, locale: string) => new Intl.NumberFormat(locale, { style: 'currency', currency: 'BRL' }).format(cents / 100)

const MODULES: { key: string; icon: LucideIcon }[] = [
  { key: 'Ops', icon: CalendarClock }, { key: 'Maint', icon: Wrench }, { key: 'Garage', icon: Warehouse },
  { key: 'Fuel', icon: Fuel }, { key: 'Checklist', icon: ClipboardCheck }, { key: 'Telemetry', icon: Radio }, { key: 'Cost', icon: Coins },
]
const STEPS = [1, 2, 3]
const REASONS = [{ n: 1, icon: Building2 }, { n: 2, icon: ShieldCheck }, { n: 3, icon: Wrench }]

function PlanCard({ plan, locale, featured }: { plan: PublicPlan; locale: string; featured: boolean }) {
  const { t } = useTranslation()
  return <article className={featured ? 'landing-plan featured' : 'landing-plan'}>
    <h3>{plan.name}</h3>
    <p className="landing-plan-price"><b>{money(plan.priceInCents, locale)}</b><span>{t('landingPlanPeriod', { count: plan.periodDays })}</span></p>
    <ul><li>{t('landingPlanUsers', { count: plan.availableUsers })}</li><li>{t('landingPlanExtra', { price: money(plan.pricePerExtraUserInCents, locale) })}</li></ul>
    <Link className={featured ? 'btn primary' : 'btn'} to="/login">{t('landingPlanCta')}</Link>
  </article>
}

function PlansSection() {
  const { t, i18n } = useTranslation(); const dispatch = useDispatch<AppDispatch>()
  const { items, loading, error } = useSelector((s: RootState) => s.publicPlans)
  useEffect(() => { dispatch(fetchPublicPlans()) }, [dispatch])
  const featured = items.length > 2 ? Math.floor(items.length / 2) : -1
  return <section id="plans" className="landing-section alt"><div className="landing-wrap">
    <header className="landing-heading"><h2>{t('landingPlansTitle')}</h2><p>{t('landingPlansSubtitle')}</p></header>
    {loading && <p className="landing-state" role="status">{t('loading')}</p>}
    {!loading && error && <div className="landing-state" role="alert"><p>{t('landingPlansError')}</p><button className="btn" onClick={() => dispatch(fetchPublicPlans())}>{t('landingPlansRetry')}</button></div>}
    {!loading && !error && items.length === 0 && <p className="landing-state">{t('landingPlansEmpty')}</p>}
    {!loading && !error && items.length > 0 && <div className="landing-plans">{items.map((plan, index) => <PlanCard key={plan.name} plan={plan} locale={i18n.language} featured={index === featured} />)}</div>}
  </div></section>
}

export function LandingPage() {
  const { t } = useTranslation()
  // A landing page é sempre a identidade padrão do Hermes; o tema salvo só vale da tela de login em diante.
  useEffect(() => { applyTheme(null) }, [])
  return <div className="landing">
    <header className="landing-nav"><div className="landing-wrap landing-nav-row">
      <Link to="/" className="landing-brand"><span><ShieldCheck size={20} /></span><b>hermes</b></Link>
      <nav aria-label="main"><a href="#features">{t('landingNavFeatures')}</a><a href="#plans">{t('landingNavPlans')}</a><a href="#contact">{t('landingNavContact')}</a></nav>
      <div className="landing-nav-actions"><LanguagePicker className="landing-language" /><Link className="btn primary" to="/login">{t('landingAccess')}</Link></div>
    </div></header>

    <section className="landing-hero"><div className="landing-wrap landing-hero-grid">
      <div><span className="eyebrow">{t('landingHeroEyebrow')}</span><h1>{t('landingHeroTitle')}</h1><p>{t('landingHeroText')}</p>
        <div className="landing-cta"><Link className="btn primary" to="/login">{t('landingAccess')} <ArrowRight size={18} /></Link><a className="btn ghost" href="#plans">{t('landingHeroSeePlans')}</a></div></div>
      <div className="landing-hero-art" aria-hidden="true"><span><ShieldCheck size={72} /></span><i /><i /><i /></div>
    </div></section>

    <section id="features" className="landing-section"><div className="landing-wrap">
      <header className="landing-heading"><h2>{t('landingFeaturesTitle')}</h2><p>{t('landingFeaturesSubtitle')}</p></header>
      <div className="landing-grid">{MODULES.map(({ key, icon: Icon }) => <article className="landing-card" key={key}><span className="landing-icon"><Icon size={22} /></span><h3>{t(`landingMod${key}Title`)}</h3><p>{t(`landingMod${key}Text`)}</p></article>)}</div>
    </div></section>

    <section className="landing-section alt"><div className="landing-wrap">
      <header className="landing-heading"><h2>{t('landingHowTitle')}</h2><p>{t('landingHowSubtitle')}</p></header>
      <ol className="landing-steps">{STEPS.map((n) => <li key={n}><span>{n}</span><h3>{t(`landingStep${n}Title`)}</h3><p>{t(`landingStep${n}Text`)}</p></li>)}</ol>
    </div></section>

    <section className="landing-section"><div className="landing-wrap">
      <header className="landing-heading"><h2>{t('landingWhyTitle')}</h2></header>
      <div className="landing-grid three">{REASONS.map(({ n, icon: Icon }) => <article className="landing-card" key={n}><span className="landing-icon"><Icon size={22} /></span><h3>{t(`landingWhy${n}Title`)}</h3><p>{t(`landingWhy${n}Text`)}</p></article>)}</div>
    </div></section>

    <PlansSection />

    <section id="contact" className="landing-section"><div className="landing-wrap"><div className="landing-contact">
      <h2>{t('landingContactTitle')}</h2><p>{t('landingContactText')}</p>
      <a className="btn accent" href={`mailto:${CONTACT_EMAIL}`}><Mail size={18} /> {t('landingContactCta')}</a>
    </div></div></section>

    <footer className="landing-footer"><div className="landing-wrap"><span>© {new Date().getFullYear()} hermes. {t('landingFooterRights')}</span><Link to="/login">{t('landingAccess')}</Link></div></footer>
  </div>
}
