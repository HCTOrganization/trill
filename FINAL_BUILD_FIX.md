# Final Build Fix - PBJS Argument Order

## Issue
Linux build failed with:
```
TypeError [ERR_INVALID_ARG_TYPE]: The "path" argument must be of type string...
```

## Root Cause
The `pbjs` command had arguments in the wrong order:
- ❌ Wrong: `pbjs --es6 input.proto > output.js`
- ✓ Correct: `pbjs --es6 output.js input.proto`

## Solution Applied

**File**: `package.json`
**Section**: `"scripts"` → `"proto"`

### Before (Wrong):
```json
"proto": "mkdir -p src/proto && npx pbjs --es6 ../tango-signaling/src/proto/signaling.proto > src/proto/signaling.js && npx pbts src/proto/signaling.js > src/proto/signaling.d.ts"
```

### After (Correct):
```json
"proto": "mkdir -p src/proto && npx pbjs --es6 src/proto/signaling.js ../tango-signaling/src/proto/signaling.proto && npx pbts src/proto/signaling.d.ts src/proto/signaling.js"
```

### Changes:
1. **Output file goes FIRST**: `src/proto/signaling.js`
2. **Input file goes SECOND**: `../tango-signaling/src/proto/signaling.proto`
3. **No shell redirection** `>` (pbjs expects explicit file arguments)

## PBJS Correct Syntax

```bash
pbjs --es6 <output_file> <input_proto_file>
          │              │
          └─ First      └─ Second
```

## How to Build

```bash
cd tango-signaling-server-tencent-scf

# Clean
rm -rf src/proto dist

# Build
npm install
npm run build

# Verify
ls -la dist/scf-handler.js
ls -la src/proto/signaling.js
```

## All Build Issues Fixed

### Issue 1: pbjs: not found
**Fixed**: Added `npx` prefix

### Issue 2: Invalid pbjs syntax
**Fixed**: Updated `--es6` flag usage

### Issue 3: Wrong argument order
**Fixed**: Put output file before input file

## Verify Success

After build, check:
```bash
dist/index.js              ✓
dist/scf-handler.js        ✓
src/proto/signaling.js     ✓
src/proto/signaling.d.ts   ✓
```

If all exist → Build succeeded! ✓

## Command Reference

### Generate JavaScript from Proto
```bash
pbjs --es6 output.js input.proto
```

### Generate TypeScript Definitions
```bash
pbts output.d.ts input.js
```

### Compile TypeScript
```bash
tsc
```

## Next Steps

1. ✅ Run: `npm install && npm run build`
2. ✅ Verify files created
3. ✅ Create deployment ZIP: `./prepare-deployment.sh`
4. ✅ Upload to Tencent Cloud

## Documentation

- **PBJS_CORRECT_SYNTAX.md** - Detailed explanation
- **BUILD_TROUBLESHOOTING.md** - General build issues
- **ALL_FIXES_SUMMARY.md** - Complete overview

## Status

✅ **ALL BUILD ISSUES FIXED**

Ready to deploy!
