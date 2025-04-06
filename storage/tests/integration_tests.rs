use vibecoin::storage::{
    RocksDBStore, KVStore,
    Block, BlockStore,
    TransactionRecord, TxStore,
    AccountState, StateStore,
    PoHEntry, PoHStore
};
use tempfile::tempdir;

#[test]
fn test_storage_integration() {
    // Create a temporary directory for the test
    let temp_dir = tempdir().unwrap();
    let kv_store = RocksDBStore::new(temp_dir.path());
    
    // Initialize all stores
    let block_store = BlockStore::new(&kv_store);
    let tx_store = TxStore::new(&kv_store);
    let state_store = StateStore::new(&kv_store);
    let poh_store = PoHStore::new(&kv_store);
    
    // Create test data
    
    // 1. Create account states
    let sender_addr = [1; 32];
    let recipient_addr = [2; 32];
    
    state_store.create_account(&sender_addr, 1000);
    state_store.create_account(&recipient_addr, 500);
    
    // 2. Create a transaction
    let tx = TransactionRecord {
        tx_id: [3; 32],
        sender: sender_addr,
        recipient: recipient_addr,
        value: 100,
        gas_used: 10,
        block_height: 1,
    };
    
    tx_store.put_transaction(&tx);
    
    // 3. Update account states based on transaction
    let sender_state = state_store.get_account_state(&sender_addr).unwrap();
    state_store.update_balance(&sender_addr, sender_state.balance - tx.value - tx.gas_used).unwrap();
    state_store.increment_nonce(&sender_addr).unwrap();
    
    let recipient_state = state_store.get_account_state(&recipient_addr).unwrap();
    state_store.update_balance(&recipient_addr, recipient_state.balance + tx.value).unwrap();
    
    // 4. Create PoH entries
    let poh_entry = PoHEntry {
        hash: [4; 32],
        sequence: 1,
        timestamp: 12345,
    };
    
    poh_store.append_entry(&poh_entry);
    
    // 5. Create a block
    let block = Block {
        height: 1,
        hash: [5; 32],
        prev_hash: [0; 32],
        timestamp: 12345,
        transactions: vec![tx.tx_id],
        state_root: [6; 32],
    };
    
    block_store.put_block(&block);
    
    // Verify everything was stored correctly
    
    // Check block
    let retrieved_block = block_store.get_block_by_height(1).unwrap();
    assert_eq!(retrieved_block, block);
    
    // Check transaction
    let retrieved_tx = tx_store.get_transaction(&tx.tx_id).unwrap();
    assert_eq!(retrieved_tx, tx);
    
    // Check account states
    let sender_state = state_store.get_account_state(&sender_addr).unwrap();
    assert_eq!(sender_state.balance, 1000 - tx.value - tx.gas_used);
    assert_eq!(sender_state.nonce, 1);
    
    let recipient_state = state_store.get_account_state(&recipient_addr).unwrap();
    assert_eq!(recipient_state.balance, 500 + tx.value);
    
    // Check PoH entry
    let retrieved_poh = poh_store.get_entry(1).unwrap();
    assert_eq!(retrieved_poh, poh_entry);
    
    // Test relationships
    
    // Block contains transaction
    let block_txs = tx_store.get_transactions_by_block(block.height);
    assert_eq!(block_txs.len(), 1);
    assert_eq!(block_txs[0].tx_id, tx.tx_id);
    
    // Transaction is associated with sender
    let sender_txs = tx_store.get_transactions_by_sender(&sender_addr);
    assert_eq!(sender_txs.len(), 1);
    assert_eq!(sender_txs[0].tx_id, tx.tx_id);
}
