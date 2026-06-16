# Manual ZIP Package Preparation Guide

This guide explains exactly what files to include in your ZIP package for console upload.

## Quick Answer

**Yes**, you can ZIP the entire `tango-signaling-server-tencent-scf` directory and upload it, but it's **not optimal** because:

1. ✗ Includes unnecessary files (documentation, git files, tests)
2. ✗ ZIP will be too large (100+ MB due to dev dependencies)
3. ✗ Upload to console will be slow

**Better approach**: Create a clean ZIP with only required files (~15 MB).

## Method 1: Using the Provided Script (EASIEST)

### Windows PowerShell
```powershell
cd tango-signaling-server-tencent-scf
.\prepare-deployment.ps1
```

### macOS/Linux
```bash
cd tango-signaling-server-tencent-scf
chmod +x prepare-deployment.sh
./prepare-deployment.sh
```

This creates an optimized ZIP file ready to upload.

---

## Method 2: Manual Preparation (Step-by-Step)

### Step 1: Navigate to Project
```bash
cd tango-signaling-server-tencent-scf
```

### Step 2: Build the Project
```bash
npm run build
```

This creates the `dist/` folder with compiled JavaScript.

**What you'll see:**
```
dist/
├── index.js
├── index.d.ts
├── scf-handler.js
└── scf-handler.d.ts
```

### Step 3: Clean Dependencies
Remove dev dependencies to reduce ZIP size:

**Windows PowerShell:**
```powershell
Remove-Item -Path "node_modules" -Recurse -Force
npm install --omit=dev
```

**macOS/Linux:**
```bash
rm -rf node_modules
npm install --omit=dev
```

This reduces size from ~500 MB to ~50 MB.

### Step 4: Create the ZIP File

**Windows PowerShell:**
```powershell
Compress-Archive -Path "dist", "node_modules", "package.json", "package-lock.json" `
  -DestinationPath "tango-signaling-server-scf.zip" -CompressionLevel Optimal
```

**Windows Command Prompt:**
```cmd
:: Use 7-Zip or WinRAR
:: Or use PowerShell:
powershell -Command "Compress-Archive -Path 'dist', 'node_modules', 'package.json', 'package-lock.json' -DestinationPath 'tango-signaling-server-scf.zip'"
```

**macOS/Linux:**
```bash
zip -r tango-signaling-server-scf.zip dist node_modules package.json package-lock.json
```

### Step 5: Verify ZIP Contents

**Windows PowerShell:**
```powershell
# List files in ZIP
Expand-Archive -Path "tango-signaling-server-scf.zip" -DestinationPath "temp-check" -Force
Get-ChildItem -Path "temp-check" -Recurse
Remove-Item -Path "temp-check" -Recurse -Force
```

**macOS/Linux:**
```bash
unzip -l tango-signaling-server-scf.zip | head -20
```

You should see:
```
Archive:  tango-signaling-server-scf.zip
  Length      Date    Time    Name
---------  ---------- -----   ----
        0  01-01-2024 00:00   dist/
        0  01-01-2024 00:00   dist/
   xxxxx   01-01-2024 00:00   dist/index.js
   xxxxx   01-01-2024 00:00   dist/scf-handler.js
        0  01-01-2024 00:00   node_modules/
   xxxxx   01-01-2024 00:00   package.json
   xxxxx   01-01-2024 00:00   package-lock.json
   ...
