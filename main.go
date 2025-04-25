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
		fmt.Printf("%sError loading wallet addresses: %v%s\n", ColorRed, err, ColorReset)
		return
	}
	fmt.Printf("%sLoaded %d wallet addresses%s\n", ColorGreen, len(walletAddresses), ColorReset)

	// Load ranges
	ranges, err := loadRanges()
	if err != nil {
		fmt.Printf("%sError loading ranges: %v%s\n", ColorRed, err, ColorReset)
		return
	}
	fmt.Printf("%sLoaded %d ranges%s\n", ColorGreen, len(ranges), ColorReset)

	// Prompt user for wallet number
	reader := bufio.NewReader(os.Stdin)
	fmt.Printf("%sEnter wallet number (1-160):%s ", ColorCyan, ColorReset)
	walletNumStr, _ := reader.ReadString('\n')
	walletNumStr = strings.TrimSpace(walletNumStr)
	walletNum, err := strconv.Atoi(walletNumStr)
	if err != nil || walletNum < 1 || walletNum > 160 {
		fmt.Printf("%sInvalid wallet number. Please enter a number between 1 and 160.%s\n", ColorRed, ColorReset)
		return
	}

	// Get the wallet address for the selected number
	walletIndex := walletNum - 1
	if walletIndex >= len(walletAddresses) {
		fmt.Printf("%sWallet index out of range.%s\n", ColorRed, ColorReset)
		return
	}
	targetAddress := walletAddresses[walletIndex]
	
	// Get the range for the selected wallet
	if walletIndex >= len(ranges) {
		fmt.Printf("%sRange index out of range.%s\n", ColorRed, ColorReset)
		return
	}
	selectedRange := ranges[walletIndex]

	fmt.Printf("%sSelected Wallet: %s%s%s\n", ColorYellow, ColorBoldYellow, targetAddress, ColorReset)
	fmt.Printf("%sRange: min=%s%s%s, max=%s%s%s\n", ColorYellow, ColorBoldCyan, selectedRange.Min, ColorReset, ColorBoldCyan, selectedRange.Max, ColorReset)

	// Convert hex strings to big int
	minKey := new(big.Int)
	maxKey := new(big.Int)
	minKey.SetString(selectedRange.Min[2:], 16) // Remove 0x prefix
	maxKey.SetString(selectedRange.Max[2:], 16) // Remove 0x prefix

	searchForPrivateKey(minKey, maxKey, targetAddress)
}


