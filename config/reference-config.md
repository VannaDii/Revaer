# Reference Configuration

This document illustrates the configuration documents stored in PostgreSQL for Revaer environments. These values are not read at runtime; they exist strictly as documentation and fixtures for tests.

## App Profile

```json
{
  "id": "00000000-0000-0000-0000-000000000001",
  "instance_name": "revaer-dev",
  "mode": "setup",
  "auth_mode": "api_key",
  "version": 1,
  "http_port": 7070,
  "bind_addr": "127.0.0.1",
  "telemetry": {
    "level": "info",
    "format": "pretty",
    "otel_enabled": false,
    "otel_service_name": null,
    "otel_endpoint": null
  },
  "label_policies": [
    {
      "kind": "category",
      "name": "tv",
      "download_dir": ".server_root/downloads",
      "rate_limit_download_bps": null,
      "rate_limit_upload_bps": null,
      "queue_position": null,
      "auto_managed": null,
      "seed_ratio_limit": null,
      "seed_time_limit": null
    }
  ],
  "immutable_keys": [
    "bind_addr",
    "http_port"
  ]
}
```

## Engine Profile

```json
{
  "id": "00000000-0000-0000-0000-000000000002",
  "implementation": "libtorrent",
  "listen_port": 51003,
  "listen_interfaces": [],
  "ipv6_mode": "disabled",
  "anonymous_mode": false,
  "force_proxy": false,
  "prefer_rc4": false,
  "allow_multiple_connections_per_ip": false,
  "enable_outgoing_utp": true,
  "enable_incoming_utp": true,
  "outgoing_port_min": null,
  "outgoing_port_max": null,
  "peer_dscp": null,
  "dht": false,
  "encryption": "require",
  "max_active": 4,
  "max_download_bps": null,
  "max_upload_bps": null,
  "seed_ratio_limit": null,
  "seed_time_limit": null,
  "connections_limit": null,
  "connections_limit_per_torrent": null,
  "unchoke_slots": null,
  "half_open_limit": null,
  "alt_speed": {
    "download_bps": null,
    "upload_bps": null,
    "schedule": null
  },
  "stats_interval_ms": null,
  "sequential_default": true,
  "auto_managed": true,
  "auto_manage_prefer_seeds": false,
  "dont_count_slow_torrents": true,
  "super_seeding": false,
  "choking_algorithm": "fixed_slots",
  "seed_choking_algorithm": "round_robin",
  "strict_super_seeding": false,
  "optimistic_unchoke_slots": null,
  "max_queued_disk_bytes": null,
  "resume_dir": ".server_root/resume",
  "download_root": ".server_root/downloads",
  "storage_mode": "sparse",
  "use_partfile": true,
  "disk_read_mode": null,
  "disk_write_mode": null,
  "verify_piece_hashes": true,
  "cache_size": null,
  "cache_expiry": null,
  "coalesce_reads": true,
  "coalesce_writes": true,
  "use_disk_cache_pool": true,
  "tracker": {
    "default": [],
    "extra": [],
    "replace": false,
    "user_agent": "revaer/0.1",
    "announce_ip": null,
    "listen_interface": null,
    "request_timeout_ms": null,
    "announce_to_all": false,
    "ssl_cert": null,
    "ssl_private_key": null,
    "ssl_ca_cert": null,
    "ssl_tracker_verify": true,
    "proxy": null,
    "auth": null
  },
  "enable_lsd": false,
  "enable_upnp": false,
  "enable_natpmp": false,
  "enable_pex": false,
  "dht_bootstrap_nodes": [],
  "dht_router_nodes": [],
  "ip_filter": {
    "cidrs": [],
    "blocklist_url": null,
    "etag": null,
    "last_updated_at": null,
    "last_error": null
  },
  "peer_classes": {
    "classes": [],
    "default": []
  }
}
```

## Filesystem Policy

```json
{
  "id": "00000000-0000-0000-0000-000000000003",
  "library_root": ".server_root/library",
  "extract": false,
  "par2": "disabled",
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
