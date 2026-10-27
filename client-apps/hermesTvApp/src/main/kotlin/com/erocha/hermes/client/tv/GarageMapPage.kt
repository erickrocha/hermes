package com.erocha.hermes.client.tv

import android.graphics.BitmapFactory
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.erocha.hermes.client.shared.GarageMap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import java.net.HttpURLConnection
import java.net.URL
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.ln
import kotlin.math.log2
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.roundToInt
import kotlin.math.sin
import kotlin.math.sqrt

private const val TILE = 256
private const val MIN_ZOOM = 3
private const val MAX_ZOOM = 17
private const val EARTH_METERS_PER_PIXEL = 156543.03392
private const val EARTH_RADIUS_METERS = 6_371_000.0

// shortcut: public OpenStreetMap tiles (their usage policy allows light use such as one display); upgrade to a
// tile provider with a key or our own tile server if more displays are added.
private const val TILE_URL = "https://tile.openstreetmap.org"
private const val USER_AGENT = "HermesGarageMonitor/0.1 (+https://hermes.erochasolucoes.com.br)"
private const val TILE_CACHE_LIMIT = 300

// Snapshot state so the canvas redraws by itself when a tile arrives.
private val tileCache = mutableStateMapOf<String, ImageBitmap>()

private data class WorldPoint(val x: Double, val y: Double)

private fun project(latitude: Double, longitude: Double, zoom: Int): WorldPoint {
    val scale = TILE * 2.0.pow(zoom)
    val sin = sin(Math.toRadians(latitude.coerceIn(-85.0, 85.0)))
    return WorldPoint(
        (longitude + 180.0) / 360.0 * scale,
        (0.5 - ln((1 + sin) / (1 - sin)) / (4 * PI)) * scale,
    )
}

private fun distanceKm(lat1: Double, lon1: Double, lat2: Double, lon2: Double): Double {
    val dLat = Math.toRadians(lat2 - lat1)
    val dLon = Math.toRadians(lon2 - lon1)
    val a = sin(dLat / 2).pow(2) + cos(Math.toRadians(lat1)) * cos(Math.toRadians(lat2)) * sin(dLon / 2).pow(2)
    return 2 * EARTH_RADIUS_METERS * kotlin.math.asin(sqrt(a)) / 1000.0
}

/** The zoom at which [radiusKm] around [latitude] just fits in half of the shorter side of the canvas. */
private fun zoomForRadius(latitude: Double, radiusKm: Double, halfSidePx: Double): Int {
    val metersPerPixelAtZoom0 = EARTH_METERS_PER_PIXEL * cos(Math.toRadians(latitude))
    return floor(log2(metersPerPixelAtZoom0 * halfSidePx / (radiusKm * 1000.0))).toInt().coerceIn(MIN_ZOOM, MAX_ZOOM)
}

private fun fetchTile(zoom: Int, x: Int, y: Int): ImageBitmap? = runCatching {
    val connection = URL("$TILE_URL/$zoom/$x/$y.png").openConnection() as HttpURLConnection
    connection.setRequestProperty("User-Agent", USER_AGENT)
    connection.connectTimeout = 8_000
    connection.readTimeout = 8_000
    try {
        connection.inputStream.use { BitmapFactory.decodeStream(it)?.asImageBitmap() }
    } finally {
        connection.disconnect()
    }
}.onFailure { android.util.Log.w("GarageMap", "tile $zoom/$x/$y failed", it) }.getOrNull()

