package main

import (
	"bytes"
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"math/big"
	"os"
	"runtime"
	"sync"
	"sync/atomic"
	"time"
)

// bytesEqual compares two byte slices for equality
func bytesEqual(a, b []byte) bool {
	return bytes.Equal(a, b)
}

// searchForPrivateKey searches for a private key that corresponds to the target hash160
// within the given range (minKey to maxKey) using multiple goroutines
func searchForPrivateKey(minKey, maxKey *big.Int, targetHash160 []byte) {
	// Determine the number of goroutines to use based on available CPU cores
	numCPU := runtime.NumCPU()
	numWorkers := numCPU * 1 // Use 2x the number of CPUs for best performance
	fmt.Printf("%sStarting key search with %d workers...%s\n", ColorBlue, numWorkers, ColorReset)
	
	// Determine the limit for iterations to prevent infinite loops
	diff := new(big.Int).Sub(maxKey, minKey)
	limit := new(big.Int).Set(diff)

	// Variables for synchronization and tracking
	var wg sync.WaitGroup
	foundMatch := false
	matchMutex := &sync.Mutex{}
	var foundKey []byte
	var foundHash160 []byte
	var totalIterations int64 = 0
	var lastKeyMutex sync.Mutex
	lastKeyChecked := new(big.Int)
	
	// Generate a random starting point within the range
	randomOffset, err := rand.Int(rand.Reader, diff)
	if err != nil {
		fmt.Printf("%sError generating random starting point: %v%s\n", ColorRed, err, ColorReset)
		return
	}
	
	// Calculate the new starting point by adding the random offset to minKey
	randomStart := new(big.Int).Add(minKey, randomOffset)
	fmt.Printf("%sStarting from random position within range...%s\n", ColorBlue, ColorReset)
	randomStartHex := hex.EncodeToString(randomStart.Bytes())
	fmt.Printf("%sRandom start point: %s%s%s\n", ColorCyan, ColorBoldCyan, randomStartHex, ColorReset)
	
	// Divide the keyspace into chunks for each worker
	chunkSize := new(big.Int).Div(limit, big.NewInt(int64(numWorkers)))
	if chunkSize.Cmp(big.NewInt(0)) <= 0 {
		chunkSize = big.NewInt(1)
	}
	
	// Create a channel to signal when a match is found
	matchFound := make(chan bool)
	
	// Setup for progress reporting
	startTime := time.Now()
	
	// Create a goroutine to report progress every 10 seconds
	go func() {
		for !foundMatch {
			time.Sleep(10 * time.Second)
			
			// If a match was found while we were sleeping, exit
			matchMutex.Lock()
			if foundMatch {
				matchMutex.Unlock()
				return
			}
			matchMutex.Unlock()
			
			// Calculate and report stats
			currentTime := time.Now()
			elapsedSeconds := currentTime.Sub(startTime).Seconds()
			itCount := atomic.LoadInt64(&totalIterations)
			keysPerSecond := float64(itCount) / elapsedSeconds
			
			// Get the last key checked
			lastKeyMutex.Lock()
			lastKeyHex := hex.EncodeToString(lastKeyChecked.Bytes())
			lastKeyMutex.Unlock()
			
			fmt.Printf("%sChecked %d keys (%.2f keys/sec) - Last key: %s%s\n", ColorCyan, itCount, keysPerSecond, lastKeyHex, ColorReset)
		}
	}()
	
	// Start worker goroutines
	for i := 0; i < numWorkers; i++ {
		wg.Add(1)
		go func(workerID int) {
			defer wg.Done()
			
			// Calculate this worker's range starting from the random point
			workerStart := new(big.Int).Set(randomStart)
			offset := new(big.Int).Mul(chunkSize, big.NewInt(int64(workerID)))
			workerStart.Add(workerStart, offset)
			
			workerEnd := new(big.Int).Set(workerStart)
			workerEnd.Add(workerEnd, chunkSize)
			
			// Make sure we don't exceed the overall max
			if workerEnd.Cmp(maxKey) > 0 || (workerID == numWorkers-1) {
				workerEnd.Set(maxKey)
			}
			
			// Handle wrap-around if we exceed maxKey
			if workerStart.Cmp(maxKey) > 0 {
				// Wrap around to minKey plus the remainder
				excess := new(big.Int).Sub(workerStart, maxKey)
				excess.Sub(excess, big.NewInt(1))
				workerStart.Set(minKey)
				workerStart.Add(workerStart, excess)
			}
			
			// Local variables for search
			currentKey := new(big.Int).Set(workerStart)
			oneBI := big.NewInt(1)
			workerIterations := int64(0)
			
			// Main loop for this worker
			for currentKey.Cmp(workerEnd) <= 0 {
				// Handle wrap-around if we reach maxKey
				if currentKey.Cmp(maxKey) > 0 {
					currentKey.Set(minKey)
				}
				// Check if a match was already found by another worker
				matchMutex.Lock()
				if foundMatch {
					matchMutex.Unlock()
					return
				}
				matchMutex.Unlock()
				
				// Convert current big int to private key
				privateKeyBytes := padPrivateKey(currentKey.Bytes(), 32)
				
				// Generate hash160 from private key
				hash160, err := privateKeyToHash160(privateKeyBytes)
				if err != nil {
					fmt.Printf("%sWorker %d: Error generating hash160: %v%s\n", ColorRed, workerID, err, ColorReset)
					return
				}
				
				// Check if it matches the target hash160
				if bytesEqual(hash160, targetHash160) {
					// We found a match!
					matchMutex.Lock()
					if !foundMatch { // Double check in case another worker just found it
						foundMatch = true
						foundKey = privateKeyBytes
						foundHash160 = hash160
						// Signal other goroutines
						close(matchFound)
					}
					matchMutex.Unlock()
					return
				}
				
				// Increment key and iterations
				currentKey.Add(currentKey, oneBI)
				workerIterations++
				
				// Periodically update the last key checked
				if workerIterations % 1000 == 0 {
					lastKeyMutex.Lock()
					lastKeyChecked.Set(currentKey)
					lastKeyMutex.Unlock()
				}
				
				// Update total iterations counter periodically
				if workerIterations % 1000 == 0 {
					atomic.AddInt64(&totalIterations, 1000)
				}
			}
			
			// Add any remaining iterations
			if workerIterations % 1000 != 0 {
				atomic.AddInt64(&totalIterations, workerIterations % 1000)
			}
		}(i)
	}
	
	// Wait for a match to be found or all workers to finish
	go func() {
		wg.Wait()
		// Only close if no match was found to avoid panic if already closed
		matchMutex.Lock()
		if !foundMatch {
			close(matchFound)
		}
		matchMutex.Unlock()
	}()
	
	// Wait for the signal that a match is found or all workers are done
	<-matchFound
	
	// Report results
	matchMutex.Lock()
	if foundMatch {
		privateKeyHex := hex.EncodeToString(foundKey)
		fmt.Printf("\n%sMATCH FOUND!%s\n", ColorBoldGreen, ColorReset)
		fmt.Printf("%sPrivate Key: %s%s%s\n", ColorGreen, ColorBoldGreen, privateKeyHex, ColorReset)
		hash160Hex := hex.EncodeToString(foundHash160)
		fmt.Printf("%sHash160: %s%s%s\n", ColorGreen, ColorBoldGreen, hash160Hex, ColorReset)
		
		// Write the private key to a file
		filename := "found_key_" + hash160Hex[:8] + ".txt"
		content := fmt.Sprintf("Private Key: %s\nHash160: %s\nFound at: %s", privateKeyHex, hash160Hex, time.Now().Format(time.RFC3339))
		err := os.WriteFile(filename, []byte(content), 0600)
		if err != nil {
			fmt.Printf("%sError writing key to file: %s%s\n", ColorRed, err, ColorReset)
		} else {
			fmt.Printf("%sPrivate key saved to file: %s%s%s\n", ColorGreen, ColorBoldGreen, filename, ColorReset)
		}
	} else {
		fmt.Printf("\n%sNo match found after checking approximately %d keys.%s\n", ColorYellow, atomic.LoadInt64(&totalIterations), ColorReset)
	}
	matchMutex.Unlock()
}
