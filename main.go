package main

import (
	"bufio"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"math/big"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/btcsuite/btcd/btcec/v2"
	"github.com/btcsuite/btcd/btcutil"
	"github.com/btcsuite/btcd/chaincfg"
)	

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

func main() {
	// Load wallet addresses
	walletAddresses, err := loadWalletAddresses()
	if err != nil {
		fmt.Printf("Error loading wallet addresses: %v\n", err)
		return
	}
	fmt.Printf("Loaded %d wallet addresses\n", len(walletAddresses))

	// Load ranges
	ranges, err := loadRanges()
	if err != nil {
		fmt.Printf("Error loading ranges: %v\n", err)
		return
	}
	fmt.Printf("Loaded %d ranges\n", len(ranges))

	// Prompt user for wallet number
	reader := bufio.NewReader(os.Stdin)
	fmt.Print("Enter wallet number (1-160): ")
	walletNumStr, _ := reader.ReadString('\n')
	walletNumStr = strings.TrimSpace(walletNumStr)
	walletNum, err := strconv.Atoi(walletNumStr)
	if err != nil || walletNum < 1 || walletNum > 160 {
		fmt.Println("Invalid wallet number. Please enter a number between 1 and 160.")
		return
	}

	// Get the wallet address for the selected number
	walletIndex := walletNum - 1
	if walletIndex >= len(walletAddresses) {
		fmt.Println("Wallet index out of range.")
		return
	}
	targetAddress := walletAddresses[walletIndex]
	
	// Get the range for the selected wallet
	if walletIndex >= len(ranges) {
		fmt.Println("Range index out of range.")
		return
	}
	selectedRange := ranges[walletIndex]

	fmt.Printf("Selected Wallet: %s\n", targetAddress)
	fmt.Printf("Range: min=%s, max=%s\n", selectedRange.Min, selectedRange.Max)

	// Convert hex strings to big int
	minKey := new(big.Int)
	maxKey := new(big.Int)
	minKey.SetString(selectedRange.Min[2:], 16) // Remove 0x prefix
	maxKey.SetString(selectedRange.Max[2:], 16) // Remove 0x prefix

	// Iterate through the range
	fmt.Println("Starting key search...")
	currentKey := new(big.Int).Set(minKey)
	oneBI := big.NewInt(1)
	
	// Determine the limit for iterations to prevent infinite loops
	diff := new(big.Int).Sub(maxKey, minKey)
	limit := new(big.Int).Set(diff)
	// Limit to a reasonable number if the range is too large
	maxIterations := big.NewInt(1000000) // Limit to 1 million iterations
	if diff.Cmp(maxIterations) > 0 {
		limit = maxIterations
		fmt.Printf("Range is very large, limiting to %s iterations\n", maxIterations.String())
	}

	iterations := big.NewInt(0)
	startTime := time.Now()
	lastReportTime := startTime
	
	// Main loop
	for currentKey.Cmp(maxKey) <= 0 && iterations.Cmp(limit) < 0 {
		// Convert current big int to private key
		privateKeyBytes := padPrivateKey(currentKey.Bytes(), 32)
		
		// Generate address from private key
		address, err := privateKeyToAddress(privateKeyBytes)
		if err != nil {
			fmt.Printf("Error generating address: %v\n", err)
			return
		}
		
		// Check if it matches the target address
		if address == targetAddress {
			privateKeyHex := hex.EncodeToString(privateKeyBytes)
			fmt.Printf("MATCH FOUND!\n")
			fmt.Printf("Private Key: %s\n", privateKeyHex)
			fmt.Printf("Address: %s\n", address)
			return
		}
		
		// Progress reporting
		iterations.Add(iterations, oneBI)
		
		// Report speed every 10 seconds
		currentTime := time.Now()
		if currentTime.Sub(lastReportTime).Seconds() >= 10 {
			elapsedSeconds := currentTime.Sub(startTime).Seconds()
			iterationsFloat, _ := new(big.Float).SetInt(iterations).Float64()
			keysPerSecond := iterationsFloat / elapsedSeconds
			fmt.Printf("Checked %s keys (%.2f keys/sec)\n", iterations.String(), keysPerSecond)
			lastReportTime = currentTime
		}
		
		// Increment key
		currentKey.Add(currentKey, oneBI)
	}
	
	fmt.Printf("No match found after checking %s keys.\n", iterations.String())
}

// loadWalletAddresses loads wallet addresses from wallets.json
func loadWalletAddresses() ([]string, error) {
	file, err := os.Open("wallets.json")
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

// loadRanges loads ranges from ranges.json
func loadRanges() ([]Range, error) {
	file, err := os.Open("ranges.json")
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

// padPrivateKey ensures the private key is 32 bytes by padding with leading zeros
func padPrivateKey(key []byte, targetLength int) []byte {
	if len(key) >= targetLength {
		return key
	}
	padded := make([]byte, targetLength)
	copy(padded[targetLength-len(key):], key)
	return padded
}

// privateKeyToAddress converts a private key to a Bitcoin address
func privateKeyToAddress(privateKeyBytes []byte) (string, error) {
	// Convert private key bytes to btcec private key
	privateKey, _ := btcec.PrivKeyFromBytes(privateKeyBytes)
	
	// Get public key from private key
	publicKey := privateKey.PubKey()
	
	// Convert public key to address
	pubKeyHash := btcutil.Hash160(publicKey.SerializeCompressed())
	address, err := btcutil.NewAddressPubKeyHash(pubKeyHash, &chaincfg.MainNetParams)
	if err != nil {
		return "", err
	}
	
	return address.EncodeAddress(), nil
}
