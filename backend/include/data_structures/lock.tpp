#pragma once

#include "data_structures/lock.hpp"
#include "system/thread_manager.tpp"

bool Lock::try_acquire() {
    return try_acquire_internal(ThreadManager::get_id());
}

bool Lock::release() {
    return release_internal(ThreadManager::get_id());
}

void Lock::acquire() {
    acquire_internal(ThreadManager::get_id());
}

void Lock::acquire_internal(ThreadManager::ThreadId thread_id) {
    while (!try_acquire_internal(thread_id)) {
    };
}

Lock *Lock::from_type(Lock::LockType type) {
    switch (type) {
    case LockType::Builtin:
        return new MutexLock();
    case LockType::Atomic:
        return new ExchangeLock();
    }
    return nullptr;
}

Lock::LockType ExchangeLock::type() const {
    return LockType::Atomic;
}

bool ExchangeLock::held() const {
    return holder != 0;
}

bool ExchangeLock::try_acquire_internal(ThreadManager::ThreadId thread_id) {
    ThreadManager::ThreadId expected = 0;
    return holder.compare_exchange_weak(expected, thread_id + 1,
                                        std::memory_order_acquire);
}

bool ExchangeLock::release_internal(ThreadManager::ThreadId thread_id) {
    ThreadManager::ThreadId expected = thread_id + 1;
    return holder.compare_exchange_weak(expected, 0,
                                        std::memory_order_release);
}

Lock::LockType MutexLock::type() const {
    return LockType::Builtin;
}

MutexLock::MutexLock() : holder(0) {}

bool MutexLock::held() const {
    return holder != 0;
}

bool MutexLock::try_acquire_internal(ThreadManager::ThreadId thread_id) {
    if (mutex.try_lock()) {
        holder = thread_id + 1;
        return true;
    }
    return false;
}

bool MutexLock::release_internal(ThreadManager::ThreadId thread_id) {
    if (holder == thread_id + 1) {
        holder = 0;
        mutex.unlock();
        return true;
    }
    return false;
}

void MutexLock::acquire_internal(ThreadManager::ThreadId thread_id) {
    mutex.lock();
    holder = thread_id + 1;
}

template <typename T, typename LockT>
T &Locked<T, LockT>::operator*() {
    return value;
}

template <typename T, typename LockT>
std::decay_t<T> *Locked<T, LockT>::operator->() {
    return &value;
}

template <typename T, typename LockT>
bool Locked<T, LockT>::try_acquire() {
    return lock.try_acquire();
}

template <typename T, typename LockT>
bool Locked<T, LockT>::release() {
    return lock.release();
}

template <typename T, typename LockT>
void Locked<T, LockT>::acquire() {
    lock.acquire();
}

template <typename T, typename LockT>
bool Locked<T, LockT>::held() const {
    return lock.held();
}

template <typename T, typename LockT>
template <typename Args>
Locked<T, LockT>::Locked(Args &&value) : value(value) {}
