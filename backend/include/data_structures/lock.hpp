#pragma once

#include "system/thread_manager.hpp"

#include <atomic>
#include <mutex>
#include <type_traits>

struct Lock {
    bool try_acquire();
    bool release();
    void acquire();
    virtual ~Lock() = default;

    enum LockType { Atomic, Builtin };
    virtual LockType type() const = 0;
    static Lock *from_type(LockType type);

  protected:
    virtual bool try_acquire_internal(ThreadManager::ThreadId thread_id) = 0;
    virtual bool release_internal(ThreadManager::ThreadId thread_id) = 0;
    virtual bool held() const = 0;
    virtual void acquire_internal(ThreadManager::ThreadId thread_id);
};

class ExchangeLock : public Lock {
  public:
    LockType type() const override;
    bool held() const override;

  protected:
    bool try_acquire_internal(ThreadManager::ThreadId thread_id) override;
    bool release_internal(ThreadManager::ThreadId thread_id) override;

  private:
    std::atomic<ThreadManager::ThreadId> holder;
};

class MutexLock : public Lock {
  public:
    LockType type() const override;
    explicit MutexLock();
    MutexLock(const MutexLock &) = delete;
    MutexLock &operator=(const MutexLock &) = delete;
    bool held() const override;

  protected:
    bool try_acquire_internal(ThreadManager::ThreadId thread_id) override;
    bool release_internal(ThreadManager::ThreadId thread_id) override;
    void acquire_internal(ThreadManager::ThreadId thread_id) override;

  private:
    std::mutex mutex;
    ThreadManager::ThreadId holder;
};

template <typename T, typename LockT = ExchangeLock> struct Locked {
    static_assert(std::is_base_of_v<Lock, LockT>);
    LockT lock;
    T value;

    T &operator*();
    const T &operator*() const;
    std::decay_t<T> *operator->();
    bool try_acquire();
    bool release();
    void acquire();
    bool held() const;

    Locked() = default;
    template <typename Args> explicit Locked(Args &&value);
};
