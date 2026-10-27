package com.erocha.hermes.client.tv

import android.app.AlertDialog
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.InputFilter
import android.text.InputType
import android.view.Gravity
import android.view.ViewGroup
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.mutableStateOf
import androidx.lifecycle.Lifecycle
import com.erocha.hermes.client.shared.GarageCallListEntry
import com.erocha.hermes.client.shared.GarageDeviceApi
import com.erocha.hermes.client.shared.GarageMap
import com.erocha.hermes.client.shared.GarageMatrix
import com.erocha.hermes.client.shared.GarageMonitorCards
import com.erocha.hermes.client.shared.GarageMonitorEntry
import com.erocha.hermes.client.shared.HermesTheme
import com.erocha.hermes.client.shared.ThemeStore
import com.erocha.hermes.client.shared.themeColor
import com.erocha.hermes.client.shared.createGarageDeviceHttpClient
import io.ktor.client.HttpClient
import io.ktor.client.plugins.ClientRequestException
import io.ktor.http.HttpStatusCode
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.runBlocking
import java.util.concurrent.Executors

class MainActivity : ComponentActivity() {
    private val mainHandler = Handler(Looper.getMainLooper())
    private val pairingExecutor = Executors.newSingleThreadExecutor()
    private val monitorState = mutableStateOf(GarageMonitorUiState())
    private lateinit var credentialStore: DeviceCredentialStore
    private lateinit var themeStore: ThemeStore
    private lateinit var httpClient: HttpClient
    private var pairingCodeInput: EditText? = null
    private var pairButton: Button? = null
    private var monitorCredential: StoredDeviceCredential? = null
    private var refreshInProgress = false
    private val refreshRunnable = object : Runnable {
        override fun run() {
            monitorCredential?.let(::refreshMonitor)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        themeStore = ThemeStore(this)
        setTheme(themeStore.load().styleRes())
        super.onCreate(savedInstanceState)
        credentialStore = DeviceCredentialStore(this)
        httpClient = createGarageDeviceHttpClient()

        val credential = credentialStore.load()
        if (credential == null) {
            showPairing()
        } else {
            showPaired(credential)
        }
    }

    override fun onDestroy() {
        mainHandler.removeCallbacks(refreshRunnable)
        pairingExecutor.shutdownNow()
        httpClient.close()
        super.onDestroy()
    }

    override fun onStart() {
        super.onStart()
        monitorCredential?.let(::beginMonitoring)
    }

    override fun onStop() {
        mainHandler.removeCallbacks(refreshRunnable)
        super.onStop()
    }

    private fun showPairing(message: String? = null, initialCode: String = "") {
        val content = createContent()
            content.addView(text(getString(R.string.pairing_title), R.style.TextAppearance_Hermes_Title))
            content.addView(text(getString(R.string.pairing_message), R.style.TextAppearance_Hermes_Body))

        val codeInput = EditText(this).apply {
            contentDescription = getString(R.string.pairing_code_hint)
            hint = getString(R.string.pairing_code_hint)
            inputType = InputType.TYPE_CLASS_NUMBER
            filters = arrayOf(InputFilter.LengthFilter(8))
            setSingleLine(true)
            setText(initialCode)
        }
        pairingCodeInput = codeInput
        content.addView(codeInput, contentParams())

        val submit = Button(this).apply {
            text = getString(R.string.pair_button)
            setOnClickListener { submitPairing(codeInput.text.toString()) }
        }
        pairButton = submit
        content.addView(submit, contentParams())
        content.addView(
            Button(this).apply {
                text = getString(R.string.theme_button, themeLabel())
                setOnClickListener { cycleTheme() }
            },
            contentParams(),
        )

        if (message != null) {
            content.addView(text(message, R.style.TextAppearance_Hermes_Error))
        }

        setContentView(content)
        codeInput.requestFocus()
    }

    private fun showPaired(credential: StoredDeviceCredential) {
        pairingCodeInput = null
        pairButton = null
        monitorCredential = credential
        monitorState.value = GarageMonitorUiState()
        setContent {
            GarageMonitorScreen(
                state = monitorState.value,
                onRefresh = { refreshMonitor(credential) },
                onRePair = { confirmRePair() },
                themeLabel = themeLabel(),
                onTheme = { cycleTheme() },
            )
        }
        if (lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) beginMonitoring(credential)
    }

    private fun beginMonitoring(credential: StoredDeviceCredential) {
        mainHandler.removeCallbacks(refreshRunnable)
        refreshMonitor(credential)
    }

    private fun refreshMonitor(credential: StoredDeviceCredential) {
        if (refreshInProgress || isFinishing || isDestroyed) return
        refreshInProgress = true
        mainHandler.removeCallbacks(refreshRunnable)
        monitorState.value = monitorState.value.copy(isRefreshing = true, errorMessage = null)
        pairingExecutor.execute {
            try {
                val api = GarageDeviceApi(httpClient, BuildConfig.API_BASE_URL)
                val result = runBlocking {
                    coroutineScope {
                        val queue = async { api.monitor(credential.accessToken) }
                        val cards = async { api.monitorCards(credential.accessToken) }
                        val calls = async { api.callList(credential.accessToken) }
                        val matrix = async { api.monitorMatrix(credential.accessToken) }
                        // The map depends on the GPS provider; its failure must not blank the rest of the monitor.
                        val map = async { runCatching { api.monitorMap(credential.accessToken) }.getOrNull() }
                        MonitorSnapshot(queue.await(), cards.await(), calls.await(), matrix.await(), map.await())
                    }
                }
                mainHandler.post {
                    refreshInProgress = false
                    if (isMonitorVisible(credential)) {
                        monitorState.value = GarageMonitorUiState(
                            queue = result.queue,
                            cards = result.cards.cards,
                            cardsNotShown = result.cards.notShown,
                            callList = result.calls,
                            matrix = result.matrix,
                            map = result.map,
                            lastUpdatedAtMillis = System.currentTimeMillis(),
                            isRefreshing = false,
                        )
                        scheduleRefresh()
                    }
                }
            } catch (failure: ClientRequestException) {
                if (failure.response.status == HttpStatusCode.Unauthorized) {
                    mainHandler.post {
                        refreshInProgress = false
                        if (isMonitorVisible(credential)) {
                            credentialStore.clear()
                            monitorCredential = null
                            mainHandler.removeCallbacks(refreshRunnable)
                            showPairing(getString(R.string.device_credential_invalid))
                        }
                    }
                } else {
                    showMonitorUnavailable(credential)
                }
            } catch (_: Exception) {
                showMonitorUnavailable(credential)
            }
        }
    }

    private fun showMonitorUnavailable(credential: StoredDeviceCredential) {
        mainHandler.post {
            refreshInProgress = false
            if (isMonitorVisible(credential)) {
                monitorState.value = monitorState.value.copy(
                    isRefreshing = false,
                    errorMessage = getString(R.string.monitor_connection_error),
                )
                scheduleRefresh()
            }
        }
    }

    private fun isMonitorVisible(credential: StoredDeviceCredential): Boolean =
        !isFinishing && !isDestroyed &&
            lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED) &&
            monitorCredential?.accessToken == credential.accessToken

