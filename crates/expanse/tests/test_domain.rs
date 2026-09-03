//! Integration tests for interned set domain (Issue #611).

#![cfg(target_pointer_width = "64")]

use expanse_trie::domain::{
    DomainError, DomainMismatch, DomainOrdinal, DomainSet, ExpanseDomainDict,
};

#[test]
fn test_domain_intern_and_resolve() {
    let mut dict = ExpanseDomainDict::new();
    assert_eq!(dict.len(), 0);
    assert!(dict.is_empty());

    let id1 = dict.intern(b"user:8f3c1e").expect("intern failed");
    assert_eq!(id1.domain_id(), dict.domain_id());
    assert_eq!(id1.ordinal(), 0);
    assert_eq!(dict.len(), 1);
    assert!(!dict.is_empty());

    // Re-interning existing key returns identical ordinal without growing vocabulary
    let id1_again = dict.intern(b"user:8f3c1e").expect("intern failed");
    assert_eq!(id1, id1_again);
    assert_eq!(dict.len(), 1);

    // Intern second distinct key
    let id2 = dict.intern(b"user:999999").expect("intern failed");
    assert_eq!(id2.ordinal(), 1);
    assert_eq!(dict.len(), 2);

    // Resolve ordinals back to slices
    assert_eq!(dict.resolve_id(id1).unwrap(), Some(&b"user:8f3c1e"[..]));
    assert_eq!(dict.resolve_id(id2).unwrap(), Some(&b"user:999999"[..]));
}

#[test]
fn test_domain_set_insert_contains_remove() {
    let mut dict = ExpanseDomainDict::new();
    let mut set = dict.new_set();
    assert_eq!(set.domain_id(), dict.domain_id());
    assert_eq!(set.len(), 0);
    assert!(set.is_empty());

    // Insert new element
    assert!(dict.insert(&mut set, b"alpha").unwrap());
    assert_eq!(set.len(), 1);
    assert!(!set.is_empty());

    // Duplicate insert returns false
    assert!(!dict.insert(&mut set, b"alpha").unwrap());
    assert_eq!(set.len(), 1);

    // Insert second element
    assert!(dict.insert(&mut set, b"beta").unwrap());
    assert_eq!(set.len(), 2);

    // Membership checks
    assert!(dict.contains(&set, b"alpha").unwrap());
    assert!(dict.contains(&set, b"beta").unwrap());

    // Checking an un-interned key returns false without growing dictionary vocabulary
    assert_eq!(dict.len(), 2);
    assert!(!dict.contains(&set, b"unseen_key").unwrap());
    assert_eq!(dict.len(), 2);

    // Removal
    assert!(dict.remove(&mut set, b"alpha").unwrap());
    assert_eq!(set.len(), 1);
    assert!(!dict.contains(&set, b"alpha").unwrap());
    assert!(dict.contains(&set, b"beta").unwrap());

    // Removing absent key returns false
    assert!(!dict.remove(&mut set, b"alpha").unwrap());
    assert!(!dict.remove(&mut set, b"unseen_key").unwrap());
}

#[test]
fn test_domain_pure_set_algebra() {
    let mut dict = ExpanseDomainDict::new();
    let mut set_a = dict.new_set();
    let mut set_b = dict.new_set();

    dict.insert(&mut set_a, b"apple").unwrap();
    dict.insert(&mut set_a, b"banana").unwrap();
    dict.insert(&mut set_a, b"cherry").unwrap();

    dict.insert(&mut set_b, b"banana").unwrap();
    dict.insert(&mut set_b, b"cherry").unwrap();
    dict.insert(&mut set_b, b"date").unwrap();

    // Intersection
    let inter = set_a.intersection(&set_b).unwrap();
    assert_eq!(inter.domain_id(), dict.domain_id());
    assert_eq!(inter.len(), 2);
    assert_eq!(set_a.intersection_len(&set_b).unwrap(), 2);
    assert!(dict.contains(&inter, b"banana").unwrap());
    assert!(dict.contains(&inter, b"cherry").unwrap());
    assert!(!dict.contains(&inter, b"apple").unwrap());
    assert!(!dict.contains(&inter, b"date").unwrap());

    // Union
    let un = set_a.union(&set_b).unwrap();
    assert_eq!(un.len(), 4);
    assert_eq!(set_a.union_len(&set_b).unwrap(), 4);
    assert!(dict.contains(&un, b"apple").unwrap());
    assert!(dict.contains(&un, b"banana").unwrap());
    assert!(dict.contains(&un, b"cherry").unwrap());
    assert!(dict.contains(&un, b"date").unwrap());

    // Difference (A \ B)
    let diff = set_a.difference(&set_b).unwrap();
    assert_eq!(diff.len(), 1);
    assert!(dict.contains(&diff, b"apple").unwrap());

    // Symmetric difference ((A \ B) ∪ (B \ A))
    let sym_diff = set_a.symmetric_difference(&set_b).unwrap();
    assert_eq!(sym_diff.len(), 2);
    assert!(dict.contains(&sym_diff, b"apple").unwrap());
    assert!(dict.contains(&sym_diff, b"date").unwrap());

    // Subset and disjointness
    assert!(inter.is_subset(&set_a).unwrap());
    assert!(inter.is_subset(&set_b).unwrap());
    assert!(!set_a.is_subset(&set_b).unwrap());

    let mut set_c = dict.new_set();
    dict.insert(&mut set_c, b"fig").unwrap();
    assert!(set_a.is_disjoint(&set_c).unwrap());
    assert!(!set_a.is_disjoint(&set_b).unwrap());
}

