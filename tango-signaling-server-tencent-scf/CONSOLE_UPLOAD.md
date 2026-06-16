# Tencent Cloud Console - ZIP Upload Deployment Guide

This guide shows how to deploy using the Tencent Cloud Console by uploading a ZIP file.

## Prerequisites

- Tencent Cloud account with SCF enabled
- Node.js 18+ installed locally
- The `tango-signaling-server-tencent-scf` project

## Step 1: Prepare the Deployment Package

### Option A: Using the Provided Script (Recommended)

**On Windows (PowerShell):**
```powershell
# Navigate to project directory
cd tango-signaling-server-tencent-scf

# Run the preparation script
.\prepare-deployment.ps1

# Or specify custom output path
.\prepare-deployment.ps1 -OutputPath "C:\deployments\my-package.zip"
```

**On macOS/Linux:**
```bash
# Navigate to project directory
cd tango-signaling-server-tencent-scf

# Make script executable
chmod +x prepare-deployment.sh

# Run the preparation script
./prepare-deployment.sh

# Or specify custom output path
./prepare-deployment.sh "./my-custom-package.zip"
```

The script will:
1. ✓ Build TypeScript to JavaScript
2. ✓ Remove dev dependencies
3. ✓ Create optimized ZIP file
4. ✓ Verify all required files are included

**Output:**
- File: `tango-signaling-server-scf.zip`
- Includes: `dist/`, `node_modules/`, `package.json`, `package-lock.json`
- Size: ~10-15 MB typical

### Option B: Manual Preparation

If you prefer not to use the script:

```bash
# 1. Build the project
npm install
npm run build

# 2. Install production-only dependencies
rm -rf node_modules
npm install --omit=dev

# 3. Create ZIP manually
# Windows PowerShell:
Compress-Archive -Path dist, node_modules, package.json, package-lock.json `
  -DestinationPath tango-signaling-server-scf.zip

# Or macOS/Linux:
zip -r tango-signaling-server-scf.zip dist node_modules package.json package-lock.json
```

## Step 2: Upload via Tencent Cloud Console

### 1. Navigate to SCF

1. Open [Tencent Cloud Console](https://console.cloud.tencent.com/)
2. Search for "SCF" or "Serverless Cloud Function"
3. Click on "Functions" → "Create Function"

### 2. Configure Function

Fill in the function configuration:

| Setting | Value |
|---------|-------|
| **Function Name** | `tango-signaling-server` |
| **Region** | `ap-shanghai` (or your preferred region) |
| **Runtime** | `Node.js 18` (IMPORTANT) |
| **Handler** | `dist/scf-handler.handler` (IMPORTANT) |
| **Memory** | `512 MB` |
| **Timeout** | `30` seconds |
| **Temporary Storage** | `/tmp` (default, read-only elsewhere) |

### 3. Upload Code

**Method 1: ZIP Upload**

1. Under "Code upload method", select **"Upload ZIP file"**
2. Click **"Upload"** button
3. Select your `tango-signaling-server-scf.zip` file
4. Wait for upload to complete (may take 30-60 seconds depending on file size)

**Method 2: Local ZIP Upload**

If file is small enough (<10 MB):
1. Under "Code upload method", select **"Upload ZIP file"**
2. Click **"Upload"** button directly

### 4. Configure Environment Variables (Optional)

If using a custom TURN server:

1. Scroll to "Environment variables" section
2. Add:
   - **Name**: `TURN_ADDR`, **Value**: `your-turn-server.com`
   - **Name**: `TURN_USER`, **Value**: `your-username`
   - **Name**: `TURN_CREDENTIAL`, **Value**: `your-credential`

Leave blank to use default Google STUN servers.

### 5. Network Configuration (Optional)

For database access or VPC connectivity:

1. Scroll to "Network configuration"
2. Select your VPC if needed
3. Leave blank for public internet access only

### 6. Create Function

1. Click **"Complete"** or **"Create"** button
2. Wait for function to be created (usually 10-30 seconds)
3. You'll see a success message with function details

## Step 3: Verify Deployment

### 1. Test the Function

Once created, you can test immediately:

1. Go to "Functions" → Your function name → "Test"
2. Under "Test method", select **"Simulate test data"**
3. In the event JSON, replace with:

```json
{
  "path": "/ok",
  "httpMethod": "GET",
  "headers": {}
}
```

4. Click **"Run test"**
5. You should see response: `{"statusCode": 200, "body": "ok"}`

### 2. View Logs

1. Go to "Logs" tab
2. You should see recent invocations
3. Check for any errors

### 3. Get Function URL

1. Go to "Function details" tab
2. Find "Function URL" section
3. Note the URL for testing

## Step 4: Configure WebSocket (API Gateway)

For WebSocket support, you need to set up API Gateway:

### 1. Create API Gateway

1. Go to API Gateway → Create
2. Select **WebSocket** protocol
3. Name it: `tango-signaling-api`
4. Click **Create**

### 2. Create Routes

Add three routes in your WebSocket API:

**Route 1: Connection**
- Route name: `$connect`
- Backend type: `SCF`
- Function: Select your `tango-signaling-server` function

**Route 2: Default (Messages)**
- Route name: `$default`
- Backend type: `SCF`
- Function: Select your `tango-signaling-server` function

**Route 3: Disconnect**
- Route name: `$disconnect`
- Backend type: `SCF`
- Function: Select your `tango-signaling-server` function

### 3. Deploy API

1. Click **Deploy**
2. Select environment: `Release` (or `Test`)
3. Click **Submit**

### 4. Get WebSocket URL

After deployment:
1. Go to API Gateway → Your API → Overview
2. Find **WebSocket URL** (looks like `wss://xxxxxxxx.execute-api.ap-shanghai.tencentapi.com/release`)
3. This is your client connection URL

