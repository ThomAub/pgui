# PGUI

A multi-database GUI client to query & manage PostgreSQL, SQLite, and ClickHouse databases.

Written in [GPUI](https://gpui.rs) and [GPUI Component](https://github.com/longbridge/gpui-component)

## Features

- 🗄️ **Multi-database support**: PostgreSQL, SQLite, and ClickHouse
- 🎨 **Modern UI**: Built with GPUI framework
- 🌓 **Theme support**: Light and dark themes using Catppuccin colors
- ✨ **SQL editor**: Syntax highlighting and formatting
- 📊 **Results viewer**: Tabular display of query results
- 🔍 **Schema browser**: Explore tables and columns

As of 2025-06-11:

![screengrab](./assets/screenshots/2025-06-11.png)

## Quick Start

1. Start test databases:
   ```bash
   docker-compose up -d
   ```

2. Run PGUI:
   ```bash
   cargo run
   ```

3. Connect to a database:
   - PostgreSQL: `postgres://test:test@localhost:5432/test`
   - SQLite: `sqlite://sqlite_data/test.db`
   - ClickHouse: `clickhouse://test:test@localhost:9000/test`

See [DATABASE_SETUP.md](./DATABASE_SETUP.md) for detailed setup instructions.
