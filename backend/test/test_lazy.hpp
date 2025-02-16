#pragma once

#include "data_structures/lazy.tpp"

#include <gtest/gtest.h>

#include <atomic>
#include <memory>

class LazyConstantTest : public ::testing::Test {
  protected:
    LazyT<Int> x = std::make_shared<Lazy<Int>>(3);
    void SetUp() override { ThreadManager::register_self(0); }
};

TEST_F(LazyConstantTest, AlwaysDone) { ASSERT_TRUE(x->done()); }

TEST_F(LazyConstantTest, CorrectValue) { ASSERT_EQ(x->value(), 3); }

TEST(MakeLazyTest, CorrectValue) {
    LazyT<Int> y = make_lazy<Int>(-3);
    ASSERT_EQ(y->value(), -3);
}

TEST_F(LazyConstantTest, UnfinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{2};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    x->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(remaining->load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(**valid, true);
    delete remaining;
}

TEST_F(LazyConstantTest, FinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    x->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
    ASSERT_EQ(**valid, false);
}

TEST_F(LazyConstantTest, InvalidFinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{false};
    x->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
}
