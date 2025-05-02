use crate::models::Range;

// Estrutura para representar um puzzle de teste (dificuldade menor)
#[derive(Clone)]
pub struct TestPuzzle {
    pub puzzle_number: u32,        // Número do puzzle
    pub address: String,           // Endereço Bitcoin
    pub bits: u32,                 // Número de bits do espaço de busca
    #[allow(dead_code)]
    pub status: String,            // "solved" (todos já foram resolvidos)
    pub hash160: Vec<u8>,          // Hash160 do endereço (preenchido pela função convert_addresses_to_hash160)
    pub private_key: String,       // Chave privada conhecida (em formato hex)
}

// Retorna uma lista de puzzles de teste com dificuldade muito menor
pub fn get_test_puzzles() -> Vec<TestPuzzle> {
    let mut puzzles = Vec::new();
    
    // Puzzles de teste com bits muito baixos
    // Dificuldade crescente: 5 bits, 15 bits, 22 bits
    let puzzle_data = [
        // puzzle_number, address, bits, private_key (hex)
        (5, "1GqhCxQkA7vNvvNQ1eDZBrUX1YiQ9SYmzB", 5, "0000000000000000000000000000000000000000000000000000000000000010"),
        (15, "19QCEkx6v2sxnuWNbRAUV5K9c7ZwVhEfYm", 15, "0000000000000000000000000000000000000000000000000000000000006000"),
        (22, "1KJXcsXnqxJH9Kv73SjcZnsgRJm9En1RJp", 22, "0000000000000000000000000000000000000000000000000000000000200000"),
    ];
    
    // Convertemos os dados para a estrutura TestPuzzle
    for (puzzle_number, address, bits, private_key) in puzzle_data.iter() {
        let puzzle = TestPuzzle {
            puzzle_number: *puzzle_number,
            address: address.to_string(),
            bits: *bits,
            status: "solved".to_string(),
            hash160: Vec::new(), // Será preenchido posteriormente
            private_key: private_key.to_string(),
        };
        
        puzzles.push(puzzle);
    }
    
    puzzles
}

// Converte endereços Bitcoin para hash160
pub fn convert_addresses_to_hash160(puzzles: &mut Vec<TestPuzzle>) -> Result<(), String> {
    for puzzle in puzzles.iter_mut() {
        // Decodificar endereço Bitcoin usando Base58
        if puzzle.address.starts_with("1") {
            // Decodificar o endereço Base58
            let decoded = match bs58::decode(&puzzle.address).into_vec() {
                Ok(data) => data,
                Err(e) => return Err(format!("Erro ao decodificar endereço {}: {}", puzzle.address, e)),
            };
            
            // Verificar se o tamanho está correto (versão + hash160 + checksum)
            if decoded.len() != 25 {
                return Err(format!("Tamanho inválido após decodificação para {}: esperado 25, obtido {}", 
                                   puzzle.address, decoded.len()));
            }
            
            // Verificar se é um endereço P2PKH (versão 0x00)
            if decoded[0] != 0x00 {
                return Err(format!("Endereço {} não é um P2PKH (versão errada: 0x{:02x})", 
                                   puzzle.address, decoded[0]));
            }
            
            // Extrair o hash160 (bytes 1-21, excluindo versão e checksum)
            puzzle.hash160 = decoded[1..21].to_vec();
            
            println!("Convertido endereço de teste: {} -> hash160: {}", 
                    puzzle.address, hex::encode(&puzzle.hash160));
        } else {
            return Err(format!("Endereço {} não é um P2PKH válido (não começa com 1)", puzzle.address));
        }
    }
    
    Ok(())
}

// Converte um puzzle para um intervalo de busca
pub fn puzzle_to_range(puzzle: &TestPuzzle) -> Range {
    let min = format!("0x{}", "0".repeat(64 - (puzzle.bits as usize / 4)));
    let max = format!("0x{}{}", "1", "0".repeat(64 - (puzzle.bits as usize / 4)));
    
    Range {
        min,
        max,
        status: 1, // Ativo
    }
}

