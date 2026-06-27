# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from unittest.mock import MagicMock, patch

import ragdoll_worker.worker as worker_module
from ragdoll_worker.worker import _handle_signal, run_worker


def setup_function() -> None:
    worker_module._shutdown = False


def test_handle_signal_sets_shutdown_flag() -> None:
    assert worker_module._shutdown is False
    _handle_signal(15, None)
    assert worker_module._shutdown is True


@patch("ragdoll_worker.worker.time.sleep")
@patch("ragdoll_worker.worker.process_job")
@patch("ragdoll_worker.worker.reconcile_jobs")
@patch("ragdoll_worker.worker.Embedder")
@patch("ragdoll_worker.worker.WorkerDb")
def test_run_worker_processes_claimed_job(
    mock_db_cls: MagicMock,
    mock_embedder_cls: MagicMock,
    mock_reconcile: MagicMock,
    mock_process_job: MagicMock,
    mock_sleep: MagicMock,
    worker_config,
) -> None:
    mock_db = MagicMock()
    mock_db_cls.return_value = mock_db
    job = {
        "id": "job-loop-1",
        "source_id": "src-loop-1",
        "attempts": 1,
        "max_attempts": 3,
    }

    def claim_side_effect(_worker_id: str):
        if worker_module._shutdown:
            return None
        worker_module._shutdown = True
        return job

    mock_db.claim_job.side_effect = claim_side_effect

    run_worker(worker_config)

    mock_reconcile.assert_called_once()
    mock_db.claim_job.assert_called()
    mock_db.heartbeat.assert_called_once_with("job-loop-1", worker_config.worker_id)
    mock_process_job.assert_called_once()
    mock_db.complete_job.assert_called_once_with("job-loop-1")
    mock_db.release.assert_called()


@patch("ragdoll_worker.worker.time.sleep")
@patch("ragdoll_worker.worker.reconcile_jobs")
@patch("ragdoll_worker.worker.Embedder")
@patch("ragdoll_worker.worker.WorkerDb")
def test_run_worker_retries_when_database_locked(
    mock_db_cls: MagicMock,
    mock_embedder_cls: MagicMock,
    mock_reconcile: MagicMock,
    mock_sleep: MagicMock,
    worker_config,
) -> None:
    mock_db = MagicMock()
    mock_db_cls.return_value = mock_db

    call_count = 0

    def claim_side_effect(_worker_id: str):
        nonlocal call_count
        call_count += 1
        if call_count == 1:
            raise ValueError("database is locked")
        worker_module._shutdown = True
        return None

    mock_db.claim_job.side_effect = claim_side_effect

    run_worker(worker_config)

    assert call_count == 2
    mock_sleep.assert_called()
    mock_db.release.assert_called()


@patch("ragdoll_worker.worker.time.sleep")
@patch("ragdoll_worker.worker.process_job")
@patch("ragdoll_worker.worker.reconcile_jobs")
@patch("ragdoll_worker.worker.Embedder")
@patch("ragdoll_worker.worker.WorkerDb")
def test_run_worker_fails_job_on_processing_error(
    mock_db_cls: MagicMock,
    mock_embedder_cls: MagicMock,
    mock_reconcile: MagicMock,
    mock_process_job: MagicMock,
    mock_sleep: MagicMock,
    worker_config,
) -> None:
    mock_db = MagicMock()
    mock_db_cls.return_value = mock_db
    job = {
        "id": "job-fail-loop",
        "source_id": "src-fail-loop",
        "attempts": 1,
        "max_attempts": 3,
    }

    def claim_side_effect(_worker_id: str):
        if worker_module._shutdown:
            return None
        worker_module._shutdown = True
        return job

    mock_db.claim_job.side_effect = claim_side_effect
    mock_process_job.side_effect = RuntimeError("chunk failed")

    run_worker(worker_config)

    mock_db.fail_job.assert_called_once_with(
        "job-fail-loop",
        "chunk failed",
        retry=True,
    )
