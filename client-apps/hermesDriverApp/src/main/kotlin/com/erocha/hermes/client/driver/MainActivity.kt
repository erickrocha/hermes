package com.erocha.hermes.client.driver

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.compose.ui.platform.ComposeView
import com.erocha.hermes.client.shared.AccessTokenResponse
import com.erocha.hermes.client.shared.AppAccess
import com.erocha.hermes.client.shared.AuthApi
import com.erocha.hermes.client.shared.DriverSession
import com.erocha.hermes.client.shared.FleetApi
import com.erocha.hermes.client.shared.GarageApi
import com.erocha.hermes.client.shared.WorkshopApi
import com.erocha.hermes.client.shared.HermesTheme
import com.erocha.hermes.client.shared.ThemeStore
import com.erocha.hermes.client.shared.access
import com.erocha.hermes.client.shared.apiError
import com.erocha.hermes.client.shared.createHermesHttpClient
import com.erocha.hermes.client.shared.secondsUntilRefresh
import com.erocha.hermes.client.shared.themeColor
import io.ktor.client.HttpClient
import io.ktor.client.plugins.ClientRequestException
import io.ktor.client.plugins.ResponseException
import io.ktor.http.HttpStatusCode
import kotlinx.coroutines.runBlocking
import java.util.concurrent.Executors

/**
 * Driver app: sign in through the platform's existing auth, keep the session encrypted, renew it, sign out (C-017),
 * and host the signed-in screens (driver, mechanic) in Compose. Every network call runs on one background thread, so a
 * scheduled renewal and a renewal after a `401` can never overlap.
 */
class MainActivity : ComponentActivity(), FleetHost {
    private val mainHandler = Handler(Looper.getMainLooper())
    private val executor = Executors.newSingleThreadExecutor()
    private lateinit var store: DriverSessionStore
    private lateinit var themeStore: ThemeStore
    private lateinit var httpClient: HttpClient
    @Volatile private var session: DriverSession? = null
    private var loginButton: Button? = null
    private val refreshRunnable = Runnable { refreshNow() }
    private val authApi get() = AuthApi(httpClient, BuildConfig.API_BASE_URL)
    private val fleetApi get() = FleetApi(httpClient, BuildConfig.API_BASE_URL)
    private val garageApi get() = GarageApi(httpClient, BuildConfig.API_BASE_URL)
    private val workshopApi get() = WorkshopApi(httpClient, BuildConfig.API_BASE_URL)

    override val context: Context get() = this
    override val userUuid: String get() = session?.uuid.orEmpty()
    override val access: AppAccess get() = session?.access ?: AppAccess.NONE

    override fun onCreate(savedInstanceState: Bundle?) {
        themeStore = ThemeStore(this)
        setTheme(themeStore.load().styleRes())
        super.onCreate(savedInstanceState)
        store = DriverSessionStore(this)
        httpClient = createHermesHttpClient(acceptLanguage = "pt-BR")
        val saved = store.load()
        session = saved
        if (saved == null) showLogin() else showAuthenticated(saved)
        // A session saved by the shell has no user uuid yet; a renewal fills it in and starts the fleet screens.
        if (saved != null && saved.uuid.isBlank()) refreshNow()
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
        content.addView(themeButton(), contentParams())
        if (message != null) content.addView(text(message, R.style.TextAppearance_Hermes_Error), contentParams())
        setContentView(content)
    }

    /**
     * The signed-in home, laid out by Compose: the garage services for whoever may perform them, the driver's own
     * information otherwise (`C-035`: decided by the session's grants, no longer by a role).
     */
    private fun showAuthenticated(current: DriverSession) {
        setContentView(
            ComposeView(this).apply {
                setContent {
                    val themeText = getString(R.string.theme_button, themeLabel(themeStore.load()))
                    val switchTheme = { themeStore.save(themeStore.load().next()); recreate() }
                    if (current.access.mechanic) {
                        MechanicApp(this@MainActivity, current.name, themeText, switchTheme) { signOut() }
                    } else {
                        val profile = current.profileName ?: getString(R.string.profile_driver)
                        DriverApp(this@MainActivity, current.name, getString(R.string.authenticated_profile, profile), themeText, switchTheme) { signOut() }
                    }
                }
            },
        )
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
        if (next == null || !next.access.admitted) {
            // Interface courtesy only: nothing is stored; the server remains the authority.
            val message = if (next != null) R.string.not_a_driver else R.string.login_failed
            showLogin(getString(message), email)
            return
        }
        store.save(next)
        session = next
        showAuthenticated(next)
        scheduleRefresh(next)
    }

