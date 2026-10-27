package com.erocha.hermes.client.driver

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.TextButton
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.Alignment
import androidx.compose.ui.res.stringResource
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.erocha.hermes.client.shared.themeColor

/** A color of the active brand theme (`attrs.xml`), so no screen carries a literal. */
@Composable
internal fun token(attr: Int) = Color(LocalContext.current.themeColor(attr))

/** The server's own reason when it gave one, else [fallback]. */
internal fun failureMessage(host: FleetHost, failure: Throwable, fallback: Int): String =
    (failure as? FleetRefused)?.reason ?: host.context.getString(fallback)

@Composable
internal fun Pill(text: String, color: Color) {
    Text(
        text,
        color = color,
        fontSize = 12.sp,
        fontWeight = FontWeight.SemiBold,
        modifier = Modifier.background(color.copy(alpha = 0.14f), RoundedCornerShape(50)).padding(horizontal = 10.dp, vertical = 3.dp),
    )
}

@Composable
internal fun ErrorBanner(message: String) {
    Text(
        message,
        color = token(R.attr.hermesError),
        fontSize = 14.sp,
        modifier = Modifier.padding(horizontal = 20.dp, vertical = 4.dp),
    )
}

/** Dialogs and fields take their colors from the Material scheme, so it is built from the brand tokens. */
@Composable
internal fun HermesMaterialTheme(content: @Composable () -> Unit) {
    val surface = token(R.attr.hermesSurface)
    MaterialTheme(
        colorScheme = lightColorScheme(
            primary = token(R.attr.hermesAccent),
            onPrimary = Color.White,
            background = token(R.attr.hermesScreenBackground),
            surface = surface,
            surfaceContainerHigh = surface,
            onSurface = token(R.attr.hermesTextPrimary),
            onSurfaceVariant = token(R.attr.hermesTextSecondary),
            outline = token(R.attr.hermesTextSecondary),
            outlineVariant = token(R.attr.hermesOutline),
            error = token(R.attr.hermesError),
        ),
        content = content,
    )
}

@Composable
internal fun BottomNav(items: List<Pair<Int, String>>, selected: Int, onSelect: (Int) -> Unit) {
    NavigationBar(containerColor = token(R.attr.hermesSurface)) {
        items.forEachIndexed { index, (label, icon) ->
            NavigationBarItem(
                selected = selected == index,
                onClick = { onSelect(index) },
                icon = { Text(icon, fontSize = 22.sp) },
                label = { Text(stringResource(label), fontSize = 12.sp) },
                colors = NavigationBarItemDefaults.colors(
                    selectedTextColor = token(R.attr.hermesAccent),
                    unselectedTextColor = token(R.attr.hermesTextSecondary),
                    indicatorColor = token(R.attr.hermesAccent).copy(alpha = 0.16f),
                ),
            )
        }
    }
}

@Composable
internal fun AppHeader(title: String, userName: String, themeLabel: String, onTheme: () -> Unit, onSignOut: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(start = 20.dp, end = 8.dp, top = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(title, color = token(R.attr.hermesTextPrimary), fontSize = 24.sp, fontWeight = FontWeight.Bold)
            Text(userName, color = token(R.attr.hermesTextSecondary), fontSize = 14.sp)
        }
        TextButton(onClick = onTheme) { Text(themeLabel, color = token(R.attr.hermesAccent)) }
        TextButton(onClick = onSignOut) { Text(stringResource(R.string.sign_out_button), color = token(R.attr.hermesAccent)) }
    }
}


@Composable
internal fun ChoiceButton(label: String, selected: Boolean, color: Color, modifier: Modifier, onClick: () -> Unit) {
    val shape = RoundedCornerShape(12.dp)
    if (selected) {
        Button(onClick = onClick, modifier = modifier.height(48.dp), shape = shape, colors = ButtonDefaults.buttonColors(containerColor = color, contentColor = Color.White)) {
            Text(label, fontSize = 14.sp, maxLines = 1)
        }
    } else {
        OutlinedButton(onClick = onClick, modifier = modifier.height(48.dp), shape = shape, border = BorderStroke(1.dp, token(R.attr.hermesOutline))) {
            Text(label, fontSize = 14.sp, maxLines = 1, color = token(R.attr.hermesTextPrimary))
        }
    }
}

