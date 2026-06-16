# Prepare deployment package for Tencent Cloud SCF Console upload
# This script creates a clean ZIP file ready for upload

param(
    [string]$OutputPath = "./tango-signaling-server-scf.zip"
)

Write-Host "Preparing deployment package for Tencent Cloud SCF..." -ForegroundColor Cyan
Write-Host ""

# Step 1: Build the project
Write-Host "Step 1: Building project..." -ForegroundColor Yellow
npm run build
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "✓ Build complete" -ForegroundColor Green
Write-Host ""

# Step 2: Clean up node_modules to remove dev dependencies
Write-Host "Step 2: Cleaning dependencies..." -ForegroundColor Yellow
Write-Host "  Removing old node_modules..." -ForegroundColor Gray
Remove-Item -Path "./node_modules" -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "  Installing production-only dependencies..." -ForegroundColor Gray
npm install --omit=dev --omit=optional
if ($LASTEXITCODE -ne 0) {
    Write-Host "Dependency installation failed!" -ForegroundColor Red
    exit 1
}
Write-Host "✓ Dependencies ready" -ForegroundColor Green
Write-Host ""

# Step 3: Create ZIP file
Write-Host "Step 3: Creating deployment package..." -ForegroundColor Yellow

# Remove old zip if exists
if (Test-Path $OutputPath) {
    Remove-Item $OutputPath -Force
}

# Files/folders to include
$filesToZip = @(
    "dist",
    "node_modules",
    "package.json",
    "package-lock.json"
)

# Create ZIP using PowerShell's Compress-Archive
Write-Host "  Adding files to ZIP..." -ForegroundColor Gray
Compress-Archive -Path $filesToZip -DestinationPath $OutputPath -CompressionLevel Optimal

$zipSize = (Get-Item $OutputPath).Length / 1MB
Write-Host "✓ Package created: $OutputPath" -ForegroundColor Green
Write-Host "  Size: $([Math]::Round($zipSize, 2)) MB" -ForegroundColor Gray
Write-Host ""

# Step 4: Verification
Write-Host "Step 4: Verifying package..." -ForegroundColor Yellow
$tempDir = "./temp-verify"
New-Item -ItemType Directory -Path $tempDir -Force | Out-Null
Expand-Archive -Path $OutputPath -DestinationPath $tempDir -Force

$requiredFiles = @("dist/index.js", "dist/scf-handler.js", "package.json", "node_modules")
$allPresent = $true

foreach ($file in $requiredFiles) {
    $fullPath = Join-Path $tempDir $file
    if (Test-Path $fullPath) {
        Write-Host "  ✓ $file" -ForegroundColor Green
    } else {
        Write-Host "  ✗ $file - MISSING!" -ForegroundColor Red
        $allPresent = $false
    }
}

Remove-Item -Path $tempDir -Recurse -Force
Write-Host ""

if ($allPresent) {
    Write-Host "✓ All required files present!" -ForegroundColor Green
    Write-Host ""
    Write-Host "=================================================================================" -ForegroundColor Cyan
    Write-Host "DEPLOYMENT PACKAGE READY!" -ForegroundColor Green
    Write-Host "=================================================================================" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor Yellow
    Write-Host "1. Go to Tencent Cloud Console → SCF → Create Function" -ForegroundColor White
    Write-Host "2. Configure:" -ForegroundColor White
    Write-Host "   - Runtime: Node.js 18" -ForegroundColor Gray
    Write-Host "   - Handler: dist/scf-handler.handler" -ForegroundColor Gray
    Write-Host "   - Memory: 512 MB (or adjust as needed)" -ForegroundColor Gray
    Write-Host "   - Timeout: 30 seconds" -ForegroundColor Gray
    Write-Host "3. Click 'Upload ZIP'" -ForegroundColor White
    Write-Host "4. Select: $OutputPath" -ForegroundColor Gray
    Write-Host ""
    Write-Host "File size: $([Math]::Round($zipSize, 2)) MB (max limit: 50 MB)" -ForegroundColor Gray
    Write-Host ""
} else {
    Write-Host "✗ Some required files are missing!" -ForegroundColor Red
    exit 1
}
