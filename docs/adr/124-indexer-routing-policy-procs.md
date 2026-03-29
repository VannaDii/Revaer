# Indexer routing policy stored procedures

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Routing policy mutations require role checks, parameter validation, and audit logging.
  - ERD_INDEXERS.md specifies parameter constraints per routing mode and secret binding
    requirements for proxy credentials.
  - Procedures must use constant error messages with structured detail codes.
- Decision:
  - Add migration 0035 implementing routing_policy_create_v1, routing_policy_set_param_v1,
    and routing_policy_bind_secret_v1 plus stable wrappers.
  - Enforce owner/admin role checks, display_name validation, and unsupported mode rejection.
  - Validate parameter types and ranges; restrict param keys to mode-specific allowlists.
  - Create verify_tls on policy creation and ensure auth parameter rows exist for proxy modes.
  - Bind secrets via secret_binding with secret_audit_log and config_audit_log entries.
- Consequences:
  - Routing policy state is validated and auditable at the database layer.
  - Proxy credential bindings are centralized with explicit secret audit events.
- Follow-up:
  - Implement routing policy API handlers using these procedures.
  - Add tests for param validation edge cases and secret binding replacement.
