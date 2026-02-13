# Troubleshooting

Common issues and their solutions for the system described in [[architecture]].

## Deployment Issues

### Container Fails to Start

Check the [[components#auth-service]] configuration and ensure environment variables are set.

<error-pattern>
  <code>ERR_AUTH_CONFIG</code>
  <cause>Missing JWT signing key</cause>
  <fix>Set AUTH_JWT_SECRET environment variable</fix>
</error-pattern>

### Health Check Failures

The health endpoint is configured in [[architecture#deployment]].

<error-pattern>
  <code>ERR_HEALTH_TIMEOUT</code>
  <cause>Service startup too slow</cause>
  <fix>Increase readiness probe timeout to 30s</fix>
</error-pattern>

## Database Issues

### Connection Pool Exhaustion

See [[components#connection-pooling]] for pool configuration.

<error-pattern>
  <code>ERR_POOL_EXHAUSTED</code>
  <cause>Too many concurrent queries</cause>
  <fix>Increase pool size or add connection queuing</fix>
</error-pattern>

### Migration Failures

Check [[components#schema-management]] for migration details.

<error-pattern>
  <code>ERR_MIGRATION_CONFLICT</code>
  <cause>Conflicting schema versions</cause>
  <fix>Run migration reset and reapply</fix>
</error-pattern>

## Auth Issues

### Token Expiry

Tokens expire after 1 hour. See [[components#token-generation]] for configuration.

### Invalid Signatures

Verify the signing key matches between [[components#auth-service]] and the API gateway.

<troubleshooting-meta>
  <last-updated>2026-02-13</last-updated>
  <maintainer>ops-team</maintainer>
</troubleshooting-meta>
