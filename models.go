package main

// WalletData represents the structure of the wallets.json file
type WalletData struct {
	Wallets []string `json:"wallets"`
}

// RangeData represents the structure of the ranges.json file
type RangeData struct {
	Ranges []Range `json:"ranges"`
}

// Range represents a single range in the ranges.json file
type Range struct {
	Min    string `json:"min"`
	Max    string `json:"max"`
	Status int    `json:"status"`
}

// Hash160Data represents the structure of the hash160s.json file
type Hash160Data struct {
	Hash160s []string `json:"hash160s"`
}
