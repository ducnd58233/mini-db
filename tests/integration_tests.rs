use mini_db::{KVStore, Record, TableDef, Value, ValueType};
use tempfile::NamedTempFile;

#[test]
fn test_kvstore_basic_crud() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut store = KVStore::open(temp_file.path()).unwrap();

    // Create
    store.set(b"name", b"Alice").unwrap();
    store.set(b"age", &30i64.to_be_bytes()).unwrap();

    // Read
    let name = store.get(b"name").unwrap();
    assert_eq!(name, Some(b"Alice".to_vec()));

    let age = store.get(b"age").unwrap();
    assert_eq!(age, Some(30i64.to_be_bytes().to_vec()));

    // Update
    store.set(b"age", &31i64.to_be_bytes()).unwrap();
    let age = store.get(b"age").unwrap();
    assert_eq!(age, Some(31i64.to_be_bytes().to_vec()));

    // Delete
    let deleted = store.del(b"age").unwrap();
    assert!(deleted);

    let age = store.get(b"age").unwrap();
    assert_eq!(age, None);
}

#[test]
fn test_kvstore_range_query() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut store = KVStore::open(temp_file.path()).unwrap();

    // Insert sequential keys
    for i in 1..=20 {
        let key = format!("key{:03}", i);
        let val = format!("value{}", i);
        store.set(key.as_bytes(), val.as_bytes()).unwrap();
    }

    // Range query from key005 to key010
    let results = store.get_range(b"key005", b"key010").unwrap();
    assert_eq!(results.len(), 6); // key005 through key010

    // Verify content
    for (i, result) in results.iter().enumerate() {
        let expected = format!("value{}", i + 5);
        assert_eq!(result, expected.as_bytes());
    }
}

#[test]
fn test_kvstore_iterator() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut store = KVStore::open(temp_file.path()).unwrap();

    // Insert data
    for i in 1..=10 {
        let key = format!("k{:02}", i);
        let val = format!("v{}", i);
        store.set(key.as_bytes(), val.as_bytes()).unwrap();
    }

    // Test iteration from beginning
    let mut iter = store.iter_from(b"k00").unwrap();
    let mut count = 0;

    while let Some((key, val)) = iter.current() {
        count += 1;
        println!(
            "Key: {:?}, Value: {:?}",
            String::from_utf8_lossy(&key),
            String::from_utf8_lossy(&val)
        );

        if !iter.next().unwrap() {
            break;
        }
    }

    assert_eq!(count, 10);

    // Test iteration from middle
    let mut iter = store.iter_from(b"k05").unwrap();
    let mut count = 0;

    while let Some(_) = iter.current() {
        count += 1;
        if !iter.next().unwrap() {
            break;
        }
    }

    assert_eq!(count, 6); // k05 through k10
}

#[test]
fn test_kvstore_persistence() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_path_buf();

    // Write data
    {
        let mut store = KVStore::open(&path).unwrap();
        for i in 1..=50 {
            let key = format!("key{}", i);
            let val = format!("value{}", i);
            store.set(key.as_bytes(), val.as_bytes()).unwrap();
        }
    } // store dropped

    // Read data after reopening
    {
        let mut store = KVStore::open(&path).unwrap();
        for i in 1..=50 {
            let key = format!("key{}", i);
            let result = store.get(key.as_bytes()).unwrap();

            assert!(result.is_some(), "Key {} not found after reopen", key);

            let expected = format!("value{}", i);
            assert_eq!(result.unwrap(), expected.as_bytes());
        }
    }
}

#[test]
fn test_record_creation_and_access() {
    let rec = Record::new()
        .add_str("name", b"Bob".to_vec())
        .add_int64("age", 25)
        .add_str("city", b"NYC".to_vec());

    assert_eq!(rec.len(), 3);
    assert!(rec.is_valid());

    assert_eq!(rec.get("name"), Some(&Value::Bytes(b"Bob".to_vec())));
    assert_eq!(rec.get("age"), Some(&Value::Int64(25)));
    assert_eq!(rec.get("city"), Some(&Value::Bytes(b"NYC".to_vec())));
    assert_eq!(rec.get("nonexistent"), None);
}

#[test]
fn test_table_def_encode_decode() {
    let tdef = TableDef::new("users", 10)
        .add_column("id", ValueType::Int64)
        .add_column("name", ValueType::Bytes)
        .add_column("age", ValueType::Int64)
        .with_pkeys(1);

    // Test encoding primary key
    let vals = vec![Value::Int64(42)];
    let encoded = tdef.encode_key(&vals).unwrap();

    assert!(!encoded.is_empty());
    assert_eq!(encoded[0], 10); // prefix

    // Test decoding
    let decoded = tdef.decode_values(&encoded).unwrap();
    assert_eq!(vals, decoded);
}

