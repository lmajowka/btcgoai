package main

import (
	"encoding/hex"
	"fmt"
	"math/big"
	"runtime"
	"sync"
	"sync/atomic"
	"time"
)

// searchForPrivateKey searches for a private key that corresponds to the target address
// within the given range (minKey to maxKey) using multiple goroutines
func searchForPrivateKey(minKey, maxKey *big.Int, targetAddress string) {
	// Determine the number of goroutines to use based on available CPU cores
	numCPU := runtime.NumCPU()
	numWorkers := numCPU * 1 // Use 2x the number of CPUs for best performance
	fmt.Printf("%sStarting key search with %d workers...%s\n", ColorBlue, numWorkers, ColorReset)
	
	// Determine the limit for iterations to prevent infinite loops
	diff := new(big.Int).Sub(maxKey, minKey)
	limit := new(big.Int).Set(diff)
	// Limit to a reasonable number if the range is too large
	maxIterations := big.NewInt(1000000) // Limit to 1 million iterations
	if diff.Cmp(maxIterations) > 0 {
		limit = maxIterations
		fmt.Printf("%sRange is very large, limiting to %s iterations%s\n", ColorYellow, maxIterations.String(), ColorReset)
	}

	// Variables for synchronization and tracking
	var wg sync.WaitGroup
	foundMatch := false
	matchMutex := &sync.Mutex{}
	var foundKey []byte
	var foundAddress string
	var totalIterations int64 = 0
	
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
			fmt.Printf("%sChecked %d keys (%.2f keys/sec)%s\n", ColorCyan, itCount, keysPerSecond, ColorReset)
		}
	}()
	
	// Start worker goroutines
	for i := 0; i < numWorkers; i++ {
		wg.Add(1)
		go func(workerID int) {
			defer wg.Done()
			
			// Calculate this worker's range
			workerStart := new(big.Int).Set(minKey)
			offset := new(big.Int).Mul(chunkSize, big.NewInt(int64(workerID)))
			workerStart.Add(workerStart, offset)
			
			workerEnd := new(big.Int).Set(workerStart)
			workerEnd.Add(workerEnd, chunkSize)
			
			// Make sure we don't exceed the overall max
			if workerEnd.Cmp(maxKey) > 0 || (workerID == numWorkers-1) {
				workerEnd.Set(maxKey)
			}
			
			// Local variables for search
			currentKey := new(big.Int).Set(workerStart)
			oneBI := big.NewInt(1)
			workerIterations := int64(0)
			
			// Main loop for this worker
			for currentKey.Cmp(workerEnd) <= 0 {
				// Check if a match was already found by another worker
				matchMutex.Lock()
				if foundMatch {
					matchMutex.Unlock()
					return
				}
				matchMutex.Unlock()
				
				// Convert current big int to private key
				privateKeyBytes := padPrivateKey(currentKey.Bytes(), 32)
				
				// Generate address from private key
				address, err := privateKeyToAddress(privateKeyBytes)
				if err != nil {
					fmt.Printf("%sWorker %d: Error generating address: %v%s\n", ColorRed, workerID, err, ColorReset)
					return
				}
				
				// Check if it matches the target address
				if address == targetAddress {
					// We found a match!
					matchMutex.Lock()
					if !foundMatch { // Double check in case another worker just found it
						foundMatch = true
						foundKey = privateKeyBytes
						foundAddress = address
						// Signal other goroutines
						close(matchFound)
					}
					matchMutex.Unlock()
					return
				}
				
				// Increment key and iterations
				currentKey.Add(currentKey, oneBI)
				workerIterations++
				
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
		fmt.Printf("%sAddress: %s%s%s\n", ColorGreen, ColorBoldGreen, foundAddress, ColorReset)
	} else {
		fmt.Printf("\n%sNo match found after checking approximately %d keys.%s\n", ColorYellow, atomic.LoadInt64(&totalIterations), ColorReset)
	}
	matchMutex.Unlock()
}
