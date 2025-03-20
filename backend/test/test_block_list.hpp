#pragma once

#include "data_structures/block_list.tpp"
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

TEST(BlockListTest, BlockListTypesTest) {
    using I = BlockList<int>::iterator;
    static_assert(std::forward_iterator<I>);
    static_assert(std::bidirectional_iterator<I>);
    static_assert(std::ranges::forward_range<BlockList<int>>);
    std::stack<int, BlockList<int>> stack;
}

TEST(BlockListTest, TestEmptySizeIsZero) {
    BlockList<int> list;
    ASSERT_EQ(list.size(), 0);
}

TEST(BlockListTest, TestSizeIncrementsOnPushBack) {
    BlockList<int> list;
    ASSERT_EQ(list.size(), 0);
    list.push_back(1);
    ASSERT_EQ(list.size(), 1);
    int x = 4;
    list.push_back(x);
    ASSERT_EQ(list.size(), 2);
}

TEST(BlockListTest, TestBlockListIterators) {
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

TEST(BlockListTest, TestPopBack) {
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

TEST(BlockListTest, TestClear) {
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

    list.clear(); // []
    for (unsigned i = 0; i < 10000; i++) {
        list.push_back(i);
    }
    ASSERT_EQ(list.size(), 10000);
    ASSERT_FALSE(list.empty());
    ASSERT_EQ(list.back(), 9999);

    list.clear(); // []

    list.push_back(1); // [1]
    ASSERT_EQ(list.size(), 1);
    ASSERT_EQ(list.back(), 1);
    ASSERT_FALSE(list.empty());

    list.pop_back(); // []
    ASSERT_EQ(list.size(), 0);
    ASSERT_TRUE(list.empty());
}
