#!/usr/bin/env bash

# Exit on any error
set -e

# Load environment variables
if [ -f ".env" ]; then
  export $(grep -v '^#' .env | xargs)
fi

# Run migrations
echo "Running SQLx migrations..."
sqlx migrate run

echo "✅ Migrations completed."



# Run cmd

#chmod +x migrate.sh
#./migrate.sh

