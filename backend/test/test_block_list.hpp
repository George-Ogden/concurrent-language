#pragma once

#include "data_structures/block_list.hpp"
#include "system/thread_manager.tpp"

#include <gtest/gtest.h>
#include <range/v3/numeric/iota.hpp>
#include <range/v3/view/zip.hpp>

#include <atomic>
#include <chrono>
#include <iterator>
#include <mutex>
#include <optional>
#include <pthread.h>
#include <queue>
#include <ranges>
#include <stack>

using namespace std::chrono_literals;

class BlockListTest : public ::testing::Test {
  protected:
    void SetUp() override { ThreadManager::register_self(0); }
};

TEST_F(BlockListTest, BlockListTypesTest) {
    using I = BlockList<int>::iterator;
    static_assert(std::forward_iterator<I>);
    static_assert(std::bidirectional_iterator<I>);
    static_assert(std::ranges::forward_range<BlockList<int>>);
    std::stack<int, BlockList<int>> stack;
    std::queue<int, BlockList<int>> queue;
}

TEST_F(BlockListTest, TestEmptySizeIsZero) {
    BlockList<int> list;
    ASSERT_EQ(list.size(), 0);
}

TEST_F(BlockListTest, TestSizeIncrementsOnPushBack) {
    BlockList<int> list;
    ASSERT_EQ(list.size(), 0);
    list.push_back(1);
    ASSERT_EQ(list.size(), 1);
    int x = 4;
    list.push_back(x);
    ASSERT_EQ(list.size(), 2);
}

TEST_F(BlockListTest, TestBlockListIterators) {
    BlockList<int> list;
    ASSERT_EQ(list.end(), list.begin());
    for (int i = 10000; i > 0; i--) {
        list.push_back(i);
    }
    int j = 10000;
    for (BlockList<int>::iterator it = list.begin(); it != list.end(); ++it) {
        ASSERT_EQ(*it, j);
        j--;
    }

    j = 10000;
    for (int x : list) {
        ASSERT_EQ(x, j);
        j--;
    }

    j = 10000;
    for (int &x : list) {
        ASSERT_EQ(x, j);
        j--;
    }

    j = 10000;
    for (BlockList<int>::const_iterator it = list.cbegin(); it != list.cend();
         ++it) {
        ASSERT_EQ(*it, j);
        j--;
    }
    ASSERT_EQ(j, 0);

    j = 10000;
    for (BlockList<int>::const_iterator it = list.begin(); it != list.end();
         ++it) {
        ASSERT_EQ(*it, j);
        j--;
    }
    ASSERT_EQ(j, 0);

    j = 10000;
    for (const int x : list) {
        ASSERT_EQ(x, j);
        j--;
    }
    ASSERT_EQ(j, 0);

    j = 10000;
    for (const int &x : list) {
        ASSERT_EQ(x, j);
        j--;
    }
    ASSERT_EQ(j, 0);

    j = 1;
    for (BlockList<int>::const_iterator it = list.end(); it != list.begin();) {
        --it;
        ASSERT_EQ(*it, j);
        j++;
    }
    ASSERT_EQ(j, 10001);

    j = 1;
    for (BlockList<int>::iterator it = list.end(); it != list.begin();) {
        --it;
        ASSERT_EQ(*it, j);
        j++;
    }
    ASSERT_EQ(j, 10001);
}

TEST_F(BlockListTest, TestPopFront) {
    BlockList<int> list; // []
    ASSERT_EQ(list.size(), 0);

    list.push_back(1); // [1]
    ASSERT_EQ(list.size(), 1);

    int x = 4;
    list.push_back(x); // [1,4]
    ASSERT_EQ(list.size(), 2);
    ASSERT_EQ(list.front(), 1);

    list.pop_front(); // [4]
    ASSERT_EQ(list.size(), 1);
    ASSERT_EQ(list.front(), 4);

    list.push_back(8); // [4,8]
    ASSERT_EQ(list.size(), 2);
    ASSERT_EQ(list.front(), 4);

    list.pop_front(); // [8]
    ASSERT_EQ(list.size(), 1);
    ASSERT_EQ(list.front(), 8);

    list.pop_front(); // []
    ASSERT_EQ(list.size(), 0);
}

