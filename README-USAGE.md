# Extended tdx-init with JSON Config Support

This fork adds support for POSTing JSON configuration data to port 8080, including:
- SSH key
- Certbot email and domain
- Arbitrary custom data

## Changes Made

1. **keys.go**: Extended HTTP handler to accept JSON POST with config data
2. **passphrase.go**: Modified to embed full config in LUKS header
3. Added `InitConfig` struct for structured configuration

## Usage

### Option 1: JSON POST (New Method)

Send a JSON payload with your config:

```bash
curl -X POST http://<VM_IP>:8080 \
  -H "Content-Type: application/json" \
  -d '{
    "ssh_key": "AAAAC3NzaC1lZDI1NTE5AAAAIMPdKdQZip5rYQAhuKTbhI09HM9aFSU...",
    "certbot_email": "admin@example.com",
    "domain": "genesis-node-1.example.com",
    "custom_data": {
      "environment": "production",
      "region": "us-east-2",
      "node_id": "1"
    }
  }'
```

Or from a file:

```bash
curl -X POST http://<VM_IP>:8080 \
  -H "Content-Type: application/json" \
  -d @config.json
```

### Option 2: Plain SSH Key (Legacy, Still Supported)

```bash
curl -X POST -d "$(cut -d' ' -f2 ~/.ssh/id_ed25519.pub)" http://<VM_IP>:8080
```

### What Gets Stored

1. **SSH Key**: Written to `/etc/searcher_key` and `/home/searcher/.ssh/authorized_keys`
2. **Full Config**: Saved to `/etc/tdx-init/config.json` (readable as JSON)
3. **LUKS Header**: Both SSH key and full config embedded for persistence across reboots

### Accessing Config Data

After initialization, your config is available at:

```bash
cat /etc/tdx-init/config.json
```

Example output:
```json
{
  "ssh_key": "AAAAC3NzaC1lZDI1NTE5AAAAIMPd...",
  "certbot_email": "admin@example.com",
  "domain": "genesis-node-1.example.com",
  "custom_data": {
    "environment": "production",
    "region": "us-east-2",
    "node_id": "1"
  }
}
```

You can parse this in your scripts:

```bash
# Get certbot email
CERTBOT_EMAIL=$(jq -r '.certbot_email' /etc/tdx-init/config.json)

# Get domain
DOMAIN=$(jq -r '.domain' /etc/tdx-init/config.json)

# Get custom data
NODE_ID=$(jq -r '.custom_data.node_id' /etc/tdx-init/config.json)
```

## Integration with flashbots-images

To use this fork in your builds, update `bob-common/mkosi.build`:

```bash
make_git_package \
    "tdx-init" \
    "your-branch-name" \
    "https://github.com/your-org/tdx-init" \
    'go build -trimpath -ldflags "-s -w -buildid=" -o ./build/tdx-init' \
    "build/tdx-init:/usr/bin/tdx-init"
```

## Backwards Compatibility

- Old plain-text SSH key POSTs still work
- Old LUKS headers with only "metadata" field are supported
- New features are optional (certbot_email, domain, custom_data can be omitted)
