# Object System Module

## Overview

The Object System Module implements a Sui-style object ID system for the VibeCoin blockchain, extending the account-based storage model into a more object-centric architecture. This module provides the core structures and functionality for creating, storing, and managing objects.

## Components

### Object Model

The object model defines the core structures for representing objects:

- **ObjectId**: A 32-byte hash that uniquely identifies an object
- **Ownership**: Defines who owns an object (Address, Shared, or Immutable)
- **Object**: The main structure representing a Sui-style object

```rust
pub enum Ownership {
    Address([u8; 32]),
    Shared,
    Immutable,
}

pub struct Object {
    pub id: ObjectId,
    pub owner: Ownership,
    pub version: u64,
    pub type_tag: String,
    pub contents: Vec<u8>,
    pub created_at: u64,
    pub updated_at: u64,
    pub last_updated_block: u64,
    pub metadata: HashMap<String, String>,
}
```

### Object Store

The object store manages the persistence of objects:

- **ObjectStore**: Provides methods for storing, retrieving, and querying objects
- **Operations**: put_object, get_object, delete_object, update_object, create_object
- **Queries**: get_objects_by_owner, get_objects_by_type, get_all_objects

### Object Transactions

The object transaction system extends the existing transaction model to support object operations:

- **ObjectTransactionKind**: Defines the types of object operations (Create, Mutate, Transfer, Delete)
- **ObjectTransactionRecord**: Represents a transaction for object operations

```rust
pub enum ObjectTransactionKind {
    CreateObject { type_tag: String, owner: Ownership, contents: Vec<u8>, metadata: Option<HashMap<String, String>> },
    MutateObject { id: ObjectId, new_contents: Vec<u8> },
    TransferObject { id: ObjectId, new_owner: Ownership },
    DeleteObject { id: ObjectId },
}
```

### Object Processor

The object processor handles the validation and execution of object transactions:

- **ObjectProcessor**: Validates and executes object transactions
- **Validation**: Checks signatures, nonces, balances, and ownership
- **Execution**: Applies the transaction to the object store

## Usage Examples

### Creating an Object

```rust
// Create an object store
let object_store = ObjectStore::new(kv_store);

// Create a new object
let object = object_store.create_object(
    Ownership::Address(sender),
    "Coin".to_string(),
    vec![1, 2, 3, 4],
    None,
);
```

### Transferring an Object

```rust
// Create a transaction to transfer an object
let kind = ObjectTransactionKind::TransferObject {
    id: object_id,
    new_owner: Ownership::Address(recipient),
};

let tx = ObjectTransactionRecord::create_signed(
    &keypair,
    kind,
    gas_price,
    gas_limit,
    nonce,
);

// Process the transaction
object_processor.process_transaction(&tx);
```

### Querying Objects

```rust
// Get objects owned by an address
let objects = object_store.get_objects_by_owner(&address);

// Get objects by type
let coins = object_store.get_objects_by_type("Coin");
```

## Ownership Rules

The object system enforces the following ownership rules:

- **Address-owned objects**: Can only be modified or transferred by the owner
- **Shared objects**: Can be modified by anyone, but cannot be transferred
- **Immutable objects**: Cannot be modified or transferred by anyone

## Integration with Existing Systems

The object system integrates with the existing VibeCoin systems:

- **State Store**: Used for account balances and nonces
- **Transaction Processing**: Object transactions are processed alongside regular transactions
- **Block Processing**: Objects are included in blocks and their state is updated accordingly

## Performance Considerations

- **Indexing**: Objects are indexed by ID, owner, and type for efficient querying
- **Caching**: Frequently accessed objects can be cached for better performance
- **Batch Operations**: Multiple object operations can be batched for atomic execution