    /** The scheduled renewal. It reads the latest session on the worker thread, never a stale copy. */
    private fun refreshNow() {
        if (!alive()) return
        executor.execute {
            val current = session ?: return@execute
            try {
                renewBlocking(current, force = true)
            } catch (ended: SessionEnded) {
                mainHandler.post { if (alive()) endSession(ended.messageRes?.let(::getString)) }
            } catch (_: Exception) {
                mainHandler.post { if (alive()) retryLater(current) }
            }
        }
    }

    /**
     * Renews the session on the worker thread. Unless [force], a session already renewed since [stale] was read is
     * reused: the refresh token rotates on every use, so a second renewal with the old one would be refused.
     */
    private fun renewBlocking(stale: DriverSession, force: Boolean = false): DriverSession {
        val latest = session ?: throw SessionEnded(null)
        if (!force && latest.accessToken != stale.accessToken) return latest
        val response = try {
            runBlocking { authApi.refresh(latest.refreshToken) }
        } catch (failure: ClientRequestException) {
            if (failure.response.status == HttpStatusCode.Unauthorized) throw SessionEnded(R.string.session_expired)
            throw failure
        }
        val next = response.toSession()
        if (next == null || !next.access.admitted) {
            // Usable tokens without the grants this app needs is a withdrawn access; no tokens is a bad response.
            throw SessionEnded(if (next != null) R.string.not_a_driver else R.string.session_expired)
        }
        store.save(next) // the backend issues a new refresh token on every refresh: replace what is stored
        session = next
        mainHandler.post { if (alive()) scheduleRefresh(next) }
        return next
    }

    override fun <T> run(work: suspend (api: FleetApi, accessToken: String) -> T, onDone: (Result<T>) -> Unit) =
        execute({ token -> work(fleetApi, token) }, onDone)

    override fun <T> runGarage(work: suspend (api: GarageApi, accessToken: String) -> T, onDone: (Result<T>) -> Unit) =
        execute({ token -> work(garageApi, token) }, onDone)

    override fun <T> runWorkshop(work: suspend (api: WorkshopApi, accessToken: String) -> T, onDone: (Result<T>) -> Unit) =
        execute({ token -> work(workshopApi, token) }, onDone)

    private fun <T> execute(call: suspend (accessToken: String) -> T, onDone: (Result<T>) -> Unit) {
        executor.execute {
            val outcome: Result<T> = try {
                val first = session ?: throw SessionEnded(null)
                try {
                    Result.success(runBlocking { call(first.accessToken) })
                } catch (failure: ClientRequestException) {
                    if (failure.response.status != HttpStatusCode.Unauthorized) throw failure
                    val renewed = renewBlocking(first)
                    Result.success(runBlocking { call(renewed.accessToken) })
                }
            } catch (ended: SessionEnded) {
                mainHandler.post { if (alive()) endSession(ended.messageRes?.let(::getString)) }
                return@execute
            } catch (refused: ResponseException) {
                // 4xx (ClientRequestException) and 5xx (ServerResponseException) both carry the platform's reason.
                val reason = runBlocking { refused.apiError() }
                Result.failure(FleetRefused(refused.response.status.value, reason?.message, reason?.errorKey.orEmpty()))
            } catch (other: Exception) {
                Result.failure(FleetUnreachable(other))
            }
            mainHandler.post { if (alive()) onDone(outcome) }
        }
    }

    /** Network trouble keeps the session; try again shortly. */
    private fun retryLater(@Suppress("UNUSED_PARAMETER") current: DriverSession) {
        mainHandler.removeCallbacks(refreshRunnable)
        mainHandler.postDelayed(refreshRunnable, RETRY_MILLIS)
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
        refreshToken?.let { DriverSession(accessToken, it, expireIn, name, permissions, uuid, profileName) }

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
        setBackgroundColor(themeColor(R.attr.hermesScreenBackground))
    }

    /** Cycles the brand theme; recreating the activity re-inflates every screen under the new one. */
    private fun themeButton() = Button(this).apply {
        text = getString(R.string.theme_button, themeLabel(themeStore.load()))
        setOnClickListener {
            themeStore.save(themeStore.load().next())
            recreate()
        }
    }

    private fun themeLabel(theme: HermesTheme) = getString(
        when (theme) {
            HermesTheme.TRANSMEGA -> R.string.theme_transmega
            HermesTheme.HERMES -> R.string.theme_hermes
        },
    )

    private fun HermesTheme.styleRes() = when (this) {
        HermesTheme.TRANSMEGA -> R.style.Theme_Hermes_Transmega
        HermesTheme.HERMES -> R.style.Theme_Hermes_Default
    }

    override fun openMap(latitude: Double, longitude: Double): Boolean = try {
        startActivity(Intent(Intent.ACTION_VIEW, Uri.parse("geo:$latitude,$longitude?q=$latitude,$longitude")))
        true
    } catch (_: ActivityNotFoundException) {
        false
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
