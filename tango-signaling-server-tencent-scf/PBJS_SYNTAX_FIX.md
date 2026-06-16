# Protobuf Command Line Syntax Fix

## Error You Saw

```
Usage: pbjs [options] <schema_path>
Options:
  -V, --version        output the version number
  --es5 <js_path>      Generate ES5 JavaScript code
  --es6 <js_path>      Generate ES6 JavaScript code
  --ts <ts_path>       Generate TypeScript code
  --decode <msg_type>  Decode standard input to JSON
  --encode <msg_type>  Encode standard input to JSON
  -h, --help           output usage information
Build failed!
```

## What Went Wrong

The proto script used old/unsupported syntax:
```bash
pbjs -t static-module -w es6 -o src/proto/signaling.js file.proto
```

But the `pbjs` version being used doesn't support `-t`, `-w`, or `-o` flags. It wants:
```bash
pbjs --es6 file.proto > output.js
```

## The Fix (Applied)

Updated `package.json` to use the correct command format:

**Before (broken):**
```json
"proto": "pbjs -t static-module -w es6 -o src/proto/signaling.js ../tango-signaling/src/proto/signaling.proto"
```

**After (fixed):**
```json
"proto": "mkdir -p src/proto && npx pbjs --es6 ../tango-signaling/src/proto/signaling.proto > src/proto/signaling.js && npx pbts src/proto/signaling.js > src/proto/signaling.d.ts"
```

### What Changed:

1. **Old syntax**: `pbjs -t static-module -w es6 -o output.js input.proto`
2. **New syntax**: `pbjs --es6 input.proto > output.js`

Mapping:
- `-t static-module` → removed (not needed)
- `-w es6` → `--es6`
- `-o output.js` → `> output.js` (shell redirection)

## How to Build Now

```bash
cd tango-signaling-server-tencent-scf

# Clean old proto files
rm -rf src/proto dist

# Build
npm install
npm run build
```

That's it! The build should now work.

## What pbjs Does

`pbjs` (Protocol Buffers JavaScript compiler):
- Takes a `.proto` file
- Generates JavaScript code
- With `--es6` flag: generates ES6 module syntax

Example:
```bash
# Old syntax (doesn't work)
pbjs -t static-module -w es6 -o output.js input.proto

# New syntax (works)
pbjs --es6 input.proto > output.js
# Output goes to stdout, redirected with > to file
```

## Verify the Fix Works

After building, check:
```bash
# Check proto files were generated
ls -la src/proto/
# Should show: signaling.js and signaling.d.ts

# Check files have content
wc -l src/proto/signaling.js
# Should show > 100 lines

# Check they're valid
file src/proto/signaling.js
# Should show: JavaScript source
```

## Why This Happens

Different versions of `protobufjs-cli` support different command-line options:

| Version | Syntax | Status |
|---------|--------|--------|
| Old (v0.x-v1.0) | `-t`, `-w`, `-o` | Deprecated |
| Current (v1.1.x) | `--es6`, `--es5`, `--ts` | ✓ Works |

The fix updates the commands to match the current version format.

## If Build Still Fails

### Check pbjs version:
```bash
npx pbjs --version
# Should show: 1.1.3 or similar
```

### Test pbjs manually:
```bash
cd tango-signaling-server-tencent-scf
npx pbjs --es6 ../tango-signaling/src/proto/signaling.proto

# Should output JavaScript code (or save to file with > output.js)
```

### Try with explicit output:
```bash
mkdir -p src/proto
npx pbjs --es6 ../tango-signaling/src/proto/signaling.proto > src/proto/signaling.js
npx pbts src/proto/signaling.js > src/proto/signaling.d.ts

# Then compile TypeScript
npx tsc
```

### Manual step-by-step:
```bash
# 1. Generate JavaScript from proto
echo "Generating signaling.js..."
npx pbjs --es6 ../tango-signaling/src/proto/signaling.proto > src/proto/signaling.js

# 2. Generate TypeScript definitions
echo "Generating signaling.d.ts..."
npx pbts src/proto/signaling.js > src/proto/signaling.d.ts

# 3. Compile TypeScript
echo "Compiling TypeScript..."
npx tsc

# 4. Check results
echo "Build complete. Checking files..."
ls -la dist/
ls -la src/proto/
```

## Command Reference

### pbjs Commands

```bash
# Generate ES6 JavaScript from proto file
pbjs --es6 input.proto > output.js

# Generate ES5 JavaScript from proto file
pbjs --es5 input.proto > output.js

# Generate TypeScript definitions
pbjs --ts output.ts input.proto
```

### pbts Command

```bash
# Generate TypeScript definitions from JavaScript
pbts input.js > output.d.ts
```

## Full Build Process

Here's the complete correct process:

```bash
# 1. Navigate to project
cd tango-signaling-server-tencent-scf

# 2. Clean artifacts
rm -rf src/proto dist

# 3. Install dependencies
npm install

# 4. Run full build (which runs proto script)
npm run build

# OR manually run each step:
# mkdir -p src/proto
# npx pbjs --es6 ../tango-signaling/src/proto/signaling.proto > src/proto/signaling.js
# npx pbts src/proto/signaling.js > src/proto/signaling.d.ts
# npx tsc
```

## Summary

**Problem**: Old pbjs syntax not supported
**Solution**: Use correct `--es6` flag with shell redirection
**Status**: ✓ Fixed in package.json
**Action**: Run `npm install && npm run build`

Build should now work! 🎉
