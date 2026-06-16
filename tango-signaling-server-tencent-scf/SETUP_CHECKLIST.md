# Tango Signaling Server SCF - Setup & Deployment Checklist

## Pre-Deployment Setup

### Prerequisites
- [ ] Node.js 18+ installed (`node --version`)
- [ ] npm installed (`npm --version`)
- [ ] Tencent Cloud account created
- [ ] Tencent Cloud API access (Secret ID & Secret Key)

### Local Setup (5 minutes)
```bash
# 1. Install dependencies
npm install

# 2. Generate protobuf types and compile
npm run build

# 3. Test locally
npm run dev
# Server should start on http://localhost:3000
# In another terminal: curl http://localhost:3000/ok
```

- [ ] Dependencies installed without errors
- [ ] Build completes successfully
- [ ] Local dev server starts
- [ ] Health check endpoint responds with "ok"

### Configuration

**Environment Setup**
```bash
# Copy template
cp .env.example .env.local

# Edit .env.local with your settings (optional)
# TURN_ADDR=your-turn-server.com
# TURN_USER=username
# TURN_CREDENTIAL=password
```

- [ ] `.env.local` created (optional for local dev)
- [ ] TURN server credentials configured (if using custom TURN)

**Project Configuration Review**
- [ ] Verify `package.json` has all dependencies
- [ ] Check `tsconfig.json` for Node.js settings
- [ ] Review `serverless.yml` for correct region (default: ap-shanghai)
- [ ] Confirm handler path: `dist/scf-handler.handler`

## Deployment Options

### Option 1: Serverless Framework (Recommended) ⭐

**Installation**
```bash
npm install -g serverless
```

- [ ] Serverless Framework installed globally
- [ ] Version check: `serverless --version` (should be v3+)

**Tencent Cloud Credentials**
```bash
serverless credentials --provider tencentcloud
# Enter your Tencent Cloud Secret ID and Secret Key
```

- [ ] Credentials configured and saved
- [ ] Test credentials: `serverless deploy` (should attempt deployment)

**Deploy**
```bash
npm run deploy
# Or manually: npm run build && serverless deploy
```

- [ ] Build completes without errors
- [ ] Serverless Framework deploys successfully
- [ ] Function URL returned in output
- [ ] Note the function endpoint for testing

**Verification**
```bash
serverless logs -f websocket
# Should show recent function invocations
```

- [ ] Can view function logs
- [ ] No errors in recent logs

---

### Option 2: Tencent Cloud Console (Manual Upload)

**Preparation**
```bash
npm run build
# Creates dist/ with compiled JavaScript
```

- [ ] Build completes
- [ ] `dist/` directory created

**Package Code**
```bash
# Create ZIP file
npm install --omit=dev  # Remove dev dependencies
# Then manually create ZIP with: dist/, node_modules/, package.json
```

- [ ] ZIP file created with required files
- [ ] ZIP file size reasonable (<50MB for Lambda/SCF)

**Upload via Console**
1. Go to: Tencent Cloud → SCF → Functions → Create
2. Configure:
   - **Runtime**: Node.js 18
   - **Handler**: `dist/scf-handler.handler`
   - **Memory**: 512 MB
   - **Timeout**: 30 seconds
3. Upload ZIP file

- [ ] Function created in Tencent Cloud console
- [ ] Handler path correct
- [ ] Memory and timeout configured

---

### Option 3: Tencent Cloud CLI

**Installation & Setup**
```bash
npm install -g tencentcloud-cli
tencentcloud configure
# Enter credentials interactively
```

- [ ] Tencent Cloud CLI installed
- [ ] Credentials configured

**Deploy**
```bash
npm run build
tencentcloud scf CreateFunction \
  --FunctionName tango-signaling \
  --Runtime nodejs18 \
  --Handler dist/scf-handler.handler \
  --Code S3Bucket=your-bucket,S3Key=function.zip
```

- [ ] Function deployed via CLI
- [ ] Function appears in console

---

## Post-Deployment Setup

### API Gateway Configuration (For WebSocket)

**Create WebSocket API**
1. Tencent Cloud Console → API Gateway → Create
2. Protocol: WebSocket
3. Create routes:
   - `$connect` (client connects)
   - `$default` (client sends message)
   - `$disconnect` (client disconnects)
4. Map each route to your SCF function

- [ ] WebSocket API created
- [ ] Routes configured
- [ ] Routes mapped to SCF function
- [ ] API Gateway endpoint obtained

**Test Connection**
```bash
# Install wscat if needed
npm install -g wscat

# Connect to your WebSocket endpoint
wscat -c "wss://your-api-gateway-url/?session_id=test-session"
# Should connect and receive Hello message
```

- [ ] WebSocket connection successful
- [ ] Server responds with Hello packet

### Environment Variables (Production)

If using custom TURN server, set in Tencent Cloud console:
- `TURN_ADDR`: Your TURN server address
- `TURN_USER`: TURN username
- `TURN_CREDENTIAL`: TURN password
- `NODE_ENV`: "production"

- [ ] Environment variables set in SCF console (if needed)
- [ ] Function redeployed after config changes

---

## Testing & Validation

### Health Check
```bash
curl https://your-function-url/ok
# Should return: ok
```

- [ ] Health check endpoint responds
- [ ] Returns expected "ok" response

