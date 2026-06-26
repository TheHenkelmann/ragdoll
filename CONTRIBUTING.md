# Contributing

Thank you for contributing to Ragdoll.

## License

By contributing, you agree that your contributions will be licensed under **AGPL-3.0-only**, the same license as the project. No separate contributor license agreement is required.

## Development

```bash
cd ragdoll
cargo test
cargo llvm-cov --lcov --output-path lcov.info   # optional coverage

cd python
uv venv && uv pip install -e ".[dev]"
pytest
mypy

cd ../frontend
npm install
npm run typecheck
npm run test:coverage
npm run build
```

Install hooks:

```bash
pre-commit install
```

## Pull requests

- Keep changes focused
- Include tests for behavior changes
- Update docs when changing configuration or architecture
- CI uploads coverage for Rust, Python, and frontend when `CODECOV_TOKEN` is configured in the repository secrets
