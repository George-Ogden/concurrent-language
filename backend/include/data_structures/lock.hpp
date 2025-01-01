#pragma once

#include "data_structures/lock.hpp"
#include "system/thread_manager.hpp"

#include <atomic>
#include <mutex>
#include <thread>
#include <type_traits>
#include <variant>

struct Lock {
    bool try_acquire() { return try_acquire_internal(ThreadManager::get_id()); }
    bool release() { return release_internal(ThreadManager::get_id()); }
    void acquire() { acquire_internal(ThreadManager::get_id()); }
    virtual ~Lock() = default;
    enum LockType { Atomic, Builtin };
    virtual LockType type() const = 0;
    static Lock *from_type(LockType type);

  protected:
    virtual bool try_acquire_internal(ThreadManager::ThreadId thread_id) = 0;
    virtual bool release_internal(ThreadManager::ThreadId thread_id) = 0;
    virtual bool held() const = 0;
    virtual void acquire_internal(ThreadManager::ThreadId thread_id) {
        while (!try_acquire_internal(thread_id)) {
        };
    }
};

class ExchangeLock : public Lock {
  public:
    LockType type() const override { return LockType::Atomic; }
    bool held() const override { return holder != 0; }

  protected:
    bool try_acquire_internal(ThreadManager::ThreadId thread_id) override {
        ThreadManager::ThreadId expected = 0;
        return holder.compare_exchange_weak(expected, thread_id + 1,
                                            std::memory_order_acquire);
    }
    bool release_internal(ThreadManager::ThreadId thread_id) override {
        ThreadManager::ThreadId expected = thread_id + 1;
        return holder.compare_exchange_weak(expected, 0,
                                            std::memory_order_release);
    }

  private:
    std::atomic<ThreadManager::ThreadId> holder;
};

class MutexLock : public Lock {
  public:
    LockType type() const override { return LockType::Builtin; }
    explicit MutexLock() : holder(0) {}
    MutexLock(const MutexLock &) = delete;
    MutexLock &operator=(const MutexLock &) = delete;
    bool held() const override { return holder != 0; }

  protected:
    bool try_acquire_internal(ThreadManager::ThreadId thread_id) override {
        if (mutex.try_lock()) {
            holder = thread_id + 1;
            return true;
        }
        return false;
    }
    bool release_internal(ThreadManager::ThreadId thread_id) override {
        if (holder == thread_id + 1) {
            holder = 0;
            mutex.unlock();
            return true;
        }
        return false;
    }
    void acquire_internal(ThreadManager::ThreadId thread_id) override {
        mutex.lock();
        holder = thread_id + 1;
    }

  private:
    std::mutex mutex;
    ThreadManager::ThreadId holder;
};

Lock *Lock::from_type(Lock::LockType type) {
    switch (type) {
    case LockType::Builtin:
        return new MutexLock();
    case LockType::Atomic:
        return new ExchangeLock();
    }
    return nullptr;
}

template <typename T, typename LockT = ExchangeLock> struct Locked {
    static_assert(std::is_base_of_v<Lock, LockT>);
    LockT lock;
    T value;
    T &operator*() { return value; }
    T *operator->() { return &value; }
    bool try_acquire() { return lock.try_acquire(); }
    bool release() { return lock.release(); }
    void acquire() { return lock.acquire(); }
    bool held() const { return lock.held(); }
};
