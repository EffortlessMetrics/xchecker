# Problem Statement: Order API Service

## Overview

Build an order processing and fulfillment microservice in Rust with the following capabilities:

- Order creation and management
- Payment processing integration
- Inventory reservation
- Shipping integration
- Order status tracking

## Goals

1. **Order Management**
   - Create orders from cart
   - Order modification (before fulfillment)
   - Order cancellation with refund
   - Order history and search

2. **Payment Processing**
   - Stripe integration
   - Payment intent creation
   - Refund processing
   - Payment failure handling

3. **Inventory Integration**
   - Stock reservation on order
   - Reservation timeout handling
   - Back-order support
   - Low stock notifications

4. **Fulfillment**
   - Shipping rate calculation
   - Label generation
   - Tracking number integration
   - Delivery status updates

5. **Notifications**
   - Order confirmation emails
   - Shipping notifications
   - Delivery confirmation
   - Review request after delivery

## Constraints

- Use Rust stable (1.70+)
- PostgreSQL for order data
- Event-driven architecture (Kafka/NATS)
- Idempotent API operations
- Support distributed transactions (Saga pattern)

## Success Criteria

- Order creation latency < 200ms p99
- Zero lost orders (exactly-once processing)
- Support 1000 orders/minute peak
- 99.99% payment success rate
- Complete audit trail for all orders