```

---

## What to Include / Exclude

### ✅ MUST INCLUDE
- `dist/` - Compiled JavaScript
- `node_modules/` - Production dependencies only
- `package.json` - Project metadata
- `package-lock.json` - Dependency lock file

### ✗ DO NOT INCLUDE
- `src/` - TypeScript source (not needed, we have dist/)
- `.git/` - Git history
- `node_modules/typescript` - Already compiled to dist/
- `node_modules/protobufjs-cli` - Build tool (not runtime)
- `README.md`, `DEPLOYMENT.md`, etc. - Documentation
- `.env.example` - Template only
- `wrangler.toml` - Cloudflare config (not used)
- `serverless.yml` - Build tool config (not needed at runtime)
- `.gitignore` - Not needed at runtime

---

## ZIP Structure (Expected)

Your ZIP file should have this structure when extracted:

```
tango-signaling-server-scf.zip (extracted)
├── dist/
│   ├── index.js
│   ├── index.d.ts
│   ├── scf-handler.js
│   └── scf-handler.d.ts
├── node_modules/
│   ├── express/
│   ├── ws/
│   ├── protobufjs/
│   ├── redis/
│   └── ... (other dependencies)
├── package.json
└── package-lock.json
```

### File Size Reference

| Component | Size |
|-----------|------|
| `dist/` | ~100 KB |
| `node_modules/` | ~50-70 MB |
| `package.json` + `package-lock.json` | ~5 KB |
| **Total ZIP (optimized)** | **~12-15 MB** |

---

## Method 3: Using System Tools (Alternative)

### Windows File Explorer

**If you prefer GUI:**

1. Create a new folder: `scf-deployment`
2. Copy into it:
   - `dist/` folder
   - `node_modules/` folder (after running `npm install --omit=dev`)
   - `package.json` file
   - `package-lock.json` file
3. Right-click the folder → Send to → Compressed (zipped) folder
4. This creates `scf-deployment.zip`

### macOS Finder

1. Create a new folder: `scf-deployment`
2. Copy the required files as above
3. Right-click the folder → Compress "scf-deployment"
4. Creates `scf-deployment.zip`

### Linux File Manager

1. Create folder and copy files as above
2. Right-click → Compress or Archive
3. Select ZIP format

---

## Upload to Tencent Cloud Console

Once you have your ZIP file:

1. **Go to**: [Tencent Cloud SCF Console](https://console.cloud.tencent.com/scf)
2. **Click**: "Create Function"
3. **Fill in**:
   - Function name: `tango-signaling-server`
   - Runtime: **Node.js 18** ⚠️ IMPORTANT
   - Handler: **`dist/scf-handler.handler`** ⚠️ IMPORTANT
   - Memory: 512 MB
   - Timeout: 30 seconds
4. **Upload**: Click "Upload ZIP" button
5. **Select**: Your `tango-signaling-server-scf.zip` file
6. **Wait**: For upload to complete
7. **Click**: "Create" or "Complete"

---

## Troubleshooting

### ZIP is too large (> 50 MB)

**Problem**: You included `node_modules` with dev dependencies

**Solution**:
```bash
rm -rf node_modules
npm install --omit=dev
# Recreate ZIP
```

### Handler error: "Cannot find module"

**Problem**: Missing files in ZIP

**Solution**:
1. Extract ZIP to verify contents
2. Check `dist/scf-handler.js` exists
3. Check `node_modules/express/` exists
4. Recreate ZIP if missing

### Runtime error: "Node.js 18 not selected"

**Problem**: Wrong runtime selected in console

**Solution**:
1. Edit function
2. Change Runtime to **Node.js 18** (not 16, not 20)
3. Save

### Upload hangs or fails

**Problem**: Connection timeout or file too large

**Solution**:
1. Check ZIP file is not corrupt: `unzip -t tango-signaling-server-scf.zip`
2. Try uploading in smaller chunks via console
3. Use Serverless Framework instead: `npm run deploy`

---

## Comparison: All Methods

| Method | Pros | Cons | Time |
|--------|------|------|------|
| **Script** (Recommended) | Automatic, verified | Requires PowerShell/Bash | 2 min |
| **Manual CLI** | Full control | Multiple steps | 5 min |
| **GUI File Manager** | Easy for beginners | Manual, error-prone | 10 min |
| **Full Directory ZIP** | Quickest zip | Very large file, slow upload | 1 min + slow upload |

---

## Quick Commands Reference

### One-liner to create clean ZIP (Linux/macOS):
```bash
cd tango-signaling-server-tencent-scf && \
npm run build && \
rm -rf node_modules && \
npm install --omit=dev && \
zip -r ../tango-signaling-server-scf.zip dist node_modules package.json package-lock.json
```

### PowerShell one-liner:
```powershell
cd tango-signaling-server-tencent-scf; `
npm run build; `
Remove-Item -Path node_modules -Recurse -Force; `
npm install --omit=dev; `
Compress-Archive -Path dist, node_modules, package.json, package-lock.json `
  -DestinationPath ../tango-signaling-server-scf.zip
```

---

## Next Steps

1. ✅ Create optimized ZIP
2. ✅ Upload to Tencent Cloud Console
3. ✅ Configure as shown in [CONSOLE_UPLOAD.md](./CONSOLE_UPLOAD.md)
4. ✅ Test WebSocket connection

That's it! Your function will be ready in minutes.
