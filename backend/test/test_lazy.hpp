#pragma once

#include "data_structures/lazy.hpp"
#include "fn/continuation.hpp"
#include "fn/operators.hpp"
#include "types/predefined.hpp"

#include <gtest/gtest.h>

#include <atomic>

class LazyConstantTest : public ::testing::Test {
  protected:
    LazyConstant<Int> x{3};
};

TEST_F(LazyConstantTest, AlwaysDone) {
    auto x = this->x;
    ASSERT_TRUE(x.done());
}

TEST_F(LazyConstantTest, CorrectValue) { ASSERT_EQ(x.value(), 3); }

TEST_F(LazyConstantTest, UnfinishedContinuationBehaviour) {
    std::atomic<unsigned> remaining{2};
    std::atomic<unsigned> counter{1};
    x.add_continuation(Continuation{remaining, counter});
    ASSERT_EQ(remaining.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
}

TEST_F(LazyConstantTest, FinishedContinuationBehaviour) {
    std::atomic<unsigned> remaining{1};
    std::atomic<unsigned> counter{1};
    x.add_continuation(Continuation{remaining, counter});
    ASSERT_EQ(remaining.load(std::memory_order_relaxed), 0);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
}

class LazyFunctionTest : public ::testing::Test {
  protected:
    ParametricFn<Int, Int> *lazy_fn;
    LazyConstant<Int> x{4};
    void SetUp() override {
        ThreadManager::override_concurrency(1);
        ThreadManager::register_self(0);
        lazy_fn = new Increment__BuiltIn{};
        lazy_fn->args = std::make_tuple(&x);
    }
    void TearDown() override {
        delete lazy_fn;
        ThreadManager::reset_concurrency_override();
    }
};

TEST_F(LazyFunctionTest, DoneLater) {
    ASSERT_FALSE(lazy_fn->done());
    lazy_fn->run();
    ASSERT_TRUE(lazy_fn->done());
}

TEST_F(LazyFunctionTest, CorrectValue) {
    lazy_fn->run();
    ASSERT_EQ(lazy_fn->value(), 5);
}

TEST_F(LazyFunctionTest, DoneUnfinishedContinuationBehaviour) {
    std::atomic<unsigned> remaining{2};
    std::atomic<unsigned> counter{1};
    lazy_fn->run();
    lazy_fn->add_continuation(Continuation{remaining, counter});
    ASSERT_EQ(remaining.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
}

TEST_F(LazyFunctionTest, NotDoneUnfinishedContinuationBehaviour) {
    std::atomic<unsigned> remaining{2};
    std::atomic<unsigned> counter{1};
    lazy_fn->add_continuation(Continuation{remaining, counter});
    ASSERT_EQ(remaining.load(std::memory_order_relaxed), 2);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    lazy_fn->run();
    ASSERT_EQ(remaining.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
}

TEST_F(LazyFunctionTest, DoneFinishedContinuationBehaviour) {
    std::atomic<unsigned> remaining{1};
    std::atomic<unsigned> counter{1};
    lazy_fn->run();
    lazy_fn->add_continuation(Continuation{remaining, counter});
    ASSERT_EQ(remaining.load(std::memory_order_relaxed), 0);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
}

TEST_F(LazyFunctionTest, NotDoneFinishedContinuationBehaviour) {
    std::atomic<unsigned> remaining{1};
    std::atomic<unsigned> counter{1};
    lazy_fn->add_continuation(Continuation{remaining, counter});
    ASSERT_EQ(remaining.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    lazy_fn->run();
    ASSERT_EQ(remaining.load(std::memory_order_relaxed), 0);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
}
