# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import json
import sqlite3
import uuid
from io import BytesIO
from unittest.mock import MagicMock, patch

import pytest
import urllib.error

from ragdoll_worker.webhooks import dispatch_ingest_webhooks

RELEASE_ID = "00000000-0000-0000-0000-000000000001"


def _insert_webhook(
    conn: sqlite3.Connection,
    *,
    url: str = "https://example.com/hook",
    secret: str = "whsec-test",
    events: list[str] | None = None,
    active: int = 1,
) -> str:
    webhook_id = str(uuid.uuid4())
    conn.execute(
        """
        INSERT INTO webhooks (id, release_id, type, url, secret, events, active)
        VALUES (?, ?, 'ingest_status', ?, ?, ?, ?)
        """,
        (
            webhook_id,
            RELEASE_ID,
            url,
            secret,
            json.dumps(events if events is not None else ["completed", "failed"]),
            active,
        ),
    )
    conn.commit()
    return webhook_id


def _delivery_rows(conn: sqlite3.Connection, webhook_id: str) -> list[tuple]:
    return conn.execute(
        "SELECT webhook_id, event, status_code, error FROM webhook_deliveries WHERE webhook_id = ?",
        (webhook_id,),
    ).fetchall()


@patch("ragdoll_worker.webhooks.urllib.request.urlopen")
def test_dispatch_delivers_completed_event(mock_urlopen: MagicMock, worker_db) -> None:
    webhook_id = _insert_webhook(worker_db.conn)
    response = MagicMock()
    response.__enter__ = MagicMock(return_value=response)
    response.__exit__ = MagicMock(return_value=False)
    response.getcode.return_value = 200
    response.read.return_value = b'{"ok":true}'
    mock_urlopen.return_value = response

    dispatch_ingest_webhooks(
        worker_db.conn,
        release_id=RELEASE_ID,
        stage_id=None,
        source_id="source-1",
        status="completed",
        chunk_count=3,
        error=None,
    )

    rows = _delivery_rows(worker_db.conn, webhook_id)
    assert len(rows) == 1
    assert rows[0][0] == webhook_id
    assert rows[0][1] == "completed"
    assert rows[0][2] == 200
    assert rows[0][3] is None

    request = mock_urlopen.call_args[0][0]
    assert request.get_full_url() == "https://example.com/hook"
    assert request.get_method() == "POST"
    header_map = dict(request.header_items())
    assert header_map["Content-type"] == "application/json"
    assert header_map["X-ragdoll-signature"].startswith("sha256=")
    assert header_map["X-ragdoll-timestamp"]


@patch("ragdoll_worker.webhooks.urllib.request.urlopen")
def test_dispatch_uses_failed_event_for_non_completed_status(mock_urlopen: MagicMock, worker_db) -> None:
    webhook_id = _insert_webhook(worker_db.conn, events=["failed"])
    response = MagicMock()
    response.__enter__ = MagicMock(return_value=response)
    response.__exit__ = MagicMock(return_value=False)
    response.getcode.return_value = 204
    response.read.return_value = b""
    mock_urlopen.return_value = response

    dispatch_ingest_webhooks(
        worker_db.conn,
        release_id=RELEASE_ID,
        stage_id="stage-1",
        source_id="source-2",
        status="failed",
        chunk_count=None,
        error="extract failed",
    )

    rows = _delivery_rows(worker_db.conn, webhook_id)
    assert len(rows) == 1
    assert rows[0][1] == "failed"


def test_dispatch_skips_inactive_webhooks(worker_db) -> None:
    webhook_id = _insert_webhook(worker_db.conn, active=0)
    with patch("ragdoll_worker.webhooks.urllib.request.urlopen") as mock_urlopen:
        dispatch_ingest_webhooks(
            worker_db.conn,
            release_id=RELEASE_ID,
            stage_id=None,
            source_id="source-3",
            status="completed",
            chunk_count=1,
            error=None,
        )
        mock_urlopen.assert_not_called()
    assert _delivery_rows(worker_db.conn, webhook_id) == []


def test_dispatch_skips_unsubscribed_events(worker_db) -> None:
    webhook_id = _insert_webhook(worker_db.conn, events=["failed"])
    with patch("ragdoll_worker.webhooks.urllib.request.urlopen") as mock_urlopen:
        dispatch_ingest_webhooks(
            worker_db.conn,
            release_id=RELEASE_ID,
            stage_id=None,
            source_id="source-4",
            status="completed",
            chunk_count=1,
            error=None,
        )
        mock_urlopen.assert_not_called()
    assert _delivery_rows(worker_db.conn, webhook_id) == []


def test_dispatch_treats_invalid_events_json_as_empty(worker_db) -> None:
    webhook_id = str(uuid.uuid4())
    worker_db.conn.execute(
        """
        INSERT INTO webhooks (id, release_id, type, url, secret, events, active)
        VALUES (?, ?, 'ingest_status', ?, ?, ?, 1)
        """,
        (webhook_id, RELEASE_ID, "https://example.com/bad-json", "secret", "not-json"),
    )
    worker_db.conn.commit()

    with patch("ragdoll_worker.webhooks.urllib.request.urlopen") as mock_urlopen:
        dispatch_ingest_webhooks(
            worker_db.conn,
            release_id=RELEASE_ID,
            stage_id=None,
            source_id="source-5",
            status="completed",
            chunk_count=0,
            error=None,
        )
        mock_urlopen.assert_not_called()


@patch("ragdoll_worker.webhooks.urllib.request.urlopen")
def test_dispatch_records_http_error(mock_urlopen: MagicMock, worker_db) -> None:
    webhook_id = _insert_webhook(worker_db.conn)
    http_error = urllib.error.HTTPError(
        url="https://example.com/hook",
        code=503,
        msg="Service Unavailable",
        hdrs=None,
        fp=BytesIO(b"down"),
    )
    mock_urlopen.side_effect = http_error

    dispatch_ingest_webhooks(
        worker_db.conn,
        release_id=RELEASE_ID,
        stage_id=None,
        source_id="source-6",
        status="completed",
        chunk_count=2,
        error=None,
    )

    rows = _delivery_rows(worker_db.conn, webhook_id)
    assert len(rows) == 1
    assert rows[0][2] == 503
    assert rows[0][3] is not None


@patch("ragdoll_worker.webhooks.urllib.request.urlopen")
def test_dispatch_records_network_error_without_raising(mock_urlopen: MagicMock, worker_db) -> None:
    webhook_id = _insert_webhook(worker_db.conn)
    mock_urlopen.side_effect = OSError("connection refused")

    dispatch_ingest_webhooks(
        worker_db.conn,
        release_id=RELEASE_ID,
        stage_id=None,
        source_id="source-7",
        status="failed",
        chunk_count=None,
        error="boom",
    )

    rows = _delivery_rows(worker_db.conn, webhook_id)
    assert len(rows) == 1
    assert rows[0][2] is None
    assert "connection refused" in (rows[0][3] or "")
