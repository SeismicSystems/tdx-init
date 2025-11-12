package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sync"
	"time"
)

// InitConfig is the JSON structure expected from the POST request
type InitConfig struct {
	SSHKey     string            `json:"ssh_key"`
	CertbotEmail string          `json:"certbot_email,omitempty"`
	Domain     string            `json:"domain,omitempty"`
	CustomData map[string]string `json:"custom_data,omitempty"`
}

const configFile = "/etc/tdx-init/config.json"

func waitForKey() {
	// Check if LUKS container exists
	cmd := exec.Command("cryptsetup", "isLuks", devicePath)
	if cmd.Run() == nil {
		// LUKS exists, extract config from header
		log.Println("Found existing LUKS container, extracting config...")
		cmd := exec.Command("cryptsetup", "token", "export", "--token-id", "1", devicePath)
		output, err := cmd.Output()
		if err != nil {
			log.Fatalf("Error exporting LUKS token: %v", err)
		}

		var token Token
		if err := json.Unmarshal(output, &token); err != nil {
			log.Fatalf("Error parsing token JSON: %v", err)
		}

		keyData, ok := token.UserData["ssh_key"]
		if !ok {
			// Fallback to old format for backwards compatibility
			keyData, ok = token.UserData["metadata"]
			if !ok {
				log.Fatalln("Error: No SSH key found in token")
			}
		}

		// Write SSH key
		writeKey(string(keyData))

		// Write full config if it exists
		if configData, ok := token.UserData["config"]; ok {
			if err := os.MkdirAll(filepath.Dir(configFile), 0755); err != nil {
				log.Printf("Warning: Could not create config directory: %v", err)
			}
			if err := os.WriteFile(configFile, []byte(configData), 0600); err != nil {
				log.Printf("Warning: Could not write config file: %v", err)
			} else {
				log.Printf("Config extracted and written to %s", configFile)
			}
		}

		log.Printf("Key extracted from LUKS header and written to %s", keyFile)
		return
	}

	// No LUKS container, start HTTP server to wait for key
	log.Printf("No LUKS container found, starting HTTP server on port %s...", httpPort)
	log.Println("Waiting for config to be provided via HTTP...")

	done := make(chan struct{})

	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			w.WriteHeader(http.StatusMethodNotAllowed)
			fmt.Fprint(w, "Only POST method is allowed")
			return
		}

		body, err := io.ReadAll(r.Body)
		if err != nil {
			w.WriteHeader(http.StatusBadRequest)
			fmt.Fprintf(w, "Error reading request: %v", err)
			return
		}

		// Try to parse as JSON first
		var config InitConfig
		if err := json.Unmarshal(body, &config); err != nil {
			// Not JSON, try legacy plain text format (just SSH key)
			key := string(body)
			matched, _ := regexp.MatchString(`^[A-Za-z0-9+/]{68}$`, key)
			if !matched {
				w.WriteHeader(http.StatusBadRequest)
				fmt.Fprint(w, "Invalid format. Expected JSON with ssh_key field or plain base64-encoded OpenSSH ed25519 public key")
				return
			}
			config.SSHKey = key
		} else {
			// Validate SSH key from JSON
			matched, _ := regexp.MatchString(`^[A-Za-z0-9+/]{68}$`, config.SSHKey)
			if !matched {
				w.WriteHeader(http.StatusBadRequest)
				fmt.Fprint(w, "Invalid ssh_key format, expected base64-encoded OpenSSH ed25519 public key")
				return
			}
		}

		// Write SSH key
		writeKey(config.SSHKey)

		// Save full config to disk
		if err := os.MkdirAll(filepath.Dir(configFile), 0755); err != nil {
			log.Printf("Warning: Could not create config directory: %v", err)
		}
		configJSON, _ := json.MarshalIndent(config, "", "  ")
		if err := os.WriteFile(configFile, configJSON, 0600); err != nil {
			log.Printf("Warning: Could not write config file: %v", err)
		} else {
			log.Printf("Config written to %s", configFile)
		}

		w.WriteHeader(http.StatusOK)
		fmt.Fprint(w, "Configuration received and stored successfully")

		close(done)
	})

	srv := &http.Server{Addr: ":" + httpPort}
	go srv.ListenAndServe()

	<-done

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	srv.Shutdown(ctx)

	log.Printf("Configuration received via HTTP and written to disk!")
}

var keyMu sync.Mutex

func writeKey(key string) {
	keyMu.Lock()
	defer keyMu.Unlock()

	os.MkdirAll(sshDir, 0700)

	// Set ownership of .ssh directory
	if err := os.Chown(sshDir, 1000, 1000); err != nil {
		log.Printf("Warning: Could not set ownership on .ssh dir: %v", err)
	}

	// Write authorized_keys with correct permissions
	authKeysFile := filepath.Join(sshDir, "authorized_keys")
	f, err := os.OpenFile(authKeysFile, os.O_CREATE|os.O_WRONLY, 0600)
	if err != nil {
		log.Fatalf("Error opening authorized_keys: %v", err)
	}
	defer f.Close()

	if _, err := f.WriteString("no-port-forwarding,no-agent-forwarding,no-X11-forwarding ssh-ed25519 " + key + "\n"); err != nil {
		log.Fatalf("Error writing to authorized_keys: %v", err)
	}

	// Set ownership of authorized_keys file
	if err := os.Chown(authKeysFile, 1000, 1000); err != nil {
		log.Printf("Warning: Could not set ownership on authorized_keys: %v", err)
	}

	// Write to separate key file (still needed for the system)
	if err := os.WriteFile(keyFile, []byte(key), 0600); err != nil {
		log.Fatalf("Error writing key file: %v", err)
	}
}