#[test]
fn test_domain_k_way_aggregate_algebra() {
    let mut dict = ExpanseDomainDict::new();
    let mut s1 = dict.new_set();
    let mut s2 = dict.new_set();
    let mut s3 = dict.new_set();

    dict.insert(&mut s1, b"shared").unwrap();
    dict.insert(&mut s1, b"s1_only").unwrap();
    dict.insert(&mut s1, b"s1_s2").unwrap();

    dict.insert(&mut s2, b"shared").unwrap();
    dict.insert(&mut s2, b"s2_only").unwrap();
    dict.insert(&mut s2, b"s1_s2").unwrap();

    dict.insert(&mut s3, b"shared").unwrap();
    dict.insert(&mut s3, b"s3_only").unwrap();

    // 3-way intersection
    let inter_all = DomainSet::intersection_many(&[&s1, &s2, &s3])
        .unwrap()
        .unwrap();
    assert_eq!(inter_all.len(), 1);
    assert_eq!(
        DomainSet::intersection_len_many(&[&s1, &s2, &s3]).unwrap(),
        1
    );
    assert!(dict.contains(&inter_all, b"shared").unwrap());

    // 3-way union
    let union_all = DomainSet::union_many(&[&s1, &s2, &s3]).unwrap().unwrap();
    assert_eq!(union_all.len(), 5);
    assert_eq!(DomainSet::union_len_many(&[&s1, &s2, &s3]).unwrap(), 5);

    // Empty slice handling
    assert!(DomainSet::intersection_many(&[]).unwrap().is_none());
    assert_eq!(DomainSet::intersection_len_many(&[]).unwrap(), 0);
    assert!(DomainSet::union_many(&[]).unwrap().is_none());
    assert_eq!(DomainSet::union_len_many(&[]).unwrap(), 0);
}

#[test]
fn test_cross_domain_negative_controls() {
    let mut dict_a = ExpanseDomainDict::new();
    let mut dict_b = ExpanseDomainDict::new();
    assert_ne!(dict_a.domain_id(), dict_b.domain_id());

    let mut set_a = dict_a.new_set();
    let mut set_b = dict_b.new_set();

    dict_a.insert(&mut set_a, b"entity_1").unwrap();
    dict_b.insert(&mut set_b, b"entity_1").unwrap();

    // 1. Attempting to insert a set into the wrong dictionary fails loud
    let err = dict_a.insert(&mut set_b, b"invalid").unwrap_err();
    assert_eq!(
        err,
        DomainError::Mismatch(DomainMismatch {
            expected: dict_a.domain_id(),
            got: dict_b.domain_id(),
        })
    );

    // 2. Cross-domain set algebra fails with DomainMismatch
    assert_eq!(
        set_a.intersection(&set_b).unwrap_err(),
        DomainMismatch {
            expected: dict_a.domain_id(),
            got: dict_b.domain_id(),
        }
    );
    assert_eq!(
        set_a.union(&set_b).unwrap_err(),
        DomainMismatch {
            expected: dict_a.domain_id(),
            got: dict_b.domain_id(),
        }
    );
    assert_eq!(
        set_a.difference(&set_b).unwrap_err(),
        DomainMismatch {
            expected: dict_a.domain_id(),
            got: dict_b.domain_id(),
        }
    );
    assert_eq!(
        set_a.symmetric_difference(&set_b).unwrap_err(),
        DomainMismatch {
            expected: dict_a.domain_id(),
            got: dict_b.domain_id(),
        }
    );
    assert_eq!(
        set_a.intersection_len(&set_b).unwrap_err(),
        DomainMismatch {
            expected: dict_a.domain_id(),
            got: dict_b.domain_id(),
        }
    );
    assert_eq!(
        set_a.union_len(&set_b).unwrap_err(),
        DomainMismatch {
            expected: dict_a.domain_id(),
            got: dict_b.domain_id(),
        }
    );
    assert_eq!(
        set_a.is_subset(&set_b).unwrap_err(),
        DomainMismatch {
            expected: dict_a.domain_id(),
            got: dict_b.domain_id(),
        }
    );
    assert_eq!(
        set_a.is_disjoint(&set_b).unwrap_err(),
        DomainMismatch {
            expected: dict_a.domain_id(),
            got: dict_b.domain_id(),
        }
    );

    // 3. Cross-domain k-way algebra fails
    assert_eq!(
        DomainSet::intersection_many(&[&set_a, &set_b]).unwrap_err(),
        DomainMismatch {
            expected: dict_a.domain_id(),
            got: dict_b.domain_id(),
        }
    );

    // 4. Cross-domain resolution fails
    assert_eq!(
        dict_a.resolve(&set_b).err().unwrap(),
        DomainMismatch {
            expected: dict_a.domain_id(),
            got: dict_b.domain_id(),
        }
    );
    let id_b = dict_b.intern(b"test").unwrap();
    assert_eq!(
        dict_a.resolve_id(id_b).unwrap_err(),
        DomainMismatch {
            expected: dict_a.domain_id(),
            got: dict_b.domain_id(),
        }
    );
}