### WebSocket Connection Test
Using `wscat` or a WebSocket client:
1. Connect with: `wss://your-endpoint/?session_id=test-123`
2. Should receive Hello message with ICE servers
3. Connection should remain open

- [ ] WebSocket upgrade succeeds
- [ ] Hello message received with ICE servers
- [ ] Connection stays open (no immediate close)

### Logging & Monitoring
```bash
serverless logs -f websocket  # If using Serverless Framework
# Or check Tencent Cloud console → Logs
```

- [ ] Can access function logs
- [ ] Logs show successful connections
- [ ] No error messages in recent logs

---

## Performance Tuning

### Memory Configuration
Current: **512 MB** (default)
- For light load: 256 MB
- For moderate load: 512 MB (current)
- For high traffic: 1024+ MB

- [ ] Memory setting appropriate for expected load
- [ ] Monitored performance in logs

### Timeout Configuration
Current: **30 seconds** (default)
- Signaling timeouts typically < 5 seconds
- Set to 30 seconds for safety
- WebSocket connections may hold longer

- [ ] Timeout sufficient for expected session duration
- [ ] No timeout-related errors in logs

### Concurrency
- [ ] Reserved concurrency set (if needed for consistent performance)
- [ ] Auto-scaling enabled (default)

---

## Production Readiness

### State Management
- [ ] Currently: In-memory (suitable for testing)
- [ ] For production: Plan Redis upgrade
- [ ] Redis connection details: (if applicable)

### Security
- [ ] Environment secrets not committed to git
- [ ] `.gitignore` properly configured
- [ ] Credentials only in Tencent Cloud console or .env.local

### Monitoring & Alerts
- [ ] CloudWatch/Logs configured
- [ ] Error alerts set up
- [ ] Performance baseline established

### Disaster Recovery
- [ ] Multiple regions deployed (if HA required)
- [ ] Backup TURN server configured
- [ ] Failover procedure documented

---

## Common Issues & Solutions

### Build Errors
```bash
npm install
npm run build
# Should complete without errors
```

**If fails:**
1. Check Node.js version: `node --version` (need 18+)
2. Delete node_modules and package-lock.json
3. Run: `npm install && npm run build`

- [ ] Build error resolved

### Deployment Errors
```bash
# Check Serverless Framework logs
serverless deploy --verbose
```

**Common issues:**
- Credentials invalid → Re-run: `serverless credentials --provider tencentcloud`
- Region error → Update region in `serverless.yml`
- Package too large → Remove unnecessary dependencies

- [ ] Deployment error identified and resolved

### WebSocket Connection Fails
1. Verify API Gateway WebSocket integration
2. Check function timeout ≥ 30s
3. Review SCF logs for errors

```bash
serverless logs -f websocket
```

- [ ] WebSocket issue diagnosed in logs
- [ ] Resolution applied

### High Latency Issues
- Increase memory allocation
- Check STUN/TURN server response times
- Monitor function duration in logs

- [ ] Latency within acceptable range

---

## Rollback Procedure

If issues occur:

**Serverless Framework:**
```bash
# Revert to previous version
git checkout HEAD~ -- serverless.yml src/
npm run deploy
```

**Tencent Cloud Console:**
1. Navigate to SCF → Function → Versions
2. Select previous version
3. Update alias to point to stable version

---

## Maintenance & Updates

### Regular Tasks
- [ ] Check logs weekly for errors
- [ ] Monitor function performance metrics
- [ ] Review WebSocket connection statistics

### Periodic Updates
- [ ] Update npm dependencies: `npm update`
- [ ] Review security advisories: `npm audit`
- [ ] Test new Tencent Cloud features

### Version Management
- [ ] Tag deployment version: `git tag v1.0.0-scf`
- [ ] Document changes in CHANGELOG
- [ ] Keep deployment notes updated

---

## Success Criteria

✅ **Deployment Complete When:**
- [ ] Local build succeeds: `npm run build`
- [ ] Local test works: `npm run dev` → `curl localhost:3000/ok`
- [ ] Deployed to Tencent Cloud SCF
- [ ] API Gateway WebSocket configured
- [ ] WebSocket connection test successful
- [ ] Health endpoint responds correctly
- [ ] Logs show successful operation
- [ ] No errors in monitoring dashboard

---

## Quick Reference Commands

```bash
# Local Development
npm install                 # Install dependencies
npm run build              # Build the project
npm run dev                # Run locally (http://localhost:3000)
npm run typecheck          # Check TypeScript types

# Deployment
npm run deploy             # Deploy via Serverless Framework
serverless remove          # Remove deployment
serverless logs -f websocket  # View function logs

# Tencent Cloud
serverless credentials --provider tencentcloud  # Configure credentials
serverless invoke -f websocket -l               # Invoke function

# Testing
curl http://localhost:3000/ok                   # Health check (local)
wscat -c "wss://endpoint/?session_id=test"     # WebSocket test
```

---

## Support & Resources

- **Quick Setup**: See [QUICKSTART.md](./QUICKSTART.md)
- **Detailed Guide**: See [DEPLOYMENT.md](./DEPLOYMENT.md)
- **Architecture**: See [README.md](./README.md)
- **Tencent Cloud Docs**: https://cloud.tencent.com/document/product/583
- **Serverless Framework**: https://www.serverless.com/framework

---

**Status**: Ready for Deployment! 🚀

Start with the **Serverless Framework** option for easiest setup.
