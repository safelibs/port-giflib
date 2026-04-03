#![allow(non_snake_case)]

use core::ptr;

use crate::bootstrap::catch_panic_or;
use crate::ffi::{GifHashTableType, HT_EMPTY_KEY, HT_KEY_MASK, HT_SIZE};
use crate::memory::alloc_struct;

fn key_item(Item: u32) -> usize {
    (((Item >> 12) ^ Item) & HT_KEY_MASK) as usize
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn clear_hash_table_impl(HashTable: *mut GifHashTableType) {
    if HashTable.is_null() {
        return;
    }
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        ptr::write_bytes((*HashTable).HTable.as_mut_ptr(), 0xFF, HT_SIZE);
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn insert_hash_table_impl(HashTable: *mut GifHashTableType, Key: u32, Code: i32) {
    if HashTable.is_null() {
        return;
    }

    let mut HKey = key_item(Key);
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let HTable = unsafe { &mut (*HashTable).HTable };
    while (HTable[HKey] >> 12) != HT_EMPTY_KEY {
        HKey = (HKey + 1) & (HT_KEY_MASK as usize);
    }
    HTable[HKey] = (Key << 12) | ((Code as u32) & 0x0FFF);
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn exists_hash_table_impl(HashTable: *mut GifHashTableType, Key: u32) -> i32 {
    if HashTable.is_null() {
        return -1;
    }

    let mut HKey = key_item(Key);
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let HTable = unsafe { &(*HashTable).HTable };
    loop {
        let entry = HTable[HKey];
        let HTKey = entry >> 12;
        if HTKey == HT_EMPTY_KEY {
            return -1;
        }
        if Key == HTKey {
            return (entry & 0x0FFF) as i32;
        }
        HKey = (HKey + 1) & (HT_KEY_MASK as usize);
    }
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn _InitHashTable() -> *mut GifHashTableType {
    catch_panic_or(ptr::null_mut(), || unsafe {
        let HashTable = alloc_struct::<GifHashTableType>();
        if HashTable.is_null() {
            return ptr::null_mut();
        }
        clear_hash_table_impl(HashTable);
        HashTable
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn _ClearHashTable(HashTable: *mut GifHashTableType) {
    catch_panic_or((), || unsafe {
        clear_hash_table_impl(HashTable);
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn _InsertHashTable(HashTable: *mut GifHashTableType, Key: u32, Code: i32) {
    catch_panic_or((), || unsafe {
        insert_hash_table_impl(HashTable, Key, Code);
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn _ExistsHashTable(HashTable: *mut GifHashTableType, Key: u32) -> i32 {
    catch_panic_or(-1, || unsafe { exists_hash_table_impl(HashTable, Key) })
}
