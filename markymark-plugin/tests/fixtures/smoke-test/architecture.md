# Architecture

## Overview

The system follows a layered architecture with clear separation of concerns.

See [[components]] for implementation details of each layer.

<architecture-diagram>
  <layer name="presentation">Web UI, CLI</layer>
  <layer name="application">API Gateway, Auth</layer>
  <layer name="domain">Business Logic</layer>
  <layer name="infrastructure">Database, Cache, Queue</layer>
</architecture-diagram>

## Data Flow

1. Request enters through the API Gateway
2. Auth middleware validates the token (see [[components#auth-service]])
3. Request is routed to the appropriate handler
4. Handler interacts with the [[components#database-layer]]
5. Response is serialized and returned

## Deployment

The system is deployed as containers. See [[troubleshooting#deployment-issues]] for common problems.

<deployment-config>
  <environment>production</environment>
  <replicas>3</replicas>
  <health-check>/api/health</health-check>
</deployment-config>

## Security

Authentication uses JWT tokens. See [[components#auth-service]] for details.

### Token Lifecycle

Tokens are issued by the auth service and validated by middleware.
See [[index#quick-links]] for navigation help.

<security-notes>
  <classification>internal</classification>
  <review-date>2026-03-01</review-date>
</security-notes>