@Composable
internal fun MapPage(
    map: GarageMap?,
    textColor: Color,
    mutedText: Color,
    surface: Color,
    accent: Color,
    warning: Color,
    ok: Color,
    outline: Color,
) {
    if (map == null) {
        EmptyState(stringResource(R.string.monitor_map_unavailable), warning)
        return
    }
    if (map.vehicles.isEmpty()) {
        EmptyState(stringResource(R.string.monitor_empty_map), mutedText)
        return
    }
    val baseLatitude = map.baseLatitude
    val baseLongitude = map.baseLongitude
    val hasBase = baseLatitude != null && baseLongitude != null
    val outside = if (hasBase) {
        map.vehicles.count { distanceKm(baseLatitude, baseLongitude, it.latitude, it.longitude) > map.mapRadiusKm }
    } else {
        0
    }
    val stale = map.vehicles.count { !it.live }
    val measurer = rememberTextMeasurer()
    var zoomDelta by rememberSaveable { mutableIntStateOf(0) }

    Column(modifier = Modifier.fillMaxSize()) {
        Text(
            buildString {
                append(stringResource(R.string.monitor_map_summary, map.vehicles.size, stale))
                if (outside > 0) append(" · ").append(stringResource(R.string.monitor_map_outside, outside, map.mapRadiusKm.roundToInt()))
            },
            color = mutedText,
            fontSize = 13.sp,
        )
        BoxWithConstraints(modifier = Modifier.weight(1f).fillMaxSize().padding(top = 6.dp).clipToBounds()) {
            val density = androidx.compose.ui.platform.LocalDensity.current
            val width = with(density) { maxWidth.toPx() }.toDouble()
            val height = with(density) { maxHeight.toPx() }.toDouble()

            // Centre: the garage. Without a base configured, the middle of the fleet.
            val centerLatitude = baseLatitude ?: map.vehicles.map { it.latitude }.average()
            val centerLongitude = baseLongitude ?: map.vehicles.map { it.longitude }.average()
            val fitZoom = zoomForRadius(centerLatitude, map.mapRadiusKm, min(width, height) / 2)
            val zoom = (fitZoom + zoomDelta).coerceIn(MIN_ZOOM, MAX_ZOOM)
            val center = project(centerLatitude, centerLongitude, zoom)
            val originX = center.x - width / 2
            val originY = center.y - height / 2
            val tilesX = floor(originX / TILE).toInt()..floor((originX + width) / TILE).toInt()
            val tilesY = floor(originY / TILE).toInt()..floor((originY + height) / TILE).toInt()
            val limit = 2.0.pow(zoom).toInt()
            val wanted = tilesX.flatMap { x -> tilesY.map { y -> Triple(zoom, x, y) } }
                .filter { (_, x, y) -> x in 0 until limit && y in 0 until limit }
            val metersPerPixel = EARTH_METERS_PER_PIXEL * cos(Math.toRadians(centerLatitude)) / 2.0.pow(zoom)

            val garageLabel = stringResource(R.string.monitor_map_garage)

            LaunchedEffect(wanted) {
                if (tileCache.size > TILE_CACHE_LIMIT) tileCache.clear()
                val missing = wanted.filter { (z, x, y) -> !tileCache.containsKey("$z/$x/$y") }
                coroutineScope {
                    missing.map { (z, x, y) ->
                        async(Dispatchers.IO) { fetchTile(z, x, y)?.let { tileCache["$z/$x/$y"] = it } }
                    }.awaitAll()
                }
            }

            Canvas(modifier = Modifier.fillMaxSize()) {
                drawRect(surface)
                wanted.forEach { (z, x, y) ->
                    tileCache["$z/$x/$y"]?.let { tile ->
                        drawImage(
                            tile,
                            dstOffset = IntOffset((x * TILE - originX).toInt(), (y * TILE - originY).toInt()),
                            dstSize = IntSize(TILE, TILE),
                        )
                    }
                }
                // Dim the light tiles so the overlay reads from across the garage.
                drawRect(Color.Black.copy(alpha = 0.35f), size = Size(size.width, size.height))

                val middle = Offset((width / 2).toFloat(), (height / 2).toFloat())
                // The service radius the tenant configured.
                drawCircle(outline, (map.mapRadiusKm * 1000.0 / metersPerPixel).toFloat(), middle, style = Stroke(width = 2f))
                if (hasBase) {
                    val geofence = max((map.baseRadiusMeters / metersPerPixel).toFloat(), 8f)
                    drawCircle(ok.copy(alpha = 0.25f), geofence, middle)
                    drawCircle(ok, geofence, middle, style = Stroke(width = 3f))
                    drawCircle(ok, 6f, middle)
                    val garage = measurer.measure(
                        garageLabel,
                        TextStyle(color = textColor, fontSize = 12.sp, fontWeight = FontWeight.Bold),
                    )
                    drawText(garage, topLeft = Offset(middle.x + geofence + 6f, middle.y - garage.size.height / 2f))
                }
                map.vehicles.forEach { vehicle ->
                    val p = project(vehicle.latitude, vehicle.longitude, zoom)
                    val point = Offset((p.x - originX).toFloat(), (p.y - originY).toFloat())
                    if (point.x < -40f || point.y < -40f || point.x > size.width + 40f || point.y > size.height + 40f) return@forEach
                    val color = if (vehicle.live) accent else mutedText
                    drawCircle(Color.Black, 10f, point)
                    drawCircle(color, 7f, point)
                    val label = measurer.measure(
                        vehicle.prefix,
                        TextStyle(color = textColor, fontSize = 12.sp, fontWeight = FontWeight.Bold),
                    )
                    val labelOffset = Offset(
                        min(max(point.x + 14f, 4f), size.width - label.size.width - 8f),
                        min(max(point.y - label.size.height / 2f, 4f), size.height - label.size.height - 4f),
                    )
                    drawRect(
                        surface.copy(alpha = 0.85f),
                        topLeft = Offset(labelOffset.x - 4f, labelOffset.y - 1f),
                        size = Size(label.size.width + 8f, label.size.height + 2f),
                    )
                    drawText(label, topLeft = labelOffset)
                }
            }
            Column(
                modifier = Modifier.align(Alignment.TopEnd).padding(8.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                ZoomButton("+", stringResource(R.string.monitor_map_zoom_in), accent, textColor, outline, surface, enabled = zoom < MAX_ZOOM) { zoomDelta++ }
                ZoomButton("−", stringResource(R.string.monitor_map_zoom_out), accent, textColor, outline, surface, enabled = zoom > MIN_ZOOM) { zoomDelta-- }
            }
            Box(modifier = Modifier.align(Alignment.BottomEnd).padding(6.dp)) {
                Text(stringResource(R.string.monitor_map_attribution), color = textColor, fontSize = 10.sp)
            }
        }
    }
}

@Composable
private fun ZoomButton(
    symbol: String,
    description: String,
    accent: Color,
    textColor: Color,
    outline: Color,
    surface: Color,
    enabled: Boolean,
    onClick: () -> Unit,
) {
    var focused by remember { mutableStateOf(false) }
    Box(
        modifier = Modifier
            .size(44.dp)
            .onFocusChanged { focused = it.isFocused }
            .focusable(enabled = enabled)
            .clickable(enabled = enabled, onClickLabel = description, onClick = onClick)
            .background(if (focused) accent.copy(alpha = 0.3f) else surface.copy(alpha = 0.9f), RoundedCornerShape(6.dp))
            .border(if (focused) 2.dp else 1.dp, if (focused) accent else outline, RoundedCornerShape(6.dp)),
        contentAlignment = Alignment.Center,
    ) {
        Text(symbol, color = if (enabled) textColor else textColor.copy(alpha = 0.4f), fontSize = 24.sp, fontWeight = FontWeight.Bold)
    }
}
