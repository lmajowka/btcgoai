package main

import (
	"encoding/json"
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
