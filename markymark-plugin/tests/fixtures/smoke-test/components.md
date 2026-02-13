# Components

## Overview

This document describes the core system components referenced in [[architecture]].

## Auth Service

The authentication service handles user identity and access control.

### Token Generation

Tokens are generated using RS256 signing. See [[architecture#security]] for the full security model.

### Token Validation

Middleware validates tokens on every request (see [[architecture#data-flow]]).

<component-spec>
  <name>auth-service</name>
  <type>microservice</type>
  <port>8080</port>
  <dependencies>
    <dep>database-layer</dep>
    <dep>cache-service</dep>
  </dependencies>
</component-spec>

## Database Layer

The database layer provides persistence using PostgreSQL.

### Schema Management

Migrations are applied automatically on startup.

### Connection Pooling

Connection pool size defaults to 20. See [[troubleshooting#database-issues]] for tuning.

<component-spec>
  <name>database-layer</name>
  <type>infrastructure</type>
  <port>5432</port>
  <dependencies>
    <dep>none</dep>
  </dependencies>
</component-spec>

## Cache Service

Redis-based caching for session data and hot queries.

See [[architecture#data-flow]] for how cache fits in the request path.

<component-spec>
  <name>cache-service</name>
  <type>infrastructure</type>
  <port>6379</port>
</component-spec>