TEST_F(BlockListTest, TestPopBack) {
    BlockList<int> list; // []
    ASSERT_EQ(list.size(), 0);
    ASSERT_TRUE(list.empty());

    list.push_back(1); // [1]
    ASSERT_EQ(list.back(), 1);
    ASSERT_EQ(list.size(), 1);
    ASSERT_FALSE(list.empty());

    int x = 4;
    list.push_back(x); // [1,4]
    ASSERT_EQ(list.size(), 2);
    ASSERT_EQ(list.back(), 4);
    ASSERT_FALSE(list.empty());

    list.pop_back(); // [1]
    ASSERT_EQ(list.size(), 1);
    ASSERT_EQ(list.back(), 1);
    ASSERT_FALSE(list.empty());

    list.push_back(1); // [1,1]
    ASSERT_EQ(list.size(), 2);
    ASSERT_EQ(list.back(), 1);
    ASSERT_FALSE(list.empty());

    list.pop_back(); // [1]
    ASSERT_EQ(list.size(), 1);
    ASSERT_EQ(list.back(), 1);
    ASSERT_FALSE(list.empty());

    list.pop_back(); // []
    ASSERT_EQ(list.size(), 0);
    ASSERT_TRUE(list.empty());
}

TEST_F(BlockListTest, TestBlockListIteratorWalk) {
    BlockList<int> list;
    BlockList<int>::iterator it = list.begin();
    BlockList<int>::const_iterator const_it = list.begin();
    for (int i = 10000; i > 0; i--) {
        list.push_back(i);
        ASSERT_EQ(list.size(), 1);
        ASSERT_FALSE(list.empty());

        ASSERT_EQ(*it, i);
        ASSERT_EQ(*const_it, i);
        ++it;
        ++const_it;

        list.pop_front();
        ASSERT_EQ(list.size(), 0);
        ASSERT_TRUE(list.empty());
    }
}

TEST_F(BlockListTest, TestBlockListAppendRange) {
    BlockList<int> list;
    std::vector<int> range(10000);
    ranges::iota(range, 0);
    list.append_range(range);
    ASSERT_EQ(list.size(), 10000);

    int i = 0;
    for (const auto &[x, r] : ranges::views::zip(list, range)) {
        ASSERT_EQ(x, r);
        i = x;
    }
    ASSERT_EQ(i, 9999);
}

TEST_F(BlockListTest, TestBlockListIteratorMidway) {
    BlockList<int> list;
    std::vector<int> range(10000);
    ranges::iota(range, 0);
    list.append_range(range);

    for (int i = 0; i < 5000; i++) {
        ASSERT_EQ(list.front(), i);
        ASSERT_EQ(list.back(), 9999 - i);

        ASSERT_EQ(list.size(), 10000 - 2 * i);

        list.pop_front();
        list.pop_back();
    }

    for (int i = 0; i < 10000; i++) {
        ASSERT_EQ(list.size(), i);

        list.push_back(i);

        ASSERT_EQ(list.front(), 0);
        ASSERT_EQ(list.back(), i);
    }
}

class BlockListMultiThreadTest : public ::testing::TestWithParam<unsigned> {
  protected:
    BlockList<unsigned> queue;
    void SetUp() override {
        auto num_cpus = GetParam();
        ThreadManager::override_concurrency(num_cpus);
        ThreadManager::register_self(0);
    }

    void TearDown() override { ThreadManager::reset_concurrency_override(); }
};

