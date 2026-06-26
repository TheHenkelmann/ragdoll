# SPDX-License-Identifier: AGPL-3.0-only

from ragdoll_worker.config import WorkerConfig
from ragdoll_worker.worker import run_worker


def main() -> None:
    config = WorkerConfig.from_env()
    run_worker(config)


if __name__ == "__main__":
    main()
