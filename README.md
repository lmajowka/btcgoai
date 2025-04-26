# Bitcoin Private Key Finder

This Go program searches for Bitcoin private keys within a specified range that correspond to a targeted Bitcoin address.

## Description

The program works as follows:
1. It prompts the user to enter a wallet number (1-160)
2. It loads the corresponding range from `ranges.json` for that wallet number
3. It then iterates through private keys in that range, converting each to a Bitcoin address
4. It compares each generated address with the target wallet address from `wallets.json`
5. If a match is found, it displays the private key

## Prerequisites

- Go 1.18 or higher

## Usage

1. Make sure the `wallets.json` and `ranges.json` files are in the same directory as the executable
2. Run the program:
   ```
   ./bitcoin_finder.exe
   ```
3. Enter a wallet number between 1 and 160 when prompted
4. The program will start searching for matching private keys

## Compilation

1. Ensure you have Go 1.18 or higher installed on your system
2. Clone this repository or download the source code
3. Navigate to the project directory
4. Build the executable:
   ```
   go build -o bitcoin_finder.exe
   ```
5. The compiled executable will be created in the same directory

## Notes

- Large ranges may take significant time to process
- The program includes a built-in limit to prevent infinite searches
- Only active ranges (status=1) will be processed