TEST_P(BlockListMultiThreadTest, TestQueuePush) {
    auto num_cpus = ThreadManager::available_concurrency();
    std::atomic<unsigned> ready{num_cpus + 1};
    std::mutex m;

    auto work = [&](const typename ThreadManager::ThreadId &cpu_id) {
        m.lock();
        ThreadManager::register_self(cpu_id);
        m.unlock();
        ready.fetch_sub(1);
        while (!ready.load() == 0) {
        }
        for (unsigned i = 0; i < 1000; i++) {
            queue.push_back(i);
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
        x = queue.pop_front();
    } while (x.has_value());
    ASSERT_EQ(total, 1000 * 999 / 2 * num_cpus);
}

TEST_P(BlockListMultiThreadTest, TestQueuePop) {
    auto num_cpus = ThreadManager::available_concurrency();
    std::atomic<unsigned> ready{num_cpus + 1};
    std::atomic<unsigned> total{0};
    std::mutex m;

    auto work = [&](const typename ThreadManager::ThreadId &cpu_id) {
        m.lock();
        ThreadManager::register_self(cpu_id);
        m.unlock();
        ready.fetch_sub(1);
        while (!ready.load() == 0) {
        }
        for (unsigned i = 0; i < 1000; i++) {
            auto x = queue.pop_front();
            total += x.value();
        }
    };

    std::vector<std::thread> threads;
    for (unsigned i = 0; i < num_cpus; i++) {
        threads.push_back(std::thread(work, i));
    }
    for (unsigned j = 0; j < num_cpus; j++) {
        for (unsigned i = 0; i < 1000; i++) {
            queue.push_back(i);
        }
    }
    ready.fetch_sub(1);
    for (auto &thread : threads) {
        thread.join();
    }

    ASSERT_TRUE(queue.empty());
    ASSERT_EQ(total, 1000 * 999 / 2 * num_cpus);
}

TEST_P(BlockListMultiThreadTest, TestQueue) {
    auto num_cpus = ThreadManager::available_concurrency();
    std::atomic<unsigned> ready{num_cpus + 1};
    std::atomic<unsigned> total{0};
    std::mutex m;

    auto work = [&](const typename ThreadManager::ThreadId &cpu_id) {
        m.lock();
        ThreadManager::register_self(cpu_id);
        m.unlock();
        ready.fetch_sub(1);
        while (!ready.load() == 0) {
        }
        for (unsigned i = 0; i < 1000; i++) {
            queue.push_back(i);
            std::this_thread::sleep_for(10us);
            std::optional<unsigned> x = queue.pop_front();
            if (x.has_value()) {
                total.fetch_add(*x, std::memory_order_relaxed);
            }
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
        x = queue.pop_back();
    } while (x.has_value());

    ASSERT_TRUE(queue.empty());
    ASSERT_EQ(queue.size(), 0);
    ASSERT_EQ(total, 1000 * 999 / 2 * num_cpus);
}

TEST_P(BlockListMultiThreadTest, TestStack) {
    auto num_cpus = ThreadManager::available_concurrency();
    std::atomic<unsigned> ready{num_cpus + 1};
    std::atomic<unsigned> total{0};
    std::mutex m;

    auto work = [&](const typename ThreadManager::ThreadId &cpu_id) {
        m.lock();
        ThreadManager::register_self(cpu_id);
        m.unlock();
        ready.fetch_sub(1);
        while (!ready.load() == 0) {
        }
        for (unsigned i = 0; i < 1000; i++) {
            queue.push_back(i);
            std::this_thread::sleep_for(10us);
            std::optional<unsigned> x = queue.pop_back();
            if (x.has_value()) {
                total.fetch_add(*x, std::memory_order_relaxed);
            }
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
        x = queue.pop_back();
    } while (x.has_value());

    ASSERT_TRUE(queue.empty());
    ASSERT_EQ(queue.size(), 0);
    ASSERT_EQ(total, 1000 * 999 / 2 * num_cpus);
}

std::vector<unsigned> queue_test_cpu_counts = {1, 2, 3, 4, 6, 8};
INSTANTIATE_TEST_SUITE_P(BlockListMultiThreadTests, BlockListMultiThreadTest,
                         ::testing::ValuesIn(queue_test_cpu_counts));
