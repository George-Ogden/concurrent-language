#pragma once

#include "data_structures/lazy.hpp"
#include "data_structures/lock.hpp"
#include "fn/continuation.hpp"
#include "fn/operators.hpp"
#include "system/thread_manager.hpp"
#include "types/builtin.hpp"
#include "types/compound.hpp"

#include <gtest/gtest.h>

#include <atomic>
#include <memory>

class LazyConstantTest : public ::testing::Test {
  protected:
    LazyConstant<Int> x{3};
    void SetUp() override { ThreadManager::register_self(0); }
};

TEST_F(LazyConstantTest, AlwaysDone) {
    auto x = this->x;
    ASSERT_TRUE(x.done());
}

TEST_F(LazyConstantTest, CorrectValue) { ASSERT_EQ(x.value(), 3); }

TEST_F(LazyConstantTest, UnfinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{2};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    x.add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(remaining->load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(**valid, true);
    delete remaining;
}

TEST_F(LazyConstantTest, FinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    x.add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
    ASSERT_EQ(**valid, false);
}

TEST_F(LazyConstantTest, InvalidFinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{false};
    x.add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
}

class LazyFunctionTest : public ::testing::Test {
  protected:
    FnT<Int, Int> lazy_fn;
    void SetUp() override {
        ThreadManager::override_concurrency(1);
        ThreadManager::register_self(0);
        WorkManager::counters = std::vector<std::atomic<unsigned>>(1);
        lazy_fn = std::make_shared<Increment__BuiltIn_Fn>();
        lazy_fn->args = std::make_tuple(std::make_shared<LazyConstant<Int>>(4));
    }
    void TearDown() override { ThreadManager::reset_concurrency_override(); }
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
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{2};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    lazy_fn->run();
    lazy_fn->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(remaining->load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(**valid, true);
    delete remaining;
}

TEST_F(LazyFunctionTest, NotDoneUnfinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{2};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    lazy_fn->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(remaining->load(std::memory_order_relaxed), 2);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(**valid, true);
    lazy_fn->run();
    ASSERT_EQ(remaining->load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(**valid, true);
    delete remaining;
}

TEST_F(LazyFunctionTest, DoneFinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    ASSERT_EQ(**valid, true);
    lazy_fn->run();
    lazy_fn->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
    ASSERT_EQ(**valid, false);
}

TEST_F(LazyFunctionTest, NotDoneFinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    lazy_fn->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(remaining->load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(**valid, true);
    lazy_fn->run();
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
    ASSERT_EQ(**valid, false);
}

TEST_F(LazyFunctionTest, DoneInvalidFinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{false};
    ASSERT_EQ(**valid, false);
    lazy_fn->run();
    lazy_fn->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
}

TEST_F(LazyFunctionTest, NotDoneInvalidFinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{false};
    lazy_fn->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(remaining->load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(**valid, false);
    lazy_fn->run();
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
}

TEST(LazyExtractionTest, Int) { ASSERT_EQ(Lazy<Int>::extract_value(8), 8); }

TEST(LazyExtractionTest, LazyInt) {
    LazyT<Int> ptr = std::make_shared<LazyConstant<Int>>(8);
    ASSERT_EQ(Lazy<Int>::extract_value(ptr), 8);
}

TEST(LazyExtractionTest, LazyTuple) {
    LazyT<TupleT<Int, TupleT<Int>>> ptr = std::make_tuple(
        std::make_shared<LazyConstant<Int>>(6),
        std::make_tuple(std::make_shared<LazyConstant<Int>>(10)));
    ASSERT_EQ((Lazy<TupleT<Int, TupleT<Int>>>::extract_value(ptr)),
              (std::make_tuple(6, std::make_tuple(10))));
}
