# Problem Statement: Product Catalog Service

## Overview

Build a product catalog and search microservice in Python using FastAPI with the following capabilities:

- Product CRUD operations
- Full-text search with Elasticsearch
- Category and attribute management
- Image handling and CDN integration
- Inventory tracking integration

## Goals

1. **Product Management**
   - Create, read, update, delete products
   - Bulk import/export (CSV, JSON)
   - Product variants (size, color, etc.)
   - Pricing with currency support

2. **Search & Discovery**
   - Full-text search with relevance ranking
   - Faceted filtering (category, price, attributes)
   - Autocomplete suggestions
   - Search analytics

3. **Categorization**
   - Hierarchical category tree
   - Product attributes per category
   - Tag-based organization
   - Featured products and collections

4. **Media Management**
   - Image upload and processing
   - Multiple images per product
   - CDN integration for delivery
   - Image optimization (WebP, thumbnails)

## Constraints

- Use Python 3.11+
- FastAPI with async support
- PostgreSQL for product data
- Elasticsearch for search
- S3-compatible storage for images

## Success Criteria

- Search latency < 100ms p95
- Support 1M+ products
- 99.9% API availability
- Comprehensive API documentation
- Integration tests for all endpoints
