# Build Troubleshooting Guide

## Error: "pbjs: not found" or "pbts: not found"

### Cause
The build script was trying to use `pbjs` and `pbts` commands that weren't installed globally on your system.

### Solution (FIXED)
The `package.json` has been updated to use `npx` which runs the local node_modules version:

```json
"proto": "mkdir -p src/proto && npx pbjs -t static-module -w es6 -o src/proto/signaling.js ../tango-signaling/src/proto/signaling.proto && npx pbts -o src/proto/signaling.d.ts src/proto/signaling.js"
```

The `npx` prefix tells npm to use locally installed tools from `node_modules/`.

### What to Do Now

**Step 1: Clean old node_modules**
```bash
rm -rf node_modules
```

**Step 2: Reinstall dependencies**
```bash
npm install
```

**Step 3: Try building again**
```bash
npm run build
```

You should see output like:
```
> tango-signaling-server-tencent-scf@0.1.0 proto
> mkdir -p src/proto && npx pbjs -t static-module -w es6 -o src/proto/signaling.js ../tango-signaling/src/proto/signaling.proto && npx pbts -o src/proto/signaling.d.ts src/proto/signaling.js

> tango-signaling-server-tencent-scf@0.1.0 build
> npm run proto && tsc

✓ Build successful
```

---

## Other Common Build Issues

### Error: "Cannot find module '../tango-signaling/src/proto/signaling.proto'"

**Cause**: The proto file path is incorrect or the tango-signaling directory doesn't exist

**Solution**:
1. Verify the directory structure:
```bash
ls -la ../tango-signaling/src/proto/signaling.proto
# Should show the file exists
```

2. If the path is different, update `package.json`:
```json
"proto": "mkdir -p src/proto && npx pbjs ... <correct-path>/signaling.proto ..."
```

3. Or update the path relative to your project location

---

### Error: "TypeScript compilation failed"

**Cause**: Type errors in the source code

**Solution**:
1. Check the error message for specific line numbers
2. Review `src/index.ts` or `src/scf-handler.ts` for the issue
3. Run type check separately:
```bash
npm run typecheck
```

This shows all type errors in detail.

---

### Error: "ENOENT: no such file or directory, scandir 'node_modules'"

**Cause**: Dependencies not installed

**Solution**:
```bash
npm install
```

Then try building again.

---

### Error: "Proto file syntax error"

**Cause**: The protobuf file has invalid syntax or the proto generation failed

**Solution**:
1. Check if the proto file is valid:
```bash
npx pbjs -t static-module ../tango-signaling/src/proto/signaling.proto
```

2. If error occurs, the proto file might be corrupted
3. Verify the file exists and is readable:
```bash
file ../tango-signaling/src/proto/signaling.proto
head ../tango-signaling/src/proto/signaling.proto
```

4. If needed, regenerate from the original source or check git history

---

### Error: "tsc: command not found"

**Cause**: TypeScript not installed

**Solution**:
```bash
npm install
# TypeScript should be installed as a dev dependency
npx tsc --version  # Verify it works
npm run build
```

---

## Complete Build Process (Step-by-Step)

If the above doesn't work, follow these exact steps:

### Linux/macOS:
```bash
# 1. Navigate to project
cd tango-signaling-server-tencent-scf

# 2. Clean everything
rm -rf node_modules src/proto dist

# 3. Reinstall dependencies
npm install

# 4. Verify proto file exists
ls -la ../tango-signaling/src/proto/signaling.proto

# 5. Run proto generation manually
npx pbjs -t static-module -w es6 \
  -o src/proto/signaling.js \
  ../tango-signaling/src/proto/signaling.proto

npx pbts -o src/proto/signaling.d.ts src/proto/signaling.js

# 6. Check proto files were created
ls -la src/proto/

# 7. Run TypeScript compiler
npx tsc

# 8. Verify build output
ls -la dist/

# 9. If all above succeeded, try npm run build
npm run build
```

### Windows (PowerShell):
```powershell
# 1. Navigate to project
cd tango-signaling-server-tencent-scf

# 2. Clean everything
Remove-Item -Path node_modules, src/proto, dist -Recurse -Force

# 3. Reinstall dependencies
npm install

# 4. Verify proto file exists
Get-Item -Path ../tango-signaling/src/proto/signaling.proto

# 5. Run proto generation manually
npx pbjs -t static-module -w es6 `
  -o src/proto/signaling.js `
  ../tango-signaling/src/proto/signaling.proto

npx pbts -o src/proto/signaling.d.ts src/proto/signaling.js

# 6. Check proto files were created
Get-Item -Path src/proto/

# 7. Run TypeScript compiler
npx tsc

# 8. Verify build output
Get-Item -Path dist/

# 9. If all above succeeded, try npm run build
npm run build
```

---

## Verifying Build Success

After `npm run build` completes, check:

```bash
# Should see compiled JavaScript files
ls -la dist/
# Expected output:
# -rw-r--r--  index.js
# -rw-r--r--  index.d.ts
# -rw-r--r--  scf-handler.js
# -rw-r--r--  scf-handler.d.ts

# Should see proto files generated
ls -la src/proto/
# Expected output:
# -rw-r--r--  signaling.js
# -rw-r--r--  signaling.d.ts
```

If you see these files, the build succeeded!

---

## Quick Fixes by Error Message

| Error Message | Fix |
|---|---|
| `pbjs: not found` | `npm install` (already fixed in package.json) |
| `Cannot find proto file` | Check file path, verify tango-signaling exists |
| `TS2307: Cannot find module` | Missing type definitions, run `npm install` |
| `TS7006: Parameter has implicit any type` | TypeScript strict mode issue in source code |
| `ENOENT: no such file` | Run `npm install` first |
| `Cannot read property 'proto' of undefined` | Corrupted node_modules, delete and reinstall |

---

## Still Not Working?

1. **Check Node.js version**:
```bash
node --version
# Should be 18+ (e.g., v18.0.0 or higher)
```

2. **Check npm version**:
```bash
npm --version
# Should be 8+ (e.g., 8.0.0 or higher)
```

3. **Clear npm cache**:
```bash
npm cache clean --force
npm install
npm run build
```

4. **Try fresh install on different directory**:
```bash
cd /tmp
git clone <your-repo>
cd tango-signaling-server-tencent-scf
npm install
npm run build
```

5. **Check available disk space**:
```bash
df -h
# Need at least 500 MB free
```

---

## If Build Still Fails

Provide these details when reporting:

```bash
# 1. Node and npm versions
node --version
npm --version

# 2. Full build output
npm run build 2>&1 | tee build.log

# 3. Directory listing
ls -la node_modules/protobufjs*
ls -la src/proto/
ls -la dist/

# 4. OS information
uname -a  # Linux/macOS
# or
systeminfo  # Windows
```

---

## Summary

**The fix**: Use `npx` to run locally installed CLI tools
**Updated in**: `package.json` scripts section
**What changed**: `pbjs` → `npx pbjs`, `pbts` → `npx pbts`
**Next step**: Run `npm install && npm run build`

Build should now work on Linux, macOS, and Windows!
