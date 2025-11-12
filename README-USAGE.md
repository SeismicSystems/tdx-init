# Extended tdx-init with Multiple SSH Keys & Domain Config

This fork adds support for:
- **Multiple SSH keys** (array)
- **Domain configuration** (certbot email and domain name)

## Changes Made

1. **keys.go**:
   - Support for multiple SSH keys in `ssh_keys` array
   - Structured `domain` object with `email` and `name`
2. **passphrase.go**: Embeds full config in LUKS header for persistence

## Usage

### POST SSH Keys and Domain Config

Send initialization config:

```bash
curl -X POST http://<VM_IP>:8080 \
  -H "Content-Type: application/json" \
  -d '{
    "ssh_keys": [
      "AAAAC3NzaC1lZDI1NTE5AAAAIMPdKdQZip5rYQAhuKTbhI09HM9aFSU...",
      "AAAAC3NzaC1lZDI1NTE5AAAAIG7J8xK9hXQwMVlBzMzBcNkK3xLhKk..."
    ],
    "domain": {
      "email": "certbot@example.com",
      "name": "genesis-node-1.example.com"
    }
  }'
```

Or from a file:

```bash
curl -X POST http://<VM_IP>:8080 \
  -H "Content-Type: application/json" \
  -d @config.json
```

### Legacy Support (Plain SSH Key)

Still supported for backwards compatibility:

```bash
curl -X POST -d "$(cut -d' ' -f2 ~/.ssh/id_ed25519.pub)" http://<VM_IP>:8080
```

## What Gets Stored

1. **SSH Keys**: All keys written to `/home/searcher/.ssh/authorized_keys`
   - First key also written to `/etc/searcher_key` for container compatibility
2. **Config**: Saved to `/etc/tdx-init/config.json`
3. **LUKS Header**: Config embedded for persistence across reboots

## Accessing Config Data

**Main config:**
```bash
cat /etc/tdx-init/config.json
```

Output:
```json
{
  "ssh_keys": [
    "AAAAC3NzaC1lZDI1NTE5AAAAIMPd...",
    "AAAAC3NzaC1lZDI1NTE5AAAAIG7J..."
  ],
  "domain": {
    "email": "certbot@example.com",
    "name": "genesis-node-1.example.com"
  }
}
```

**Parsing with jq:**
```bash
# Get domain email
EMAIL=$(jq -r '.domain.email' /etc/tdx-init/config.json)

# Get domain name
DOMAIN=$(jq -r '.domain.name' /etc/tdx-init/config.json)

# Get first SSH key
FIRST_KEY=$(jq -r '.ssh_keys[0]' /etc/tdx-init/config.json)

# Count SSH keys
KEY_COUNT=$(jq '.ssh_keys | length' /etc/tdx-init/config.json)
```

## Integration with flashbots-images

Update `bob-common/mkosi.build`:

```bash
make_git_package \
    "tdx-init" \
    "seismic-custom" \
    "https://github.com/SeismicSystems/tdx-init" \
    'go build -trimpath -ldflags "-s -w -buildid=" -o ./build/tdx-init' \
    "build/tdx-init:/usr/bin/tdx-init"
```

## Endpoints

- `POST /` - Initialize with SSH keys and domain config

## Backwards Compatibility

- Plain-text SSH key POSTs still work
- Old LUKS headers with "metadata" field are supported
- All new fields are optional
