package main

import (
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"

	"github.com/btcsuite/btcd/btcutil"
	"github.com/btcsuite/btcd/chaincfg"
)

// WalletData represents the structure of the wallets.json file
type WalletData struct {
	Wallets []string `json:"wallets"`
}

// Hash160Data represents the structure of the hash160s.json file
type Hash160Data struct {
	Hash160s []string `json:"hash160s"`
}

func main() {
	// Get current directory
	currentDir, err := os.Getwd()
	if err != nil {
		fmt.Printf("Error getting current directory: %v\n", err)
		return
	}

	// Define paths
	projectDir := filepath.Dir(currentDir)
	walletsPath := filepath.Join(projectDir, "data", "wallets.json")
	outputPath := filepath.Join(projectDir, "data", "hash160s.json")

	// Load wallet addresses from data/wallets.json
	walletAddresses, err := loadWalletAddresses(walletsPath)
	if err != nil {
		fmt.Printf("Error loading wallet addresses: %v\n", err)
		return
	}

	// Convert addresses to hash160 values
	hash160s := make([]string, 0, len(walletAddresses))
	for _, addr := range walletAddresses {
		hash160, err := addressToHash160(addr)
		if err != nil {
			fmt.Printf("Warning: Unable to convert address %s: %v\n", addr, err)
			continue
		}
		hash160s = append(hash160s, hash160)
	}

	// Create the hash160s.json file
	hash160Data := Hash160Data{
		Hash160s: hash160s,
	}

	jsonData, err := json.MarshalIndent(hash160Data, "", "    ")
	if err != nil {
		fmt.Printf("Error marshaling hash160 data: %v\n", err)
		return
	}

	err = ioutil.WriteFile(outputPath, jsonData, 0644)
	if err != nil {
		fmt.Printf("Error writing hash160s.json: %v\n", err)
		return
	}

	fmt.Printf("Successfully created %s with %d hash160 values\n", outputPath, len(hash160s))
}

// addressToHash160 converts a Bitcoin address to its hash160 representation as a hex string
func addressToHash160(addrStr string) (string, error) {
	// Decode the address
	addr, err := btcutil.DecodeAddress(addrStr, &chaincfg.MainNetParams)
	if err != nil {
		return "", fmt.Errorf("invalid address: %v", err)
	}

	// Extract the hash160
	if addr.IsForNet(&chaincfg.MainNetParams) {
		switch a := addr.(type) {
		case *btcutil.AddressPubKeyHash:
			return hex.EncodeToString(a.Hash160()[:]), nil
		case *btcutil.AddressScriptHash:
			return hex.EncodeToString(a.Hash160()[:]), nil
		case *btcutil.AddressPubKey:
			return hex.EncodeToString(a.AddressPubKeyHash().Hash160()[:]), nil
		default:
			return "", fmt.Errorf("unsupported address type")
		}
	}

	return "", fmt.Errorf("address is not for mainnet")
}

// loadWalletAddresses loads wallet addresses from wallets.json
func loadWalletAddresses(filePath string) ([]string, error) {
	file, err := os.Open(filePath)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	var walletData WalletData
	decoder := json.NewDecoder(file)
	if err := decoder.Decode(&walletData); err != nil {
		return nil, err
	}

	return walletData.Wallets, nil
}
