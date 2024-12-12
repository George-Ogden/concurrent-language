#pragma once

#include "data_structures/lock.hpp"
#include "system/thread_manager.hpp"

#include <gtest/gtest.h>
#include <range/v3/numeric/accumulate.hpp>

#include <array>
#include <numeric>

template <typename T> class LockAcquireTest : public ::testing::Test {
  public:
    using LockT = T;
    LockT lock;
};

template <typename T>
class EightCoreLockAcquireTest : public LockAcquireTest<T> {
    void SetUp() override { ThreadManager::override_concurrency(8); }

    void TearDown() override { ThreadManager::reset_concurrency_override(); }
};

typedef ::testing::Types<ExchangeLock, MutexLock> LockTypes;

TYPED_TEST_SUITE(LockAcquireTest, LockTypes);
TYPED_TEST_SUITE(EightCoreLockAcquireTest, LockTypes);

TYPED_TEST(LockAcquireTest, TestSingleThreadLockTryAcquire) {
    auto &lock = this->lock;
    ThreadManager::register_self(0);
    ASSERT_TRUE(lock.try_acquire());
    ASSERT_FALSE(lock.try_acquire());
}

TYPED_TEST(LockAcquireTest, TestSingleThreadLockRelease) {
    auto &lock = this->lock;
    ThreadManager::register_self(0);
    ASSERT_FALSE(lock.held());
    ASSERT_TRUE(lock.try_acquire());
    ASSERT_TRUE(lock.held());
    ASSERT_TRUE(lock.release());
    ASSERT_FALSE(lock.held());
    ASSERT_FALSE(lock.release());
}

TYPED_TEST(LockAcquireTest, TestSingleThreadLockAcquireRelease) {
    auto &lock = this->lock;
    ThreadManager::register_self(0);
    ASSERT_TRUE(lock.try_acquire());
    ASSERT_FALSE(lock.try_acquire());
    ASSERT_TRUE(lock.release());
    ASSERT_FALSE(lock.release());
    ASSERT_TRUE(lock.try_acquire());
    ASSERT_FALSE(lock.try_acquire());
    ASSERT_TRUE(lock.release());
    ASSERT_FALSE(lock.release());
}

TYPED_TEST(LockAcquireTest, TestTwoThreadLockTryAcquire) {
    auto &lock = this->lock;
    std::mutex m;

    std::atomic<bool> thread1_acquired(false);
    std::atomic<bool> thread2_acquired(false);

    auto try_acquire_in_thread =
        [&](std::atomic<bool> &result,
            const typename ThreadManager::ThreadId &cpu_id) {
            m.lock();
            ThreadManager::register_self(cpu_id);
            m.unlock();
            result = lock.try_acquire();
        };

    std::thread t1(try_acquire_in_thread, std::ref(thread1_acquired), 0);
    std::thread t2(try_acquire_in_thread, std::ref(thread2_acquired), 1);

    t1.join();
    t2.join();

    ASSERT_TRUE(thread1_acquired || thread2_acquired);
    ASSERT_FALSE(thread1_acquired && thread2_acquired);
}

TYPED_TEST(EightCoreLockAcquireTest, TestEightThreadLockTryAcquire) {
    using Lock = TestFixture::LockT;
    Lock lock;
    std::mutex m;

    std::array<std::atomic<bool>, 8> thread_acquired = {false};
    std::atomic<bool> ready = false;

    auto eventually_try_acquire_in_thread =
        [&](std::atomic<bool> &result,
            const typename ThreadManager::ThreadId &cpu_id) {
            m.lock();
            ThreadManager::register_self(cpu_id);
            m.unlock();
            while (!ready) {
            }
            result = lock.try_acquire();
        };

    std::vector<std::thread> threads;
    for (int i = 0; i < 8; i++) {
        threads.push_back(std::thread(eventually_try_acquire_in_thread,
                                      std::ref(thread_acquired[i]), i));
    }

    ready = true;
    for (auto &thread : threads) {
        thread.join();
    }

    int held = ranges::accumulate(thread_acquired, 0);
    ASSERT_EQ(held, 1);
}

class LockCreationTest : public ::testing::Test {
  public:
    Lock *lock = nullptr;
    void TearDown() override {
        if (lock != nullptr) {
            delete lock;
        }
    }
};

TEST_F(LockCreationTest, MutexLockCreation) {
    this->lock = Lock::from_type(Lock::LockType::Builtin);
    ASSERT_NE(dynamic_cast<MutexLock *>(lock), nullptr);
    ASSERT_EQ(this->lock->type(), Lock::LockType::Builtin);
}

TEST_F(LockCreationTest, ExchangeLockCreation) {
    this->lock = Lock::from_type(Lock::LockType::Atomic);
    ASSERT_NE(dynamic_cast<ExchangeLock *>(lock), nullptr);
    ASSERT_EQ(this->lock->type(), Lock::LockType::Atomic);
}
