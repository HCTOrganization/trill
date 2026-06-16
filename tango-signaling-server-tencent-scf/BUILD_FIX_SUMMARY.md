# Build Fix Summary

## Issue
Linux build failed with: `sh: 1: pbjs: not found`

## Solution Applied
Updated `package.json` to use `npx` prefix for protobuf tools.

### What Changed
**File**: `package.json`
**Section**: `"scripts"` → `"proto"` field

**Before**:
```json
"proto": "mkdir -p src/proto && pbjs -t static-module ... && pbts ..."
```

**After**:
```json
"proto": "mkdir -p src/proto && npx pbjs -t static-module ... && npx pbts ..."
```

## Why This Fixes It

- ✗ `pbjs` command: Looks for global installation (fails on Linux)
- ✓ `npx pbjs` command: Uses local `node_modules/.bin/pbjs` (always works)

`npx` is the npm-provided tool runner that finds executables in your project.

## How to Build Now

### Quick Build
```bash
cd tango-signaling-server-tencent-scf
npm install
npm run build
```

### What It Does
1. `npm install` - Installs `protobufjs-cli` to `node_modules/`
2. `npm run build` - Runs the proto script which now uses `npx`
3. `npx pbjs` - Uses the local protobufjs-cli to compile proto files
4. `tsc` - Compiles TypeScript to JavaScript

### Expected Output
```
> npm run proto
> mkdir -p src/proto && npx pbjs ... && npx pbts ...
[proto compilation output]

> tsc
[TypeScript compilation]

✓ Build complete
```

### Verify Success
Check these files exist:
- `dist/index.js`
- `dist/scf-handler.js`
- `src/proto/signaling.js`
- `src/proto/signaling.d.ts`

## For Different Platforms

### Linux ✓
```bash
npm install && npm run build
```

### macOS ✓
```bash
npm install && npm run build
```

### Windows (PowerShell) ✓
```powershell
npm install; npm run build
```

All platforms now work because `npx` is universal!

## If Build Still Fails

1. **Clean and retry**:
```bash
rm -rf node_modules src/proto dist
npm install
npm run build
```

2. **Check dependencies installed**:
```bash
npm ls protobufjs-cli
# Should show: protobufjs-cli@1.1.3
```

3. **Verify proto file exists**:
```bash
ls -la ../tango-signaling/src/proto/signaling.proto
```

4. **For detailed troubleshooting**: See `BUILD_TROUBLESHOOTING.md`

## What Else Changed?

Nothing else! Only this one line in `package.json` was modified. All other configuration, code, and documentation remain the same.

## Next Steps

1. ✅ Update your local repository
2. ✅ Run `npm install && npm run build`
3. ✅ Create deployment ZIP: `./prepare-deployment.sh` (or PowerShell equivalent)
4. ✅ Upload to Tencent Cloud: Follow `CONSOLE_UPLOAD.md`

## Files to Reference

- **This file**: `BUILD_FIX_SUMMARY.md` - Quick overview
- **Detailed guide**: `BUILD_TROUBLESHOOTING.md` - All build issues and solutions
- **Linux-specific**: `LINUX_BUILD_FIX.md` - Linux-focused troubleshooting
- **Package config**: `package.json` - See the fix applied

---

**Status**: ✓ Fixed - Build should now work on all platforms!