#[test]
fn test_embedded_nul_and_binary_keys() {
    let mut dict = ExpanseDomainDict::new();
    let mut set = dict.new_set();

    // Real binary UUIDs and keys containing embedded 0x00 and 0x01 bytes
    let key1 = b"uuid\x00alpha";
    let key2 = b"uuid\x00beta";
    let key3 = b"\x00\x01\x00\x01raw_bytes\x00\x00";

    dict.insert(&mut set, key1).unwrap();
    dict.insert(&mut set, key2).unwrap();
    dict.insert(&mut set, key3).unwrap();

    assert_eq!(dict.len(), 3);
    assert_eq!(set.len(), 3);

    // In un-escaped StrMap, key1 and key2 would alias to "uuid" in release mode.
    // With order-preserving escape encoding, they are strictly distinct:
    assert!(dict.contains(&set, key1).unwrap());
    assert!(dict.contains(&set, key2).unwrap());
    assert!(dict.contains(&set, key3).unwrap());
    assert!(!dict.contains(&set, b"uuid\x00gamma").unwrap());

    // Resolve verifies exact original bytes round-trip
    let id1 = dict.intern(key1).unwrap();
    let id2 = dict.intern(key2).unwrap();
    let id3 = dict.intern(key3).unwrap();

    assert_ne!(id1, id2);
    assert_eq!(dict.resolve_id(id1).unwrap(), Some(&key1[..]));
    assert_eq!(dict.resolve_id(id2).unwrap(), Some(&key2[..]));
    assert_eq!(dict.resolve_id(id3).unwrap(), Some(&key3[..]));
}

#[test]
fn test_zero_copy_set_resolution() {
    let mut dict = ExpanseDomainDict::new();
    let mut set = dict.new_set();

    let expected = vec![
        b"alpha".to_vec(),
        b"beta".to_vec(),
        b"gamma".to_vec(),
        b"uuid:\x00\x01\x02".to_vec(),
    ];

    for key in &expected {
        dict.insert(&mut set, key).unwrap();
    }

    let resolved: Vec<Vec<u8>> = dict
        .resolve(&set)
        .unwrap()
        .map(|slice| slice.to_vec())
        .collect();

    let expected_sorted = expected.clone();
    // Ordinal order is insertion order
    assert_eq!(resolved, expected_sorted);
}

#[test]
fn test_batch_operations() {
    let mut dict = ExpanseDomainDict::new();
    let mut set = dict.new_set();

    let keys: Vec<&[u8]> = vec![b"batch_0", b"batch_1", b"batch_2", b"batch_3"];
    let mut ids = vec![DomainOrdinal::default(); 4];

    dict.intern_batch(&keys, &mut ids).unwrap();
    assert_eq!(dict.len(), 4);

    let added = dict.insert_batch(&mut set, &keys).unwrap();
    assert_eq!(added, 4);
    assert_eq!(set.len(), 4);

    // Re-inserting returns 0 added
    let added_again = dict.insert_batch(&mut set, &keys).unwrap();
    assert_eq!(added_again, 0);
}

#[test]
fn test_memory_introspection_honesty() {
    let mut dict = ExpanseDomainDict::new();
    let mut set = dict.new_set();

    dict.insert(&mut set, b"key_one").unwrap();
    dict.insert(&mut set, b"key_two").unwrap();

    let dict_mem = dict.dictionary_mem_used();
    let set_mem = set.mem_used();

    assert!(dict_mem > 0, "dictionary memory must be accounted for");
    assert!(set_mem > 0, "set memory must be accounted for");
}