#[test]
fn test_table_def_serialization() {
    let tdef = TableDef::new("products", 20)
        .add_column("id", ValueType::Int64)
        .add_column("name", ValueType::Bytes)
        .add_column("price", ValueType::Int64)
        .add_column("description", ValueType::Bytes)
        .with_pkeys(1);

    // Serialize
    let serialized = tdef.serialize().unwrap();
    assert!(!serialized.is_empty());

    // Deserialize
    let deserialized = TableDef::deserialize(&serialized).unwrap();

    assert_eq!(tdef.name, deserialized.name);
    assert_eq!(tdef.prefix, deserialized.prefix);
    assert_eq!(tdef.pkeys, deserialized.pkeys);
    assert_eq!(tdef.cols, deserialized.cols);
    assert_eq!(tdef.types, deserialized.types);
}

#[test]
fn test_table_def_check_record() {
    let tdef = TableDef::new("users", 10)
        .add_column("id", ValueType::Int64)
        .add_column("name", ValueType::Bytes)
        .add_column("age", ValueType::Int64)
        .with_pkeys(1);

    // Create record with columns in different order
    let mut rec = Record::new()
        .add_int64("age", 30)
        .add_str("name", b"Alice".to_vec())
        .add_int64("id", 1);

    // Check and reorder
    tdef.check_record(&mut rec).unwrap();

    // After reordering, columns should match table definition order
    assert_eq!(rec.cols[0], "id");
    assert_eq!(rec.cols[1], "name");
    assert_eq!(rec.cols[2], "age");
}

#[test]
fn test_table_def_missing_primary_key() {
    let tdef = TableDef::new("users", 10)
        .add_column("id", ValueType::Int64)
        .add_column("name", ValueType::Bytes)
        .with_pkeys(1);

    // Create record without primary key
    let mut rec = Record::new().add_str("name", b"Alice".to_vec());

    // This should fail
    let result = tdef.check_record(&mut rec);
    assert!(result.is_err());
}

#[test]
fn test_value_types() {
    // Test Int64
    let v1 = Value::Int64(42);
    assert_eq!(v1.as_i64(), Some(42));
    assert_eq!(v1.as_bytes(), None);

    // Test Bytes
    let v2 = Value::Bytes(b"hello".to_vec());
    assert_eq!(v2.as_bytes(), Some(b"hello".as_ref()));
    assert_eq!(v2.as_i64(), None);

    // Test From conversions
    let v3: Value = 123i64.into();
    assert_eq!(v3, Value::Int64(123));

    let v4: Value = "test".into();
    assert_eq!(v4, Value::Bytes(b"test".to_vec()));
}

#[test]
fn test_complex_workflow() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut store = KVStore::open(temp_file.path()).unwrap();

    // Create a table definition
    let tdef = TableDef::new("users", 50)
        .add_column("id", ValueType::Int64)
        .add_column("name", ValueType::Bytes)
        .add_column("email", ValueType::Bytes)
        .with_pkeys(1);

    // Create and store some records
    for i in 1..=20 {
        let id_val = Value::Int64(i);
        let key = tdef.encode_key(&[id_val.clone()]).unwrap();

        let rec = Record::new()
            .add("id", id_val)
            .add_str("name", format!("U{}", i).into_bytes())
            .add_str("email", format!("e{}", i).into_bytes());

        let val = tdef.encode_key(&rec.vals[1..]).unwrap();
        store.set(&key, &val).unwrap();
    }

    // Query some records
    for i in 1..=20 {
        let id_val = Value::Int64(i);
        let key = tdef.encode_key(&[id_val]).unwrap();

        let result = store.get(&key).unwrap();
        assert!(result.is_some(), "Record {} not found", i);

        let decoded = tdef.decode_values(&result.unwrap()).unwrap();
        assert_eq!(decoded.len(), 2); // name and email
    }

    // Range query
    let start_key = tdef.encode_key(&[Value::Int64(5)]).unwrap();
    let end_key = tdef.encode_key(&[Value::Int64(15)]).unwrap();

    let results = store.get_range(&start_key, &end_key).unwrap();
    assert_eq!(results.len(), 11); // 5 through 15 inclusive
}

#[test]
fn test_concurrent_operations() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut store = KVStore::open(temp_file.path()).unwrap();

    // Perform interleaved operations
    for i in 0..100 {
        let key = format!("k{}", i);
        store.set(key.as_bytes(), b"v1").unwrap();
    }

    for i in 0..50 {
        let key = format!("k{}", i);
        store.set(key.as_bytes(), b"v2").unwrap();
    }

    for i in 0..25 {
        let key = format!("k{}", i);
        store.del(key.as_bytes()).unwrap();
    }

    // Verify final state
    for i in 0..100 {
        let key = format!("k{}", i);
        let result = store.get(key.as_bytes()).unwrap();

        if i < 25 {
            assert_eq!(result, None, "Key {} should be deleted", i);
        } else if i < 50 {
            assert_eq!(result, Some(b"v2".to_vec()), "Key {} should have v2", i);
        } else {
            assert_eq!(result, Some(b"v1".to_vec()), "Key {} should have v1", i);
        }
    }
}
