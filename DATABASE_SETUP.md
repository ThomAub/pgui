# Database Setup Guide

This project now supports three database types: PostgreSQL, SQLite, and ClickHouse. Use the provided `docker-compose.yml` to quickly set up test databases.

## Starting the Databases

### Start all databases:
```bash
docker-compose up -d
```

### Start specific database:
```bash
# PostgreSQL only
docker-compose up -d postgres

# ClickHouse only
docker-compose up -d clickhouse

# SQLite initialization
docker-compose up sqlite-init
```

## Connection URLs

After starting the containers, use these connection URLs in PGUI:

### PostgreSQL
- URL: `postgres://test:test@localhost:5432/test`
- Port: 5432

### SQLite
- URL: `sqlite://sqlite_data/test.db`
- Or just: `sqlite_data/test.db`
- The database file will be created at `./sqlite_data/test.db`

### ClickHouse
- URL: `http://test:test@localhost:8123/test`
- HTTP Port: 8123 (default)
- Native Port: 9000

## Sample Data

Each database is initialized with the same schema containing:
- Users table with 6 sample users
- Companies table with 5 companies
- Products table with sample products
- Orders and order items
- Categories with hierarchy
- Product reviews
- System logs
- User roles and permissions

### Database-Specific Features

**PostgreSQL**:
- Full foreign key constraints
- Array data types for permissions
- INET type for IP addresses

**SQLite**:
- Full-text search on products
- Triggers for updated_at timestamps
- Daily statistics table

**ClickHouse**:
- Partitioned tables for time-series data
- Materialized views for analytics
- TTL (Time To Live) for metrics data
- Real-time event tracking tables

## Stopping Databases

```bash
# Stop all containers
docker-compose down

# Stop and remove volumes (deletes all data)
docker-compose down -v
```

## Troubleshooting

### ClickHouse Connection Issues
If you can't connect to ClickHouse, ensure:
1. The container is running: `docker ps`
2. Port 8123 is not in use: `lsof -i :8123`
3. Try the HTTP interface: `curl http://localhost:8123/`

### SQLite Permissions
If you have permission issues with SQLite:
```bash
chmod 755 sqlite_data
chmod 644 sqlite_data/test.db
```

### PostgreSQL Connection Refused
Ensure PostgreSQL is fully started:
```bash
docker logs postgres_db
```