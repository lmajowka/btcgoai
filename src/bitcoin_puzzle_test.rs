use crate::models::Range;

// Estrutura para representar um puzzle de teste (dificuldade menor)
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

// Exibe informações sobre o puzzle de teste selecionado
pub fn display_test_puzzle_info(puzzle: &TestPuzzle) {
    println!("==== Bitcoin Puzzle Test Challenge ====");
    println!("Puzzle #{}:", puzzle.puzzle_number);
    println!("Endereço: {}", puzzle.address);
    println!("Bits: {} (dificuldade baixa para teste)", puzzle.bits);
    println!("Chave privada conhecida: {}", puzzle.private_key);
    
    if !puzzle.hash160.is_empty() {
        println!("Hash160: {}", hex::encode(&puzzle.hash160));
    }
    
    println!("======================================");
}

/// Find training puzzles within a specific bit range
pub fn find_training_puzzles(min_bits: u8, max_bits: u8) -> Vec<TestPuzzle> {
    let mut test_puzzles = get_test_puzzles();
    // Convert addresses to hash160 for all puzzles
    let _ = convert_addresses_to_hash160(&mut test_puzzles);
    
    // Filter by bit range
    test_puzzles.into_iter()
        .filter(|puzzle| puzzle.bits >= min_bits as u32 && puzzle.bits <= max_bits as u32)
        .collect()
} 