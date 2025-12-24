# Native Libtorrent Integration Tests

These tests are opt-in (gated by `REVAER_NATIVE_IT`) to keep the default matrix deterministic; include them explicitly in feature-matrix runs.

To run the feature-gated native libtorrent integration suite locally:

```bash
# Ensure Docker (or colima) is running and DOCKER_HOST is set if not using /var/run/docker.sock
export DOCKER_HOST=${DOCKER_HOST:-unix:///Users/vanna/.colima/default/docker.sock}

# Enable native integration tests
export REVAER_NATIVE_IT=1

# Run the full gate (preferred)
just ci

# Or target only the libtorrent native suite
just test-native
```

CI note: add a matrix job that sets `REVAER_NATIVE_IT=1` and points `DOCKER_HOST` at the runner’s daemon to ensure the native path stays covered.
