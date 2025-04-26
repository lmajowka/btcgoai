package main

import (
	"bufio"
	"encoding/hex"
	"fmt"
	"math/big"
	"os"
	"strconv"
	"strings"
)

func main() {
	// Load wallet hash160s
	walletHash160s, err := loadWalletHash160s()
	if err != nil {
		fmt.Printf("%sError loading wallet hash160s: %v%s\n", ColorRed, err, ColorReset)
		return
	}
	fmt.Printf("%sLoaded %d wallet hash160 values%s\n", ColorGreen, len(walletHash160s), ColorReset)

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

	// Get the wallet hash160 for the selected number
	walletIndex := walletNum - 1
	if walletIndex >= len(walletHash160s) {
		fmt.Printf("%sWallet index out of range.%s\n", ColorRed, ColorReset)
		return
	}
	targetHash160 := walletHash160s[walletIndex]
	
	// Get the range for the selected wallet
	if walletIndex >= len(ranges) {
		fmt.Printf("%sRange index out of range.%s\n", ColorRed, ColorReset)
		return
	}
	selectedRange := ranges[walletIndex]

	targetHash160Hex := hex.EncodeToString(targetHash160)
	fmt.Printf("%sSelected Wallet Hash160: %s%s%s\n", ColorYellow, ColorBoldYellow, targetHash160Hex, ColorReset)
	fmt.Printf("%sRange: min=%s%s%s, max=%s%s%s\n", ColorYellow, ColorBoldCyan, selectedRange.Min, ColorReset, ColorBoldCyan, selectedRange.Max, ColorReset)

	// Convert hex strings to big int
	minKey := new(big.Int)
	maxKey := new(big.Int)
	minKey.SetString(selectedRange.Min[2:], 16) // Remove 0x prefix
	maxKey.SetString(selectedRange.Max[2:], 16) // Remove 0x prefix

	searchForPrivateKey(minKey, maxKey, targetHash160)
}


