import { execFileSync } from 'node:child_process'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const directory = dirname(fileURLToPath(import.meta.url))
const acceptanceScript = resolve(directory, '../../../acceptance/billing_acceptance.py')
const hermesDirectory = resolve(directory, '../../../')

function cleanup(tag: string, environment: NodeJS.ProcessEnv) {
  execFileSync('python3', [acceptanceScript, '--cleanup-tag', tag], {
    cwd: hermesDirectory,
    env: environment,
    stdio: 'inherit',
  })
}

export default async function globalSetup() {
  const required = ['ACCESS_TOKEN_SECRET', 'HERMES_ADMIN_EMAIL', 'HERMES_ADMIN_PASSWORD', 'HERMES_DB_PASSWORD']
  const missing = required.filter((key) => !process.env[key])
  if (missing.length) throw new Error(`Missing billing E2E environment variables: ${missing.join(', ')}`)

  const environment: NodeJS.ProcessEnv = {
    ...process.env,
    HERMES_API: process.env.HERMES_API || 'http://127.0.0.1:8081',
    HERMES_DB_CONTAINER: process.env.HERMES_DB_CONTAINER || 'hermes-billing-qa-mariadb',
  }

  let output = ''
  try {
    output = execFileSync('python3', [acceptanceScript, '--keep'], {
      cwd: hermesDirectory,
      env: environment,
      encoding: 'utf8',
    })
  } catch (error) {
    output = String((error as NodeJS.ErrnoException & { stdout?: Buffer }).stdout || '')
    const retained = output.match(/Fixtures retained: tag=(qa-bill-[a-f0-9]{10})/)
    if (retained) cleanup(retained[1], environment)
    throw error
  }

  const retained = output.match(
    /Fixtures retained: tag=(qa-bill-[a-f0-9]{10}); ownerA=([^;]+); ownerB=([^;]+); password=([^\r\n]+)/,
  )
  if (!retained || !output.includes('Billing acceptance: 11/11 passed; failed=[]')) {
    const partialTag = output.match(/Fixtures retained: tag=(qa-bill-[a-f0-9]{10})/)
    if (partialTag) cleanup(partialTag[1], environment)
    throw new Error(`Billing API acceptance did not pass before browser tests:\n${output}`)
  }

  process.env.BILLING_TEST_TAG = retained[1]
  process.env.BILLING_TEST_OWNER_A = retained[2]
  process.env.BILLING_TEST_OWNER_B = retained[3]
  process.env.BILLING_TEST_PASSWORD = retained[4]
  process.env.BILLING_TEST_TENANT_B = `${retained[1]}-tenant-B`
  process.env.BILLING_TEST_ADMIN = process.env.HERMES_ADMIN_EMAIL
  process.env.BILLING_TEST_ADMIN_PASSWORD = process.env.HERMES_ADMIN_PASSWORD
  console.log('Billing API setup passed (11/11); browser fixtures are isolated.')

  return async () => cleanup(retained[1], environment)
}