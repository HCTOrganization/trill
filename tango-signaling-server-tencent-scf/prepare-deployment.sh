#!/bin/bash

# Prepare deployment package for Tencent Cloud SCF Console upload
# This script creates a clean ZIP file ready for upload

OUTPUT_PATH="${1:-./"tango-signaling-server-scf.zip"}"

echo -e "\033[36mPreparing deployment package for Tencent Cloud SCF...\033[0m"
echo ""

# Step 1: Build the project
echo -e "\033[33mStep 1: Building project...\033[0m"
npm run build
if [ $? -ne 0 ]; then
    echo -e "\033[31mBuild failed!\033[0m"
    exit 1
fi
echo -e "\033[32m✓ Build complete\033[0m"
echo ""

# Step 2: Clean up node_modules to remove dev dependencies
echo -e "\033[33mStep 2: Cleaning dependencies...\033[0m"
echo -e "\033[90m  Removing old node_modules...\033[0m"
rm -rf node_modules

echo -e "\033[90m  Installing production-only dependencies...\033[0m"
npm install --omit=dev --omit=optional
if [ $? -ne 0 ]; then
    echo -e "\033[31mDependency installation failed!\033[0m"
    exit 1
fi
echo -e "\033[32m✓ Dependencies ready\033[0m"
echo ""

# Step 3: Create ZIP file
echo -e "\033[33mStep 3: Creating deployment package...\033[0m"

# Remove old zip if exists
rm -f "$OUTPUT_PATH"

# Create ZIP
echo -e "\033[90m  Adding files to ZIP...\033[0m"
zip -r -q "$OUTPUT_PATH" dist node_modules package.json package-lock.json

ZIP_SIZE=$(du -h "$OUTPUT_PATH" | cut -f1)
echo -e "\033[32m✓ Package created: $OUTPUT_PATH\033[0m"
echo -e "\033[90m  Size: $ZIP_SIZE\033[0m"
echo ""

# Step 4: Verification
echo -e "\033[33mStep 4: Verifying package...\033[0m"
TEMP_DIR="./temp-verify-$$"
mkdir -p "$TEMP_DIR"
unzip -q "$OUTPUT_PATH" -d "$TEMP_DIR"

REQUIRED_FILES=("dist/index.js" "dist/scf-handler.js" "package.json" "node_modules")
ALL_PRESENT=true

for file in "${REQUIRED_FILES[@]}"; do
    if [ -e "$TEMP_DIR/$file" ]; then
        echo -e "  \033[32m✓\033[0m $file"
    else
        echo -e "  \033[31m✗\033[0m $file - MISSING!"
        ALL_PRESENT=false
    fi
done

rm -rf "$TEMP_DIR"
echo ""

if [ "$ALL_PRESENT" = true ]; then
    echo -e "\033[32m✓ All required files present!\033[0m"
    echo ""
    echo -e "\033[36m================================================================================\033[0m"
    echo -e "\033[32mDEPLOYMENT PACKAGE READY!\033[0m"
    echo -e "\033[36m================================================================================\033[0m"
    echo ""
    echo -e "\033[33mNext steps:\033[0m"
    echo -e "\033[37m1. Go to Tencent Cloud Console → SCF → Create Function\033[0m"
    echo -e "\033[37m2. Configure:\033[0m"
    echo -e "\033[90m   - Runtime: Node.js 18\033[0m"
    echo -e "\033[90m   - Handler: dist/scf-handler.handler\033[0m"
    echo -e "\033[90m   - Memory: 512 MB (or adjust as needed)\033[0m"
    echo -e "\033[90m   - Timeout: 30 seconds\033[0m"
    echo -e "\033[37m3. Click 'Upload ZIP'\033[0m"
    echo -e "\033[37m4. Select: $OUTPUT_PATH\033[0m"
    echo ""
    echo -e "\033[90mFile size: $ZIP_SIZE (max limit: 50 MB)\033[0m"
    echo ""
else
    echo -e "\033[31m✗ Some required files are missing!\033[0m"
    exit 1
fi
