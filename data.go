package main

import (
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
)

// loadWalletAddresses loads wallet addresses from data/wallets.json
func loadWalletAddresses() ([]string, error) {
	file, err := os.Open("data/wallets.json")
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

// loadRanges loads ranges from data/ranges.json
// loadWalletHash160s loads wallet hash160 values from data/hash160s.json
func loadWalletHash160s() ([][]byte, error) {
	file, err := os.Open("data/hash160s.json")
	if err != nil {
		// If the dedicated hash160s file doesn't exist, try converting from addresses
		return convertAddressesToHash160()
	}
	defer file.Close()

	var hash160Data Hash160Data
	decoder := json.NewDecoder(file)
	if err := decoder.Decode(&hash160Data); err != nil {
		return nil, err
	}

	// Convert hex strings to byte slices
	result := make([][]byte, len(hash160Data.Hash160s))
	for i, hexStr := range hash160Data.Hash160s {
		hash160Bytes, err := hex.DecodeString(hexStr)
		if err != nil {
			return nil, err
		}
		result[i] = hash160Bytes
	}

	return result, nil
}

// convertAddressesToHash160 is a fallback function that loads wallet addresses and
// converts them to hash160 values
func convertAddressesToHash160() ([][]byte, error) {
	_, err := loadWalletAddresses()
	if err != nil {
		return nil, err
	}

	// This is a stub implementation that should be replaced with actual conversion logic
	// In a real implementation, you would use btcutil to convert each address to hash160
	// For now, we'll just return an error to indicate this isn't implemented
	return nil, fmt.Errorf("conversion from addresses to hash160 not implemented, please create data/hash160s.json")
}

func loadRanges() ([]Range, error) {
	file, err := os.Open("data/ranges.json")
	if err != nil {
		return nil, err
	}
	defer file.Close()

	var rangeData RangeData
	decoder := json.NewDecoder(file)
	if err := decoder.Decode(&rangeData); err != nil {
		return nil, err
	}

	return rangeData.Ranges, nil
}
