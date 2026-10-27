package com.erocha.hermes.client.tv

import android.graphics.BitmapFactory
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.res.stringResource
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
import java.util.concurrent.ConcurrentHashMap
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.ln
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.tan

private const val TILE = 256
private const val MIN_ZOOM = 3
private const val MAX_ZOOM = 17
private const val MARGIN_PX = 140.0

// shortcut: public OpenStreetMap tiles (their usage policy allows light use such as one display); upgrade to a
// tile provider with a key or our own tile server if more displays are added.
private const val TILE_URL = "https://tile.openstreetmap.org"
private const val USER_AGENT = "HermesGarageMonitor/0.1 (+https://hermes.erochasolucoes.com.br)"
private const val TILE_CACHE_LIMIT = 300

private val tileCache = ConcurrentHashMap<String, ImageBitmap>()

private data class WorldPoint(val x: Double, val y: Double)

private fun project(latitude: Double, longitude: Double, zoom: Int): WorldPoint {
    val scale = TILE * 2.0.pow(zoom)
    val sin = kotlin.math.sin(Math.toRadians(latitude.coerceIn(-85.0, 85.0)))
    return WorldPoint(
        (longitude + 180.0) / 360.0 * scale,
        (0.5 - ln((1 + sin) / (1 - sin)) / (4 * PI)) * scale,
    )
}

/** The largest zoom at which every point (and [MARGIN_PX] around them) fits in the canvas. */
private fun fitZoom(points: List<Pair<Double, Double>>, width: Double, height: Double): Int {
    for (zoom in MAX_ZOOM downTo MIN_ZOOM) {
        val projected = points.map { project(it.first, it.second, zoom) }
        val spanX = projected.maxOf { it.x } - projected.minOf { it.x }
        val spanY = projected.maxOf { it.y } - projected.minOf { it.y }
        if (spanX <= width - 2 * MARGIN_PX && spanY <= height - 2 * MARGIN_PX) return zoom
    }
    return MIN_ZOOM
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
}.getOrNull()

@Composable
internal fun MapPage(
    map: GarageMap?,
    textColor: Color,
    mutedText: Color,
    surface: Color,
    accent: Color,
    warning: Color,
    ok: Color,
) {
    if (map == null) {
        EmptyState(stringResource(R.string.monitor_map_unavailable), warning)
        return
    }
    if (map.vehicles.isEmpty()) {
        EmptyState(stringResource(R.string.monitor_empty_map), mutedText)
        return
    }
    val base = map.baseLatitude?.let { latitude -> map.baseLongitude?.let { latitude to it } }
    val points = map.vehicles.map { it.latitude to it.longitude } + listOfNotNull(base)
    val stale = map.vehicles.count { !it.live }
    val measurer = rememberTextMeasurer()
    var tilesLoaded by remember { mutableIntStateOf(0) }

    Column(modifier = Modifier.fillMaxSize()) {
        Text(
            stringResource(R.string.monitor_map_summary, map.vehicles.size, stale),
            color = mutedText,
            fontSize = 16.sp,
        )
        BoxWithConstraints(modifier = Modifier.weight(1f).fillMaxSize().padding(top = 8.dp)) {
            val density = androidx.compose.ui.platform.LocalDensity.current
            val width = with(density) { maxWidth.toPx() }.toDouble()
            val height = with(density) { maxHeight.toPx() }.toDouble()
            val zoom = fitZoom(points, width, height)
            val projected = points.map { project(it.first, it.second, zoom) }
            val centerX = (projected.maxOf { it.x } + projected.minOf { it.x }) / 2
            val centerY = (projected.maxOf { it.y } + projected.minOf { it.y }) / 2
            val originX = centerX - width / 2
            val originY = centerY - height / 2
            val tilesX = floor(originX / TILE).toInt()..floor((originX + width) / TILE).toInt()
            val tilesY = floor(originY / TILE).toInt()..floor((originY + height) / TILE).toInt()
            val limit = 2.0.pow(zoom).toInt()
            val wanted = tilesX.flatMap { x -> tilesY.map { y -> Triple(zoom, x, y) } }
                .filter { (_, x, y) -> x in 0 until limit && y in 0 until limit }

            LaunchedEffect(wanted) {
                if (tileCache.size > TILE_CACHE_LIMIT) tileCache.clear()
                val missing = wanted.filter { (z, x, y) -> !tileCache.containsKey("$z/$x/$y") }
                coroutineScope {
                    missing.map { (z, x, y) ->
                        async(Dispatchers.IO) { fetchTile(z, x, y)?.let { tileCache["$z/$x/$y"] = it } }
                    }.awaitAll()
                }
                if (missing.isNotEmpty()) tilesLoaded++
            }

            Canvas(modifier = Modifier.fillMaxSize()) {
                @Suppress("UNUSED_EXPRESSION") tilesLoaded // read here so the canvas redraws when tiles arrive
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

                if (base != null) {
                    val b = project(base.first, base.second, zoom)
                    val center = Offset((b.x - originX).toFloat(), (b.y - originY).toFloat())
                    val metersPerPixel = 156543.03392 * cos(Math.toRadians(base.first)) / 2.0.pow(zoom)
                    val radius = max((map.baseRadiusMeters / metersPerPixel).toFloat(), 14f)
                    drawCircle(ok.copy(alpha = 0.18f), radius, center)
                    drawCircle(ok, radius, center, style = Stroke(width = 3f))
                }
                map.vehicles.forEach { vehicle ->
                    val p = project(vehicle.latitude, vehicle.longitude, zoom)
                    val center = Offset((p.x - originX).toFloat(), (p.y - originY).toFloat())
                    val color = if (vehicle.live) accent else mutedText
                    drawCircle(Color.Black, 15f, center)
                    drawCircle(color, 11f, center)
                    val label = measurer.measure(
                        vehicle.prefix,
                        TextStyle(color = textColor, fontSize = 18.sp, fontWeight = FontWeight.Bold),
                    )
                    val labelOffset = Offset(
                        min(max(center.x + 20f, 4f), size.width - label.size.width - 12f),
                        min(max(center.y - label.size.height / 2f, 4f), size.height - label.size.height - 4f),
                    )
                    drawRect(
                        surface.copy(alpha = 0.85f),
                        topLeft = Offset(labelOffset.x - 6f, labelOffset.y - 2f),
                        size = Size(label.size.width + 12f, label.size.height + 4f),
                    )
                    drawText(label, topLeft = labelOffset)
                }
            }
            Box(modifier = Modifier.align(Alignment.BottomEnd).padding(8.dp)) {
                Text(stringResource(R.string.monitor_map_attribution), color = textColor, fontSize = 12.sp)
            }
        }
    }
}
