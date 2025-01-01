#pragma once

#include "data_structures/lazy.hpp"
#include "fn/continuation.hpp"

#include <gtest/gtest.h>

#include <atomic>

TEST(LazyConstant, AlwaysDone) {
    LazyConstant<int> x{3};
    ASSERT_TRUE(x.done());
}

TEST(LazyConstant, CorrectValue) {
    LazyConstant<int> x{3};
    ASSERT_EQ(x.value(), 3);
}

TEST(LazyConstant, UnfinishedContinuationBehaviour) {
    LazyConstant<int> x{3};
    std::atomic<unsigned> remaining{2};
    std::atomic<unsigned> counter{1};
    x.add_continuation(Continuation{remaining, counter});
    ASSERT_EQ(remaining.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
}

TEST(LazyConstant, FinishedContinuationBehaviour) {
    LazyConstant<int> x{3};
    std::atomic<unsigned> remaining{1};
    std::atomic<unsigned> counter{1};
    x.add_continuation(Continuation{remaining, counter});
    ASSERT_EQ(remaining.load(std::memory_order_relaxed), 0);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
}
