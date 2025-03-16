#pragma once

#include "data_structures/cyclic_queue.tpp"
#include "data_structures/lock.tpp"
#include "system/thread_manager.tpp"

#include <gtest/gtest.h>

#include <atomic>
#include <chrono>
#include <mutex>
#include <optional>
#include <pthread.h>

using namespace std::chrono_literals;

class QueueTest : public ::testing::TestWithParam<unsigned> {
  protected:
    CyclicQueue<unsigned> queue;
    void SetUp() override {
        auto num_cpus = GetParam();
        ThreadManager::override_concurrency(num_cpus);
        ThreadManager::register_self(0);
    }

    void TearDown() override { ThreadManager::reset_concurrency_override(); }
};

TEST_P(QueueTest, TestQueuePush) {
    auto num_cpus = ThreadManager::available_concurrency();
    std::atomic<unsigned> ready{num_cpus + 1};
    std::mutex m;
    queue = CyclicQueue<unsigned>{1000 * num_cpus};

    auto work = [&](const typename ThreadManager::ThreadId &cpu_id) {
        m.lock();
        ThreadManager::register_self(cpu_id);
        m.unlock();
        ready.fetch_sub(1);
        while (!ready.load() == 0) {
        }
        for (unsigned i = 0; i < 1000; i++) {
            queue.push(i);
        }
    };

    std::vector<std::thread> threads;
    for (unsigned i = 0; i < num_cpus; i++) {
        threads.push_back(std::thread(work, i));
    }
    ready.fetch_sub(1);
    for (auto &thread : threads) {
        thread.join();
    }

    std::atomic<unsigned> total{0};
    ASSERT_TRUE(queue.size() == num_cpus * 1000);
    std::optional<unsigned> x = 0;
    do {
        total.fetch_add(x.value(), std::memory_order_relaxed);
        x = queue.pop();
    } while (x.has_value());
    ASSERT_EQ(total, 1000 * 999 / 2 * num_cpus);
}

TEST_P(QueueTest, TestQueuePop) {
    auto num_cpus = ThreadManager::available_concurrency();
    std::atomic<unsigned> ready{num_cpus + 1};
    std::atomic<unsigned> total{0};
    std::mutex m;
    queue = CyclicQueue<unsigned>{1000 * num_cpus};

    auto work = [&](const typename ThreadManager::ThreadId &cpu_id) {
        m.lock();
        ThreadManager::register_self(cpu_id);
        m.unlock();
        ready.fetch_sub(1);
        while (!ready.load() == 0) {
        }
        for (unsigned i = 0; i < 1000; i++) {
            auto x = queue.pop();
            total += x.value();
        }
    };

    std::vector<std::thread> threads;
    for (unsigned i = 0; i < num_cpus; i++) {
        threads.push_back(std::thread(work, i));
    }
    for (unsigned j = 0; j < num_cpus; j++) {
        for (unsigned i = 0; i < 1000; i++) {
            queue.push(i);
        }
    }
    ready.fetch_sub(1);
    for (auto &thread : threads) {
        thread.join();
    }

    ASSERT_TRUE(queue.empty());
    ASSERT_EQ(total, 1000 * 999 / 2 * num_cpus);
}

TEST_P(QueueTest, TestQueueNoWrap) {
    auto num_cpus = ThreadManager::available_concurrency();
    std::atomic<unsigned> ready{num_cpus + 1};
    std::atomic<unsigned> total{0};
    std::mutex m;
    queue = CyclicQueue<unsigned>{1000 * num_cpus};

    auto work = [&](const typename ThreadManager::ThreadId &cpu_id) {
        m.lock();
        ThreadManager::register_self(cpu_id);
        m.unlock();
        ready.fetch_sub(1);
        while (!ready.load() == 0) {
        }
        for (unsigned i = 0; i < 1000; i++) {
            queue.push(i);
            std::optional<unsigned> x{0};
            do {
                std::this_thread::sleep_for(10us);
                total.fetch_add(x.value(), std::memory_order_relaxed);
                x = queue.pop();
            } while (x.has_value());
        }
    };

    std::vector<std::thread> threads;
    for (unsigned i = 0; i < num_cpus; i++) {
        threads.push_back(std::thread(work, i));
    }
    ready.fetch_sub(1);
    for (auto &thread : threads) {
        thread.join();
    }
    ASSERT_LE(queue.size(), num_cpus);

    std::optional<unsigned> x = 0;
    do {
        total.fetch_add(x.value(), std::memory_order_relaxed);
        x = queue.pop();
    } while (x.has_value());

    ASSERT_TRUE(queue.empty());
    ASSERT_EQ(queue.size(), 0);
    ASSERT_EQ(total, 1000 * 999 / 2 * num_cpus);
}

TEST_P(QueueTest, TestQueue) {
    auto num_cpus = ThreadManager::available_concurrency();
    std::atomic<unsigned> ready{num_cpus + 1};
    std::atomic<unsigned> total{0};
    std::mutex m;
    queue = CyclicQueue<unsigned>{num_cpus};

    auto work = [&](const typename ThreadManager::ThreadId &cpu_id) {
        m.lock();
        ThreadManager::register_self(cpu_id);
        m.unlock();
        ready.fetch_sub(1);
        while (!ready.load() == 0) {
        }
        for (unsigned i = 0; i < 1000; i++) {
            queue.push(i);
            std::optional<unsigned> x{0};
            do {
                std::this_thread::sleep_for(10us);
                total.fetch_add(x.value(), std::memory_order_relaxed);
                x = queue.pop();
            } while (x.has_value());
        }
    };

    std::vector<std::thread> threads;
    for (unsigned i = 0; i < num_cpus; i++) {
        threads.push_back(std::thread(work, i));
    }
    ready.fetch_sub(1);
    for (auto &thread : threads) {
        thread.join();
    }
    ASSERT_LE(queue.size(), num_cpus);

    std::optional<unsigned> x = 0;
    do {
        total.fetch_add(x.value(), std::memory_order_relaxed);
        x = queue.pop();
    } while (x.has_value());

    ASSERT_TRUE(queue.empty());
    ASSERT_EQ(queue.size(), 0);
    ASSERT_EQ(total, 1000 * 999 / 2 * num_cpus);
}

TEST(QueueTest, TestQueueReference) {
    ThreadManager::register_self(0);
    CyclicQueue<unsigned> queue{1};
    auto &x = queue.push(0);
    x++;
    ASSERT_EQ(queue.pop(), 1);
}

std::vector<unsigned> queue_test_cpu_counts = {1, 2, 3, 4, 6, 8};
INSTANTIATE_TEST_SUITE_P(QueueTests, QueueTest,
                         ::testing::ValuesIn(queue_test_cpu_counts));
