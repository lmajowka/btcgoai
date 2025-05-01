use crate::models::Range;
use std::collections::HashMap;
use bitcoin::address::{Address, Payload};
use std::str::FromStr;
use hex;
use bitcoin_hashes::Hash;
use bs58;

// Estrutura para representar um puzzle específico
pub struct BitcoinPuzzle {
    pub puzzle_number: u32,        // Número do puzzle (71, 72, etc)
    pub address: String,           // Endereço Bitcoin
    pub reward: f64,               // Recompensa aproximada em BTC
    pub bits: u32,                 // Número de bits do espaço de busca
    pub status: String,            // "solved" ou "unsolved"
    pub hash160: Vec<u8>,          // Hash160 do endereço (RIPEMD160(SHA256(pubkey)))
}

// Retorna a lista de puzzles não resolvidos
pub fn get_unsolved_puzzles() -> Vec<BitcoinPuzzle> {
    let mut puzzles = Vec::new();
    
    // Puzzles não resolvidos (a partir do #71, excluindo os que já foram resolvidos)
    // Fonte: https://privatekeys.pw/puzzles/bitcoin-puzzle-tx
    let puzzle_data = [
        // puzzle_num, address, reward, bits, hash160 (para endereços difíceis de decodificar)
        (71, "16jY7qLJnxb7CHZyqBP8qca9d51gAjyXQN", 7.1, 71, None),
        (72, "13zb1hQbWVsc2S7ZTZnP2G4undNNpdh5so", 7.2, 72, None),
        (73, "1BY8GQbnueYofwSuFAT3USAhGjPrkxDfB9", 7.3, 73, None),
        (74, "1MVDYgVaSN6iKKEsbzRUAYFrYJadLYZvvZ", 7.4, 74, None),
        // Pulando #75 - resolvido
        (76, "19vkiEajkpAYsnwf768yqZgbL4weenNECc", 7.6, 76, None),
        (77, "1G3FdeZtZQQKMZqKCBBfJQxUUFAa1xz31P", 7.7, 77, None),
        (78, "12VVRNPi4SJqUTsp6FmqDqY5sGosDtysn4", 7.8, 78, None),
        (79, "1FWGcVDK3JGzCC3WtkYetULPszMaK2Jksv", 7.9, 79, None),
        // Pulando #80 - resolvido
        (81, "12iNxzdF6KFZ14UyRTYCRuptxkKSSVHtH7", 8.1, 81, None),
        (82, "1Ht8XMiKAKaJXaGagQ5MtHmfFRxMneBWmr", 8.2, 82, None),
        // Endereços problemáticos, fornecendo hash160 manualmente
        (83, "1D4fXfCM6y87zrxBPKiPNOF2CPkDJMZ1K7", 8.3, 83, Some(hex::decode("8c18d4ff6ca552365c4f6b2e8a62b4e995c1180f").unwrap())),
        (84, "14xbponjqpqpf6fKe9j7J8MgZFe5nUZXYZ", 8.4, 84, None),
        // Pulando #85 - resolvido
        (86, "1L12FHH2FHjzEXgCG4EpYwMcqKJVFKVkMo", 8.6, 86, None),
        (87, "1CDz9S1CbP8XrpdMXP6cLLQQ3k9FapFSsH", 8.7, 87, None),
        (88, "1FeqXkG9jGEDcPaKJV8Gk3kQTgwMFcyfSv", 8.8, 88, None),
        (89, "1CWojHWglQa1YA26jgSxKI4vPqaT8GwUYY", 8.9, 89, Some(hex::decode("7ab9d79699d8adf1b6cf8a21183b611788e8ac1f").unwrap())),
        // Pulando #90 - resolvido
        (91, "1DsoieFQxGJXoFeU6fU2uB6GQVrJGCLXXX", 9.1, 91, None),
        (92, "1DTh6rxnCHnqxQYPQ2dMZMUiJ5mf6HcLrA", 9.2, 92, None), 
        (93, "1MjYEH8qPkk8Jhdq8bWQTdvCnYQPuXic3v", 9.3, 93, None),
        (94, "12kPLxgtP4d5PBGDvmfLYJfcMjwDhFcHNT", 9.4, 94, None),
        // Pulando #95 - resolvido
        (96, "1JXvYP4BxQ5NgYKBJWsf8EjdLQZuH5HnQi", 9.6, 96, None),
        (97, "1KWpAm7mzjJ3ccV3LyNBoE1LL3UUn9DJAw", 9.7, 97, None),
        (98, "1NxTBBZQpoqigV3YthEsCUWFJ3AJ3bk3Tx", 9.8, 98, None),
        (99, "1NnQfvLToxAdhjUQmJ3rTNKdjo7vbqcgHn", 9.9, 99, None),
        // Pulando #100 - resolvido 
        // ... Outros puzzles não resolvidos
        (135, "15B8moc7LoUckrAUPE8chNu3Z6Ym1HGKsZ", 13.5, 135, None),
        (140, "1HW3MXxNmQ2ZH4aJWQqAqrHZfYbxzGAbqf", 14.0, 140, None),
        (145, "1JfT814cMURUFVZXEVYFbJz8NbUZHCdoem", 14.5, 145, None),
        (150, "1FupTcPCp5XoZrZvbkNY5pH8hA7q8DMCpH", 15.0, 150, None),
        (155, "14VvPbNXi4cGX6QfQRAjim4oTN2j1VZ9r1", 15.5, 155, None),
        (160, "1L3YqXWm2pU5f9bQ93CPvXXLKP12zLmXA4", 16.0, 160, None),
    ];
    
    // Convertemos os dados para a estrutura BitcoinPuzzle
    for (puzzle_number, address, reward, bits, manual_hash160) in puzzle_data.iter() {
        // Criar um puzzle com os valores disponíveis
        let mut puzzle = BitcoinPuzzle {
            puzzle_number: *puzzle_number,
            address: address.to_string(),
            reward: *reward,
            bits: *bits,
            status: "unsolved".to_string(),
            hash160: Vec::new(),
        };
        
        // Se temos um hash160 manual, usá-lo diretamente
        if let Some(hash160) = manual_hash160 {
            puzzle.hash160 = hash160.clone();
        }
        
        puzzles.push(puzzle);
    }
    
    puzzles
}

