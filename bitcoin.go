package main

import (
	"github.com/btcsuite/btcd/btcec/v2"
	"github.com/btcsuite/btcd/btcutil"
	"github.com/btcsuite/btcd/chaincfg"
)

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

// privateKeyToHash160 converts a private key to a rim160 address
func privateKeyToHash160(privateKeyBytes []byte) ([]byte, error) {
	// Convert private key bytes to btcec private key
	privateKey, _ := btcec.PrivKeyFromBytes(privateKeyBytes)

	// Get public key from private key
	publicKey := privateKey.PubKey()

	// Convert public key to address
	pubKeyHash := btcutil.Hash160(publicKey.SerializeCompressed())
	return pubKeyHash, nil
}
