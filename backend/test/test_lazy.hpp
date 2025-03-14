#pragma once

#include "lazy/fns.hpp"
#include "lazy/lazy.tpp"
#include "work/work.tpp"

#include <gtest/gtest.h>

#include <atomic>
#include <memory>
#include <optional>

class LazyConstantTest : public ::testing::Test {
  protected:
    LazyT<Int> x = make_lazy<Int>(3);
    void SetUp() override { ThreadManager::register_self(0); }
};

TEST_F(LazyConstantTest, AlwaysDone) { ASSERT_TRUE(x->done()); }

TEST_F(LazyConstantTest, CorrectValue) { ASSERT_EQ(x->value(), 3); }

TEST(MakeLazyTest, CorrectValue) {
    LazyT<Int> y = make_lazy<Int>(-3);
    ASSERT_EQ(y->value(), -3);
}

TEST(EnsureLazyTest, NonLazy) {
    LazyT<Int> y = ensure_lazy(Int{-3});
    ASSERT_EQ(y->value(), -3);
}

TEST(EnsureLazyTest, Lazy) {
    LazyT<Int> y = ensure_lazy(make_lazy<Int>(-3));
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
    delete valid;
    delete remaining;
}

TEST_F(LazyConstantTest, FinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    x->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
    ASSERT_EQ(**valid, false);
    delete valid;
}

TEST_F(LazyConstantTest, InvalidFinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{false};
    x->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
}

TEST(LazyCacheTest, BooleanCache) {
    LazyT<Bool> t0 = make_lazy<Bool>(true);
    LazyT<Bool> t1 = make_lazy<Bool>(true);
    LazyT<Bool> f0 = make_lazy<Bool>(false);
    LazyT<Bool> f1 = make_lazy<Bool>(false);
    ASSERT_TRUE(t0->value());
    ASSERT_TRUE(t1->value());
    ASSERT_FALSE(f0->value());
    ASSERT_FALSE(f1->value());

    ASSERT_EQ(t0.get(), t1.get());
    ASSERT_EQ(f0.get(), f1.get());
}

TEST(LazyCacheTest, IntegerCache) {
    for (Int i = -128; i < 128; i++) {
        LazyT<Int> m = make_lazy<Int>(i);
        LazyT<Int> n = make_lazy<Int>(i);
        ASSERT_EQ(m->value(), i);
        ASSERT_EQ(n->value(), i);
        ASSERT_EQ(m.get(), n.get());
    }
}

TEST(LazyCacheTest, IntegerCacheBounds) {
    for (Int i : std::vector<Int>{-129, 128}) {
        LazyT<Int> m = make_lazy<Int>(i);
        LazyT<Int> n = make_lazy<Int>(i);
        ASSERT_EQ(m->value(), i);
        ASSERT_EQ(n->value(), i);
        ASSERT_NE(m.get(), n.get());
    }
}
