package com.progressrail.hermes.client.driver

import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.view.Gravity
import android.view.ViewGroup
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import com.progressrail.hermes.client.shared.AccessTokenResponse
import com.progressrail.hermes.client.shared.AuthApi
import com.progressrail.hermes.client.shared.DriverSession
import com.progressrail.hermes.client.shared.createHermesHttpClient
import com.progressrail.hermes.client.shared.isDriverRole
import com.progressrail.hermes.client.shared.secondsUntilRefresh
import io.ktor.client.HttpClient
import io.ktor.client.plugins.ClientRequestException
import io.ktor.http.HttpStatusCode
import kotlinx.coroutines.runBlocking
import java.util.concurrent.Executors

/**
 * Driver app shell: sign in through the platform's existing auth, keep the session encrypted, renew it, sign out.
 * No domain screens here; C-031 plugs them into [renderContent].
 */
class MainActivity : ComponentActivity() {
    private val mainHandler = Handler(Looper.getMainLooper())
    private val executor = Executors.newSingleThreadExecutor()
    private lateinit var store: DriverSessionStore
    private lateinit var httpClient: HttpClient
    private var session: DriverSession? = null
    private var loginButton: Button? = null
    private var refreshInProgress = false
    private val refreshRunnable = Runnable { session?.let(::refresh) }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        store = DriverSessionStore(this)
        httpClient = createHermesHttpClient()
        val saved = store.load()
        session = saved
        if (saved == null) showLogin() else showAuthenticated(saved)
    }

    override fun onStart() {
        super.onStart()
        session?.let(::scheduleRefresh)
    }

    override fun onStop() {
        mainHandler.removeCallbacks(refreshRunnable)
        super.onStop()
    }

    override fun onDestroy() {
        mainHandler.removeCallbacks(refreshRunnable)
        executor.shutdownNow()
        httpClient.close()
        super.onDestroy()
    }

    /** Extension point for the C-031 driver screens. The shell renders no fleet-operations data. */
    @Suppress("UNUSED_PARAMETER")
    private fun renderContent(session: DriverSession, container: LinearLayout) = Unit

    private fun showLogin(message: String? = null, email: String = "") {
        val content = createContent()
        content.addView(text(getString(R.string.login_title), R.style.TextAppearance_Hermes_Title))
        content.addView(text(getString(R.string.login_message), R.style.TextAppearance_Hermes_Body))

        val emailInput = EditText(this).apply {
            hint = getString(R.string.email_hint)
            contentDescription = getString(R.string.email_hint)
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
            setSingleLine(true)
            setText(email)
        }
        val passwordInput = EditText(this).apply {
            hint = getString(R.string.password_hint)
            contentDescription = getString(R.string.password_hint)
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
            setSingleLine(true)
        }
        content.addView(emailInput, contentParams())
        content.addView(passwordInput, contentParams())

        val submit = Button(this).apply {
            text = getString(R.string.login_button)
            setOnClickListener { submitLogin(emailInput.text.toString().trim(), passwordInput.text.toString()) }
        }
        loginButton = submit
        content.addView(submit, contentParams())
        if (message != null) content.addView(text(message, R.style.TextAppearance_Hermes_Error), contentParams())
        setContentView(content)
    }

    private fun showAuthenticated(current: DriverSession, message: String? = null) {
        val content = createContent()
        content.addView(text(getString(R.string.authenticated_title, current.name), R.style.TextAppearance_Hermes_Title))
        content.addView(text(getString(R.string.authenticated_role), R.style.TextAppearance_Hermes_Body))
        content.addView(text(getString(R.string.authenticated_message), R.style.TextAppearance_Hermes_Body))
        if (message != null) content.addView(text(message, R.style.TextAppearance_Hermes_Error), contentParams())
        renderContent(current, content)
        content.addView(
            Button(this).apply {
                text = getString(R.string.sign_out_button)
                setOnClickListener { signOut() }
            },
            contentParams(),
        )
        setContentView(content)
    }

    private fun submitLogin(email: String, password: String) {
        if (email.isEmpty() || password.isEmpty()) {
            showLogin(getString(R.string.credentials_required), email)
            return
        }
        loginButton?.isEnabled = false
        loginButton?.text = getString(R.string.login_in_progress)
        executor.execute {
            try {
                val response = runBlocking { AuthApi(httpClient, BuildConfig.API_BASE_URL).login(email, password) }
                mainHandler.post { if (alive()) acceptLogin(response, email) }
            } catch (failure: ClientRequestException) {
                val message = if (failure.response.status == HttpStatusCode.Unauthorized) {
                    getString(R.string.credentials_invalid)
                } else {
                    getString(R.string.login_failed)
                }
                mainHandler.post { if (alive()) showLogin(message, email) }
            } catch (_: Exception) {
                mainHandler.post { if (alive()) showLogin(getString(R.string.login_failed), email) }
            }
        }
    }

    private fun acceptLogin(response: AccessTokenResponse, email: String) {
        val next = response.toSession()
        if (next == null || !isDriverRole(response.role)) {
            // Interface courtesy only: nothing is stored; the server remains the authority.
            showLogin(getString(R.string.not_a_driver), email)
            return
        }
        store.save(next)
        session = next
        showAuthenticated(next)
        scheduleRefresh(next)
    }

    private fun refresh(current: DriverSession) {
        if (refreshInProgress || !alive()) return
        refreshInProgress = true
        executor.execute {
            try {
                val response = runBlocking { AuthApi(httpClient, BuildConfig.API_BASE_URL).refresh(current.refreshToken) }
                mainHandler.post {
                    refreshInProgress = false
                    if (!alive()) return@post
                    val next = response.toSession()
                    if (next == null || !isDriverRole(response.role)) {
                        endSession(getString(R.string.not_a_driver))
                    } else {
                        // The backend issues a new refresh token on every refresh: replace what is stored.
                        store.save(next)
                        session = next
                        showAuthenticated(next) // also clears a stale "no connection" message
                        scheduleRefresh(next)
                    }
                }
            } catch (failure: ClientRequestException) {
                mainHandler.post {
                    refreshInProgress = false
                    if (!alive()) return@post
                    if (failure.response.status == HttpStatusCode.Unauthorized) {
                        endSession(getString(R.string.session_expired))
                    } else {
                        retryLater(current)
                    }
                }
            } catch (_: Exception) {
                mainHandler.post {
                    refreshInProgress = false
                    if (alive()) retryLater(current)
                }
            }
        }
    }

    /** Network trouble keeps the session; try again shortly. */
    private fun retryLater(current: DriverSession) {
        mainHandler.removeCallbacks(refreshRunnable)
        mainHandler.postDelayed(refreshRunnable, RETRY_MILLIS)
        if (session == current) showAuthenticated(current, getString(R.string.refresh_unavailable))
    }

    private fun scheduleRefresh(current: DriverSession) {
        mainHandler.removeCallbacks(refreshRunnable)
        val seconds = secondsUntilRefresh(current.expireIn, System.currentTimeMillis() / 1000L)
        mainHandler.postDelayed(refreshRunnable, minOf(seconds * 1000L, MAX_DELAY_MILLIS))
    }

    private fun signOut() = endSession(null)

    private fun endSession(message: String?) {
        mainHandler.removeCallbacks(refreshRunnable)
        store.clear()
        session = null
        showLogin(message)
    }

    private fun AccessTokenResponse.toSession(): DriverSession? =
        refreshToken?.let { DriverSession(accessToken, it, expireIn, name, role) }

    private fun alive() = !isFinishing && !isDestroyed

    private fun createContent(): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        gravity = Gravity.CENTER_VERTICAL
        setPadding(
            resources.getDimensionPixelSize(R.dimen.screen_horizontal_padding),
            resources.getDimensionPixelSize(R.dimen.screen_vertical_padding),
            resources.getDimensionPixelSize(R.dimen.screen_horizontal_padding),
            resources.getDimensionPixelSize(R.dimen.screen_vertical_padding),
        )
        setBackgroundResource(R.color.screen_background)
    }

    private fun text(value: String, appearance: Int): TextView = TextView(this).apply {
        setTextAppearance(this@MainActivity, appearance)
        text = value
    }

    private fun contentParams(): LinearLayout.LayoutParams = LinearLayout.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        ViewGroup.LayoutParams.WRAP_CONTENT,
    ).apply { topMargin = resources.getDimensionPixelSize(R.dimen.content_spacing) }

    private companion object {
        const val RETRY_MILLIS = 60_000L
        const val MAX_DELAY_MILLIS = 6L * 60 * 60 * 1000
    }
}