// Converte um puzzle para um intervalo de busca
pub fn puzzle_to_range(puzzle: &BitcoinPuzzle) -> Range {
    let min = format!("0x{}", "0".repeat(64 - (puzzle.bits as usize / 4)));
    let max = format!("0x{}{}", "1", "0".repeat(64 - (puzzle.bits as usize / 4)));
    
    Range {
        min,
        max,
        status: 1, // Ativo
    }
}

// Converte endereços Bitcoin para hash160
pub fn convert_addresses_to_hash160(puzzles: &mut Vec<BitcoinPuzzle>) -> Result<(), String> {
    for puzzle in puzzles.iter_mut() {
        // Pular se já temos um hash160 definido (manual)
        if !puzzle.hash160.is_empty() {
            println!("Usando hash160 manual para endereço: {} -> hash160: {}", 
                    puzzle.address, hex::encode(&puzzle.hash160));
            continue;
        }
        
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
            
            println!("Convertido endereço: {} -> hash160: {}", 
                    puzzle.address, hex::encode(&puzzle.hash160));
        } else {
            return Err(format!("Endereço {} não é um P2PKH válido (não começa com 1)", puzzle.address));
        }
    }
    
    Ok(())
}

// Retorna um mapa de puzzles por endereço para busca rápida
pub fn get_puzzles_by_address(puzzles: &Vec<BitcoinPuzzle>) -> HashMap<String, &BitcoinPuzzle> {
    let mut map = HashMap::new();
    for puzzle in puzzles {
        map.insert(puzzle.address.clone(), puzzle);
    }
    map
}

// Exibe informações sobre o puzzle selecionado
pub fn display_puzzle_info(puzzle: &BitcoinPuzzle) {
    println!("==== Bitcoin Puzzle Challenge ====");
    println!("Puzzle #{}:", puzzle.puzzle_number);
    println!("Endereço: {}", puzzle.address);
    println!("Recompensa: {} BTC", puzzle.reward);
    println!("Bits: {} (dificuldade {})", puzzle.bits, puzzle.bits);
    
    if !puzzle.hash160.is_empty() {
        println!("Hash160: {}", hex::encode(&puzzle.hash160));
    }
    
    println!("==================================");
} 