**Example WebSocket URL for clients:**
```
wss://xxxxxxxx.execute-api.ap-shanghai.tencentapi.com/release?session_id=my-session-123
```

## Step 5: Test WebSocket Connection

### Using wscat (Command Line)

```bash
# Install if needed
npm install -g wscat

# Connect to your WebSocket
wscat -c "wss://your-api-url?session_id=test-session"

# You should see:
# Connected (press CTRL+C to quit)
# < Hello message with ICE servers
```

### Using JavaScript Client

```javascript
// Create WebSocket connection
const sessionId = "test-session-" + Date.now();
const ws = new WebSocket(
  `wss://your-api-url?session_id=${sessionId}`
);

ws.onopen = () => {
  console.log("Connected!");
};

ws.onmessage = (event) => {
  console.log("Received:", event.data);
};

ws.onerror = (error) => {
  console.error("Error:", error);
};
```

## Troubleshooting

### Upload Fails

**Error: "File too large"**
- ZIP size must be < 50 MB
- Run the preparation script again to optimize

**Error: "Invalid file format"**
- Make sure you're uploading a valid ZIP file
- Windows: Use `Compress-Archive` or WinRAR
- macOS/Linux: Use `zip` command

### Function Creation Fails

**Error: "Invalid handler"**
- Check handler is exactly: `dist/scf-handler.handler`
- Make sure `dist/scf-handler.js` exists in ZIP

**Error: "Runtime not supported"**
- Select `Node.js 18` from the dropdown
- Other Node.js versions may not have all required modules

### WebSocket Connection Fails

**Error: "403 Forbidden"** or **Connection timeout**
- Verify API Gateway WebSocket integration is created
- Check routes are properly mapped to SCF function
- Wait 2-3 minutes after API deployment

**Error: "405 Method Not Allowed"**
- Verify `$connect` route exists and is mapped
- Check function timeout is at least 30 seconds

### High Latency

**Problem: Slow connections or TURN failures**
- Check TURN server is reachable: `ping your-turn-server.com`
- Try with Google STUN (default) first
- Monitor SCF function metrics for duration

## Performance Tuning

### Memory Allocation

Adjust based on load:
- **256 MB**: Light testing, single user
- **512 MB**: Default, handles ~10 concurrent connections
- **1024+ MB**: High traffic, many concurrent sessions

To increase:
1. Go to "Function details"
2. Click "Edit"
3. Change "Memory size"
4. Save

### Timeout

Default 30 seconds is good for WebSocket. Adjust if needed:
1. Go to "Function details"
2. Click "Edit"
3. Change "Timeout"
4. Save (max 900 seconds for SCF)

## Updating Your Code

To update the function after deployment:

1. Make changes to `src/` files
2. Run `npm run build` to compile
3. Create new ZIP: `prepare-deployment.ps1` or `prepare-deployment.sh`
4. Go to function → "Upload ZIP"
5. Select the new ZIP file
6. Function updates automatically

## Monitoring

### View Metrics

1. Go to function → "Monitoring"
2. Check:
   - **Invocation count** - Number of requests
   - **Duration** - Average execution time
   - **Error count** - Failed invocations
   - **Memory usage** - Peak memory used

### View Logs

1. Go to function → "Logs"
2. Filter by date/time range
3. Search for errors: `"error"` or `"failed"`

## Cost Estimation

Tencent Cloud SCF pricing (approximate):
- **Invocations**: ¥0.0000002 per invocation
- **Execution time**: ¥0.00001667 per GB-second
- **Free tier**: 1M invocations + 400,000 GB-seconds per month

With 512 MB memory:
- 1 second execution = 0.256 GB-seconds
- 1000 WebSocket connections/day ≈ ¥0.2-1 per day (rough estimate)

Check [Tencent Cloud Pricing](https://cloud.tencent.com/document/product/583/12282) for current rates.

## Next Steps

✅ Function deployed and running
✅ WebSocket API configured
✅ Testing successful

Now you can:
1. Point your clients to the WebSocket URL
2. Monitor performance in the console
3. Scale memory/timeout as needed
4. Set up alerts for errors

## Support

For issues:
1. Check "Logs" tab in SCF console
2. Review this guide's troubleshooting section
3. Refer to main [DEPLOYMENT.md](./DEPLOYMENT.md)
4. Check [Tencent Cloud SCF Documentation](https://cloud.tencent.com/document/product/583)
