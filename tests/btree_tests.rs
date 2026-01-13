use mini_db::btree::BPTreeDisk;
use mini_db::btree::page::KeyVal;
use rand::seq::SliceRandom;
use std::ops::Deref;
use tempfile::NamedTempFile;

fn int_to_bytes(x: i64) -> Vec<u8> {
    x.to_be_bytes().to_vec()
}

#[test]
fn test_btree_disk_sequential() {
    let max_num = 100;
    let temp_file = NamedTempFile::new().unwrap();
    let mut test_db = BPTreeDisk::new(temp_file.path()).unwrap();

    // Insert test: insert nodes from 1 to max_num
    for i in 1..=max_num {
        let key = int_to_bytes(i);
        let val = int_to_bytes(i);
        println!("insert key = {:?}", key);
        test_db.insert(&key, &val).unwrap();
        println!("=========================================");
    }

    // Find test: Find these kv if they are the same
    for i in 1..=max_num {
        let key = int_to_bytes(i);
        let kv = test_db.find(&key).unwrap();
        let expected = KeyVal::from_i64(i, i);

        assert!(kv.is_some(), "Cannot find key = {}", i);
        assert_eq!(kv.unwrap(), expected, "Find test failed for key = {}", i);
    }

    // Set test: Set to i + 5
    for i in 1..=max_num {
        let key = int_to_bytes(i);
        let val = int_to_bytes(i + 5);
        println!("set key = {:?}", key);
        test_db.set(&key, &val).unwrap();
        println!("=========================================");
    }

    // Find test: Find these kv if they are the same
    for i in 1..=max_num {
        let key = int_to_bytes(i);
        let kv = test_db.find(&key).unwrap();
        let expected = KeyVal::from_i64(i, i + 5);

        assert!(kv.is_some(), "Cannot find key = {}", i);
        assert_eq!(kv.unwrap(), expected, "Find test failed for key = {}", i);
    }

    // Iter test: Get an iterator and next 10 times
    for i in 1..=max_num - 10 {
        let key = int_to_bytes(i);
        let mut iter = test_db.seek_ge(&key).unwrap();

        for j in 0..10 {
            let kv = iter.deref().expect("Iterator exhausted too early");
            let expected = KeyVal::from_i64(i + j, i + j + 5);

            assert_eq!(kv, expected, "Iter test failed for i = {} and j = {}", i, j);

            if j < 9 {
                iter.next(&mut test_db).unwrap();
            }
        }
    }

    // Del test: delete odd keys
    for i in 1..=max_num {
        if i % 2 == 0 {
            continue;
        }
        let key = int_to_bytes(i);
        println!("del key = {:?}", key);
        test_db.del(&key).unwrap();
        println!("=========================================");
    }

    // Find test: Find these kv if they are the same
    for i in 1..=max_num {
        let key = int_to_bytes(i);
        let kv = test_db.find(&key).unwrap();

        if i % 2 == 0 {
            let expected = KeyVal::from_i64(i, i + 5);
            assert!(kv.is_some(), "Cannot find key = {}", i);
            assert_eq!(kv.unwrap(), expected, "Find test failed for key = {}", i);
        } else {
            assert!(kv.is_none(), "Expected key {} to be deleted", i);
        }
    }
}

#[test]
fn test_btree_disk_shuffle() {
    let max_num = 2000;
    let mut rng = rand::thread_rng();

    let mut numbers: Vec<i64> = (1..=max_num).collect();
    numbers.shuffle(&mut rng);

    let temp_file = NamedTempFile::new().unwrap();
    let mut test_db = BPTreeDisk::new(temp_file.path()).unwrap();

    // Insert test: insert in random order
    for &i in &numbers {
        let key = int_to_bytes(i);
        let val = int_to_bytes(i);
        println!("insert key = {:?}", key);
        test_db.insert(&key, &val).unwrap();
        println!("=========================================");
    }

    // Find test: Find these kv if they are the same
    numbers.shuffle(&mut rng);
    for &i in &numbers {
        let key = int_to_bytes(i);
        let kv = test_db.find(&key).unwrap();
        let expected = KeyVal::from_i64(i, i);

        assert!(kv.is_some(), "Cannot find key = {}", i);
        assert_eq!(kv.unwrap(), expected, "Find test failed for key = {}", i);
    }

    // Set test: Set to i + 5
    numbers.shuffle(&mut rng);
    for &i in &numbers {
        let key = int_to_bytes(i);
        let val = int_to_bytes(i + 5);
        println!("set key = {:?}", key);
        test_db.set(&key, &val).unwrap();
        println!("=========================================");
    }

    // Find test: Find these kv if they are the same
    numbers.shuffle(&mut rng);
    for &i in &numbers {
        let key = int_to_bytes(i);
        let kv = test_db.find(&key).unwrap();
        let expected = KeyVal::from_i64(i, i + 5);

        assert!(kv.is_some(), "Cannot find key = {}", i);
        assert_eq!(kv.unwrap(), expected, "Find test failed for key = {}", i);
    }

    // Del test: delete odd keys
    numbers.shuffle(&mut rng);
    for &i in &numbers {
        if i % 2 == 0 {
            continue;
        }
        let key = int_to_bytes(i);
        println!("del key = {:?}", key);
        test_db.del(&key).unwrap();
        println!("=========================================");
    }

    // Find test: Find these kv if they are the same
    numbers.shuffle(&mut rng);
    for &i in &numbers {
        let key = int_to_bytes(i);
        let kv = test_db.find(&key).unwrap();

        if i % 2 == 0 {
            let expected = KeyVal::from_i64(i, i + 5);
            assert!(kv.is_some(), "Cannot find key = {}", i);
            assert_eq!(kv.unwrap(), expected, "Find test failed for key = {}", i);
        } else {
            assert!(kv.is_none(), "Expected key {} to be deleted", i);
        }
    }
}

#[test]
fn test_btree_persistence() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_path_buf();

    // Insert data
    {
        let mut db = BPTreeDisk::new(&path).unwrap();
        for i in 1..=100 {
            let key = int_to_bytes(i);
            let val = int_to_bytes(i * 10);
            db.insert(&key, &val).unwrap();
        }
    } // db is dropped here

    // Reopen and verify data persists
    {
        let mut db = BPTreeDisk::new(&path).unwrap();
        for i in 1..=100 {
            let key = int_to_bytes(i);
            let kv = db.find(&key).unwrap();
            assert!(kv.is_some(), "Key {} not found after reopening", i);

            let expected = KeyVal::from_i64(i, i * 10);
            assert_eq!(kv.unwrap(), expected, "Value mismatch for key {}", i);
        }
    }
}

#[test]
fn test_btree_large_dataset() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut db = BPTreeDisk::new(temp_file.path()).unwrap();

    let count = 5000;

    // Insert large dataset
    for i in 1..=count {
        let key = int_to_bytes(i);
        let val = int_to_bytes(i * 100);
        db.insert(&key, &val).unwrap();
    }

    // Verify all entries
    for i in 1..=count {
        let key = int_to_bytes(i);
        let kv = db.find(&key).unwrap();
        assert!(kv.is_some(), "Key {} not found", i);
    }

    // Range query test
    let start_key = int_to_bytes(100);
    let mut iter = db.seek_ge(&start_key).unwrap();
    let mut count_iter = 0;

    while let Some(_kv) = iter.deref() {
        count_iter += 1;
        if !iter.next(&mut db).unwrap() || count_iter >= 100 {
            break;
        }
    }

    assert!(count_iter >= 100, "Iterator didn't traverse enough entries");
}
