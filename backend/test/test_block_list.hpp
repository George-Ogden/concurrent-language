#pragma once

#include "data_structures/block_list.hpp"
#include "system/thread_manager.tpp"

#include <gtest/gtest.h>
#include <range/v3/numeric/iota.hpp>
#include <range/v3/view/zip.hpp>

#include <iterator>
#include <queue>
#include <ranges>
#include <stack>

TEST(BlockList, BlockListTypesTest) {
    using I = BlockList<int>::iterator;
    static_assert(std::forward_iterator<I>);
    static_assert(std::bidirectional_iterator<I>);
    static_assert(std::ranges::forward_range<BlockList<int>>);
    std::stack<int, BlockList<int>> stack;
    std::queue<int, BlockList<int>> queue;
}

TEST(BlockList, TestEmptySizeIsZero) {
    BlockList<int> list;
    ASSERT_EQ(list.size(), 0);
}

TEST(BlockList, TestSizeIncrementsOnPushBack) {
    BlockList<int> list;
    ASSERT_EQ(list.size(), 0);
    list.push_back(1);
    ASSERT_EQ(list.size(), 1);
    int x = 4;
    list.push_back(x);
    ASSERT_EQ(list.size(), 2);
}

TEST(BlockList, TestBlockListIterators) {
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

TEST(BlockList, TestPopFront) {
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

TEST(BlockList, TestPopBack) {
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

TEST(BlockList, TestBlockListIteratorWalk) {
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

TEST(BlockList, TestBlockListAppendRange) {
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

TEST(BlockList, TestBlockListIteratorMidway) {
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
