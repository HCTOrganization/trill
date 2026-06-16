# Linux Build Fix - pbjs Not Found Error

## Problem

When building on Linux, you see:
```
sh: 1: pbjs: not found
```

## Root Cause

The build script was trying to use `pbjs` and `pbts` commands as if they were globally installed, but they're only available in `node_modules/.bin/`.

## Solution (ALREADY APPLIED)

The `package.json` has been updated to use `npx` prefix, which automatically uses the local version:

**Before (Broken):**
```json
"proto": "pbjs -t static-module -w es6 -o src/proto/signaling.js ../tango-signaling/src/proto/signaling.proto"
```

**After (Fixed):**
```json
"proto": "mkdir -p src/proto && npx pbjs -t static-module -w es6 -o src/proto/signaling.js ../tango-signaling/src/proto/signaling.proto && npx pbts -o src/proto/signaling.d.ts src/proto/signaling.js"
```

## How to Fix

### Step 1: Update Your Local Copy
Make sure you have the latest package.json with the fix. Check it has `npx` in the proto script:

```bash
grep -A 1 '"proto"' package.json
# Should show: "proto": "... npx pbjs ... npx pbts ..."
```

### Step 2: Clean Install

```bash
# Remove old node_modules and build artifacts
rm -rf node_modules src/proto dist

# Reinstall dependencies
npm install
```

### Step 3: Build

```bash
npm run build
```

### Step 4: Verify

You should see output like:
```
> tango-signaling-server-tencent-scf@0.1.0 proto
> mkdir -p src/proto && npx pbjs ... && npx pbts ...

[proto generation output]

> tango-signaling-server-tencent-scf@0.1.0 build
> npm run proto && tsc

✓ Build complete
```

Check for generated files:
```bash
ls -la dist/
ls -la src/proto/
# Should show: index.js, scf-handler.js, signaling.js, etc.
```

---

## What is npx?

`npx` is a tool included with npm that:
- Finds executables in `node_modules/.bin/`
- Runs them without requiring global installation
- Works across Windows, macOS, and Linux

Example:
- `pbjs` (won't work - tries global)
- `npx pbjs` (works - uses local or downloads if needed)

---

## Why This Matters

| Method | Works | Pros | Cons |
|--------|-------|------|------|
| Global install `pbjs` | If installed globally | Fast once setup | Requires global install |
| Local only | If in node_modules | Works everywhere | Need `npx` prefix |
| `npx pbjs` | ✓ Always works | Works on any system | Slightly slower (first time) |

Using `npx` is the modern, recommended approach because:
- No need for global installation
- Works the same on Linux, macOS, Windows
- Each project can use different versions
- No "works on my machine but not on yours" issues

---

## Full Build Steps (If Still Not Working)

```bash
# 1. Navigate to project directory
cd tango-signaling-server-tencent-scf

# 2. Verify you're on Linux and have Node 18+
uname -a
node --version  # Should be v18+
npm --version   # Should be 8+

# 3. Clean everything
rm -rf node_modules src/proto dist package-lock.json

# 4. Reinstall fresh
npm install

# 5. Verify proto file exists
file ../tango-signaling/src/proto/signaling.proto
# Should output: ... regular file ...

# 6. Try building
npm run build

# 7. Check output
ls -la dist/ src/proto/
```

---

## Manual Proto Generation (If npm run proto Fails)

If the automated build still fails, try manually:

```bash
# 1. Ensure dependencies installed
npm install

# 2. Create output directory
mkdir -p src/proto

# 3. Run pbjs manually
npx pbjs -t static-module -w es6 \
  -o src/proto/signaling.js \
  ../tango-signaling/src/proto/signaling.proto

# 4. Run pbts manually
npx pbts -o src/proto/signaling.d.ts src/proto/signaling.js

# 5. Verify files created
ls -la src/proto/
# Should show: signaling.js and signaling.d.ts

# 6. Run TypeScript compiler
npx tsc

# 7. Verify build output
ls -la dist/
# Should show: index.js and scf-handler.js
```

---

## Verify Proto File Exists

On Linux, verify the proto file is accessible:

```bash
# From project root (tango-signaling-server-tencent-scf/)
file ../tango-signaling/src/proto/signaling.proto
# Output: ... regular file ...

# Check it's not empty
wc -l ../tango-signaling/src/proto/signaling.proto
# Should show > 10 lines

# Check read permissions
ls -l ../tango-signaling/src/proto/signaling.proto
# Should show -r--r--r-- (readable)
```

---

## Common Linux-Specific Issues

### Issue: "Permission denied"
```bash
# Make scripts executable
chmod +x prepare-deployment.sh
./prepare-deployment.sh
```

### Issue: "Not enough space"
```bash
# Check disk space
df -h
# Need at least 500 MB free
```

### Issue: "Too many open files"
```bash
# Increase file descriptor limit
ulimit -n 2048
npm install
npm run build
```

### Issue: "npm install hangs"
```bash
# Clear cache and retry
npm cache clean --force
npm install --no-optional
```

---

## What Changed in Your Project

Only one file was modified:

**File**: `package.json`
**Change**: Added `npx` prefix to `pbjs` and `pbts` commands in the `"proto"` script

That's it! Everything else stays the same.

---

## Test the Fix

After applying the fix:

```bash
# Clean
rm -rf node_modules src/proto dist

# Install fresh
npm install

# Build
npm run build

# Verify success
if [ -f dist/scf-handler.js ] && [ -f src/proto/signaling.js ]; then
  echo "✓ Build successful!"
else
  echo "✗ Build failed"
fi
```

---

## Still Having Issues?

1. **Check proto path**: Make sure `../tango-signaling/src/proto/signaling.proto` is correct from your current directory

2. **Verify proto file**: 
```bash
cat ../tango-signaling/src/proto/signaling.proto | head -5
# Should show protobuf definitions
```

3. **Check npm links**:
```bash
npm ls protobufjs-cli
# Should show: protobufjs-cli@1.1.3
```

4. **Manually test pbjs**:
```bash
npx pbjs --version
# Should output version number
```

5. **Full diagnostic**:
```bash
echo "=== Environment ===" && \
node --version && \
npm --version && \
echo "=== Proto file ===" && \
file ../tango-signaling/src/proto/signaling.proto && \
echo "=== Dependencies ===" && \
npm ls protobufjs-cli && \
echo "=== Testing npx ===" && \
npx pbjs --version
```

If the above shows all OK, the build should work!

---

## Summary

**What was wrong**: Build script used global `pbjs` without `npx`
**What's fixed**: Updated to use `npx pbjs` (uses local version)
**What you need to do**: Run `npm install && npm run build`
**Why it works**: `npx` finds tools in `node_modules/.bin/` automatically

Build should now work on Linux! 🐧
