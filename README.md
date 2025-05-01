# Bitcoin Private Key Finder (Rust)

Esta é uma implementação em Rust de alta performance para busca de chaves privadas Bitcoin dentro de intervalos específicos, com suporte especial para os desafios Bitcoin Puzzle TX.

## Características

1. **Modelo de concorrência eficiente** usando threads leves do Rust e operações atômicas
2. **Uso eficiente de memória** com melhor gerenciamento e operações zero-copy
3. **Sincronização eficiente de threads** usando `Arc` e `Mutex` para estado compartilhado
4. **Operações criptográficas otimizadas** via bibliotecas Bitcoin para Rust
5. **Otimizações de compilador** habilitadas com LTO (Link Time Optimization) e otimizações agressivas

## Requisitos

- Rust 1.50 ou superior

## Compilação

```bash
# Build de desenvolvimento
cargo build

# Build de produção (muito mais rápido)
cargo build --release
```

## Execução

```bash
# Executar o build de produção
cargo run --release
```

## Modos de Operação

### Modo de Teste

O programa oferece um modo de teste com puzzles de baixa dificuldade (5, 15 e 22 bits) que podem ser resolvidos rapidamente para verificar o funcionamento correto do algoritmo de busca.

Para usar o modo de teste:
1. Execute o programa
2. Responda "S" quando perguntar se deseja executar em modo de teste
3. Selecione um dos puzzles de teste disponíveis
4. Confirme para iniciar a busca

Os puzzles de teste têm chaves privadas conhecidas e são configurados para serem resolvidos em segundos ou minutos, dependendo da dificuldade escolhida e poder de processamento.

### Modo Normal

Este é o modo padrão que permite buscar chaves privadas em puzzles reais do [Bitcoin Puzzle TX Challenge](https://privatekeys.pw/puzzles/bitcoin-puzzle-tx).

## Desenvolvimento

### Arquivos Ignorados

Este projeto utiliza `.gitignore` para excluir do controle de versão:

- Diretórios `/target/` com binários compilados
- Arquivos de debug (*.pdb)
- Executáveis (*.exe, *.dll, *.so, *.dylib)
- Arquivos temporários de sistema (.DS_Store, Thumbs.db)
- Arquivos de configuração de IDE (.idea/, .vscode/)
- Arquivos de log e variáveis de ambiente

Isso mantém o repositório limpo, contendo apenas código fonte e documentação essencial.

## Funcionalidades

- Busca de chaves privadas para puzzles Bitcoin não resolvidos
- Interface de usuário interativa para seleção de puzzles
- Estatísticas de busca em tempo real
- Suporte multi-thread para máxima performance
- Salvamento automático de resultados encontrados
- Modo de teste com puzzles de baixa dificuldade para verificação do funcionamento

## License

Este software é fornecido como está, sem garantias.