/// Display information about a test puzzle
pub fn display_test_puzzle_info(puzzle: &TestPuzzle) {
    println!("\n{}Puzzle selecionado:{}", crate::colors::BOLD_GREEN, crate::colors::RESET);
    println!("{}Dificuldade: {} bits{}", crate::colors::CYAN, puzzle.bits, crate::colors::RESET);
    println!("{}Endereço Bitcoin: {}{}", crate::colors::CYAN, puzzle.address, crate::colors::RESET);
    println!("{}Hash160: {}{}", crate::colors::CYAN, hex::encode(&puzzle.hash160), crate::colors::RESET);
    
    // No modo de treinamento, também mostramos a chave privada para fins educacionais
    println!("{}Chave privada (solução): {}{}", crate::colors::MAGENTA, puzzle.private_key, crate::colors::RESET);
}

/// Find training puzzles with difficulties in the specified range
pub fn find_training_puzzles(min_bits: u32, max_bits: u32) -> Vec<TestPuzzle> {
    let mut puzzles = Vec::new();
    
    // Load built-in puzzles
    let built_in = load_puzzles();
    
    // Filter puzzles by difficulty range
    for p in built_in {
        if p.bits >= min_bits && p.bits <= max_bits {
            puzzles.push(p);
        }
    }
    
    puzzles
}

/// Load puzzles from built-in data
fn load_puzzles() -> Vec<TestPuzzle> {
    let mut puzzles = Vec::new();
    
    // Add some test puzzles with known solutions (very small bit ranges for fast testing)
    // Format: bits, address, private_key
    // All private keys are in hex format
    
    // 5-bit puzzles
    puzzles.push(TestPuzzle {
        puzzle_number: 0,
        bits: 5,
        address: "17BetZTwF7Tmb7R6RscgQQZFeYYT6YYvYb".to_string(),
        private_key: "1f".to_string(),
        hash160: [0u8; 20].to_vec(),
        status: "solved".to_string(),
    });
    
    // 8-bit puzzles
    puzzles.push(TestPuzzle {
        puzzle_number: 1,
        bits: 8,
        address: "13sU6LUW2BkpaTrEWu4f7c21b3YGfYSn91".to_string(),
        private_key: "a3".to_string(),
        hash160: [0u8; 20].to_vec(),
        status: "solved".to_string(),
    });
    
    // 12-bit puzzles
    puzzles.push(TestPuzzle {
        puzzle_number: 2,
        bits: 12,
        address: "1E9nMRheuBGQtbXfiav6Q9jpYTpYKzQgHE".to_string(),
        private_key: "ab1".to_string(),
        hash160: [0u8; 20].to_vec(),
        status: "solved".to_string(),
    });
    
    // 16-bit puzzles
    puzzles.push(TestPuzzle {
        puzzle_number: 3,
        bits: 16,
        address: "1LdKKwqVxkJfZYS3nkThmNsYVNAXw5Gq9i".to_string(),
        private_key: "a11e".to_string(),
        hash160: [0u8; 20].to_vec(),
        status: "solved".to_string(),
    });
    
    // 20-bit puzzles
    puzzles.push(TestPuzzle {
        puzzle_number: 4,
        bits: 20,
        address: "1FgN1dXRqxQG7GMdhdFvYH1fJR7a9uDq9w".to_string(),
        private_key: "a11f3".to_string(),
        hash160: [0u8; 20].to_vec(),
        status: "solved".to_string(),
    });
    
    // 22-bit puzzles
    puzzles.push(TestPuzzle {
        puzzle_number: 5,
        bits: 22,
        address: "1Q2TWHE3GMdB6BZKafqwxXtWAWgFt5Jvm3".to_string(),
        private_key: "3072af".to_string(),
        hash160: [0u8; 20].to_vec(),
        status: "solved".to_string(),
    });
    
    puzzles
} 