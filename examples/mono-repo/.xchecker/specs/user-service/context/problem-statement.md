# Problem Statement: User Service

## Overview

Build a high-performance user authentication and profile management microservice in Rust with the following capabilities:

- JWT-based authentication
- OAuth2 provider integration (Google, GitHub)
- User profile CRUD operations
- Password reset flow
- Session management
- Rate limiting

## Goals

1. **Authentication**
   - Email/password login with bcrypt hashing
   - JWT token generation and validation
   - Refresh token rotation
   - OAuth2 authorization code flow

2. **User Management**
   - User registration with email verification
   - Profile updates (name, avatar, preferences)
   - Account deletion (GDPR compliance)
   - Password change and reset

3. **Security**
   - Rate limiting per IP and user
   - Brute force protection
   - Secure session handling
   - Audit logging

4. **Integration**
   - gRPC API for internal services
   - REST API for external clients
   - Event publishing for user lifecycle events

## Constraints

- Use Rust stable (1.70+)
- PostgreSQL for persistent storage
- Redis for session cache and rate limiting
- Follow OWASP security guidelines
- Support horizontal scaling

## Success Criteria

- Authentication latency < 50ms p99
- Support 10,000 concurrent sessions
- Zero security vulnerabilities in audit
- 95%+ test coverage
- API documentation complete
