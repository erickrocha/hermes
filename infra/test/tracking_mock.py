#!/usr/bin/env python3
import base64
import json
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Lock

POSITIONS = []
POSITIONS_LOCK = Lock()
AUTHORIZATION = "Basic " + base64.b64encode(b"test:test").decode()


class Handler(BaseHTTPRequestHandler):
    def _json(self, status, value):
        body = json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/health":
            return self._json(200, {"ok": True})
        if self.path != "/api/positions":
            return self._json(404, {"error": "not found"})
        if self.headers.get("Authorization") != AUTHORIZATION:
            return self._json(401, {"error": "unauthorized"})

        now = datetime.now(timezone.utc)
        with POSITIONS_LOCK:
            positions = list(POSITIONS)
        result = []
        for position in positions:
            age = int(position.get("ageSeconds", 0))
            reported_at = now - timedelta(seconds=age)
            result.append({
                "deviceId": position["deviceId"],
                "latitude": position["latitude"],
                "longitude": position["longitude"],
                "valid": position.get("valid", True),
                "serverTime": reported_at.isoformat(),
                "attributes": {
                    "ignition": position.get("ignition", False),
                    **({"totalDistance": position["odometerMeters"]}
                       if "odometerMeters" in position else {}),
                },
            })
        return self._json(200, result)

    def do_POST(self):
        if self.path != "/control/positions":
            return self._json(404, {"error": "not found"})
        try:
            length = int(self.headers.get("Content-Length", "0"))
            positions = json.loads(self.rfile.read(length))
            if not isinstance(positions, list):
                raise ValueError("positions must be a list")
            for position in positions:
                if not isinstance(position, dict) or not {"deviceId", "latitude", "longitude"} <= position.keys():
                    raise ValueError("each position needs deviceId, latitude, and longitude")
        except (json.JSONDecodeError, ValueError) as error:
            return self._json(400, {"error": str(error)})
        with POSITIONS_LOCK:
            POSITIONS[:] = positions
        return self._json(200, {"positions": len(positions)})

    def log_message(self, _format, *_args):
        return


ThreadingHTTPServer(("0.0.0.0", 8080), Handler).serve_forever()