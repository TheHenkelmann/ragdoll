# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import hashlib
import hmac
import json
import logging
import sqlite3
import time
import urllib.error
import urllib.request
from typing import Any

logger = logging.getLogger(__name__)


def dispatch_ingest_webhooks(
    conn: sqlite3.Connection,
    *,
    release_id: str,
    stage_id: str | None,
    source_id: str,
    status: str,
    chunk_count: int | None,
    error: str | None,
) -> None:
    event = "completed" if status == "completed" else "failed"
    rows = conn.execute(
        """
        SELECT id, type, url, secret, events
        FROM webhooks
        WHERE release_id = ? AND active = 1
        """,
        (release_id,),
    ).fetchall()
    payload = {
        "type": "ingest_status",
        "event": event,
        "source_id": source_id,
        "status": status,
        "release_id": release_id,
        "stage_id": stage_id,
        "chunk_count": chunk_count,
        "error": error,
        "ts": int(time.time()),
    }
    body = json.dumps(payload, separators=(",", ":"))
    for webhook_id, webhook_type, url, secret, events_raw in rows:
        try:
            events = json.loads(events_raw or "[]")
        except json.JSONDecodeError:
            events = []
        if event not in events:
            continue
        _deliver_webhook(conn, webhook_id, url, secret, body, payload)


def _deliver_webhook(
    conn: sqlite3.Connection,
    webhook_id: str,
    url: str,
    secret: str,
    body: str,
    payload: dict[str, Any],
) -> None:
    ts = str(int(time.time()))
    signing_input = f"{ts}.{body}"
    signature = hmac.new(
        secret.encode("utf-8"), signing_input.encode("utf-8"), hashlib.sha256
    ).hexdigest()
    request = urllib.request.Request(
        url,
        data=body.encode("utf-8"),
        headers={
            "Content-Type": "application/json",
            "X-Ragdoll-Signature": f"sha256={signature}",
            "X-Ragdoll-Timestamp": ts,
        },
        method="POST",
    )
    delivery_id = payload.get("source_id", "") + ts
    status_code: int | None = None
    response_body = ""
    error: str | None = None
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            status_code = response.getcode()
            response_body = response.read().decode("utf-8", errors="replace")[:4096]
    except urllib.error.HTTPError as exc:
        status_code = exc.code
        response_body = exc.read().decode("utf-8", errors="replace")[:4096]
        error = str(exc)
    except Exception as exc:  # noqa: BLE001 - webhook failures must not block ingest
        error = str(exc)
        logger.warning("webhook delivery failed for %s: %s", webhook_id, exc)
    conn.execute(
        """
        INSERT INTO webhook_deliveries (id, webhook_id, event, payload, status_code, response, error)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        """,
        (
            delivery_id,
            webhook_id,
            payload.get("event"),
            body,
            status_code,
            response_body,
            error,
        ),
    )
    conn.commit()
