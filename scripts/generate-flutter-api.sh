#!/bin/bash

# Generate Dart API client from OpenAPI spec
cd "$(dirname "$0")/.."

echo "Generating Flutter API client from OpenAPI spec..."

# Ensure openapi-generator is installed
if ! command -v openapi-generator &> /dev/null; then
    echo "Warning: openapi-generator not found"
    echo "Install: npm install -g @openapitools/openapi-generator-cli"
    echo "Skipping Flutter API client generation..."
    exit 0
fi

# Generate Dart client
openapi-generator generate \
    -i api/swagger.json \
    -g dart-dio \
    -o ui/lib/generated/api \
    --additional-properties=pubName=nexus_api,pubAuthor="Nexus Team",dateLibrary=core

echo "Flutter API client generated successfully!"
echo "Import in Dart: import 'package:nexus_api/nexus_api.dart';"
