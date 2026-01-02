# Reference Configuration

This document illustrates the configuration documents stored in PostgreSQL for Revaer environments. These values are **not** read at runtime; they exist strictly as documentation and fixtures for tests.

## App Profile
```json
{
  "id": "00000000-0000-0000-0000-000000000001",
  "version": 1,
  "mode": "setup",
  "instance_name": "revaer-dev",
  "http_port": 7070,
  "bind_addr": "127.0.0.1",
  "telemetry": {
    "log_level": "info",
    "prometheus": false
  },
  "features": {
    "fs_extract": false,
    "par2": false,
    "sse_backpressure": false
  },
  "immutable_keys": [
    "bind_addr",
    "http_port"
  ]
}
```

## Engine Profile
```json
{
  "engine_impl": "libtorrent",
  "listen_port": 51003,
  "dht": false,
  "encryption": "require",
  "max_active": 4,
  "max_download_bps": null,
  "max_upload_bps": null,
  "sequential_default": true,
  "resume_dir": ".server_root/resume",
  "download_root": ".server_root/downloads",
  "tracker": {
    "user_agent": "revaer/0.1",
    "announce_interval_override_secs": null
  }
}
```

## Filesystem Policy
```json
{
  "library_root": ".server_root/library",
  "extract": false,
  "par2": "off",
  "flatten": false,
  "move_mode": "hardlink",
  "cleanup_keep": [
    "**/*.srt"
  ],
  "cleanup_drop": [
    "**/*.nfo"
  ],
  "chmod_file": "0644",
  "chmod_dir": "0755",
  "owner": null,
  "group": null,
  "umask": "002",
  "allow_paths": [
    ".server_root/downloads",
    ".server_root/library"
  ]
}
```
