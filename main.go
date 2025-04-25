package main

import (
	"bufio"
	"fmt"
	"math/big"
	"os"
	"strconv"
	"strings"
)

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

	searchForPrivateKey(minKey, maxKey, targetAddress)
}