    private fun scheduleRefresh() {
        mainHandler.removeCallbacks(refreshRunnable)
        if (lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) {
            mainHandler.postDelayed(refreshRunnable, MONITOR_REFRESH_INTERVAL_MILLIS)
        }
    }

    private fun submitPairing(rawCode: String) {
        val code = rawCode.trim()
        if (code.isEmpty()) {
            showPairing(getString(R.string.pairing_code_required), rawCode)
            return
        }

        pairButton?.isEnabled = false
        pairButton?.text = getString(R.string.pairing_in_progress)
        pairingExecutor.execute {
            try {
                val api = GarageDeviceApi(httpClient, BuildConfig.API_BASE_URL)
                val credential = runBlocking { api.pair(code) }
                val stored = StoredDeviceCredential(
                    accessToken = credential.accessToken,
                    deviceUuid = credential.deviceUuid,
                    deviceName = credential.deviceName,
                    expiresAt = credential.expiresAt,
                )
                credentialStore.save(stored)
                mainHandler.post {
                    if (!isFinishing && !isDestroyed) showPaired(stored)
                }
            } catch (failure: ClientRequestException) {
                val message = if (failure.response.status == HttpStatusCode.Unauthorized) {
                    getString(R.string.pairing_code_invalid)
                } else {
                    getString(R.string.pairing_failed)
                }
                mainHandler.post {
                    if (!isFinishing && !isDestroyed) showPairing(message, code)
                }
            } catch (_: Exception) {
                mainHandler.post {
                    if (!isFinishing && !isDestroyed) {
                        showPairing(getString(R.string.pairing_failed), code)
                    }
                }
            }
        }
    }

    private fun confirmRePair() {
        AlertDialog.Builder(this)
            .setTitle(R.string.repair_confirmation_title)
            .setMessage(R.string.repair_confirmation_message)
            .setNegativeButton(R.string.cancel_button, null)
            .setPositiveButton(R.string.confirm_button) { _, _ ->
                credentialStore.clear()
                monitorCredential = null
                mainHandler.removeCallbacks(refreshRunnable)
                showPairing()
            }
            .show()
    }

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

    /** Cycles the brand theme; recreating the activity re-inflates the pairing screen and the monitor under it. */
    private fun cycleTheme() {
        themeStore.save(themeStore.load().next())
        recreate()
    }

    private fun themeLabel() = getString(
        when (themeStore.load()) {
            HermesTheme.TRANSMEGA -> R.string.theme_transmega
            HermesTheme.HERMES -> R.string.theme_hermes
        },
    )

    private fun HermesTheme.styleRes() = when (this) {
        HermesTheme.TRANSMEGA -> R.style.Theme_Hermes_Transmega
        HermesTheme.HERMES -> R.style.Theme_Hermes_Default
    }

    private fun text(value: String, appearance: Int): TextView = TextView(this).apply {
        setTextAppearance(this@MainActivity, appearance)
        text = value
    }

    private fun contentParams(): LinearLayout.LayoutParams = LinearLayout.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        ViewGroup.LayoutParams.WRAP_CONTENT,
    ).apply {
        topMargin = resources.getDimensionPixelSize(R.dimen.content_spacing)
    }

    private data class MonitorSnapshot(
        val queue: List<GarageMonitorEntry>,
        val cards: GarageMonitorCards,
        val calls: List<GarageCallListEntry>,
        val matrix: GarageMatrix,
        val map: GarageMap?,
    )

    private companion object {
        const val MONITOR_REFRESH_INTERVAL_MILLIS = 30_000L
    }
}