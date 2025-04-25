package main

import (
	"encoding/hex"
	"fmt"
	"math/big"
	"time"
)

// searchForPrivateKey searches for a private key that corresponds to the target address
// within the given range (minKey to maxKey)
func searchForPrivateKey(minKey, maxKey *big.Int, targetAddress string) {
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
