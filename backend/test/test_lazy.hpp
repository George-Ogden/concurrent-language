#pragma once

#include "lazy/lazy.tpp"
#include "work/work.tpp"

#include "test/inc.hpp"

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

TEST(LazyWorkTest, LazyConstantWork) {
    LazyConstant<Int> x{3};
    ASSERT_EQ(x.get_work(), std::nullopt);
}

TEST(LazyWorkTest, LazyWork) {
    auto [work, lazy] = Work::fn_call(inc_fn, make_lazy<Int>(4));
    ASSERT_EQ(lazy->get_work(), work);
}

TEST(LazyWorkTest, LazyPlaceholderWork) {
    auto [work, lazy] = Work::fn_call(inc_fn, make_lazy<Int>(4));
    LazyPlaceholder<Int> placeholder{work};
    ASSERT_EQ(lazy->get_work(), work);
    work->run();
    ASSERT_EQ(lazy->get_work(), std::nullopt);
}

TEST(LazyWorkTest, LazyDoublePlaceholderWork) {
    auto [work1, lazy1] = Work::fn_call(inc_fn, make_lazy<Int>(4));
    auto [work2, lazy2] = Work::fn_call(inc_fn, lazy1);
    ASSERT_EQ(lazy1->get_work(), work1);
    ASSERT_EQ(lazy2->get_work(), work2);
    work1->run();
    ASSERT_EQ(lazy1->get_work(), std::nullopt);
    ASSERT_EQ(lazy2->get_work(), work2);
}

TEST(LazyWorkTest, AddWork) {
    std::vector<WorkT> works;
    auto [work, lazy] = Work::fn_call(inc_fn, make_lazy<Int>(4));
    LazyConstant<Int> x{3};
    x.save_work(works);
    ASSERT_EQ(works, std::vector<WorkT>{});
    lazy->save_work(works);
    ASSERT_EQ(works, std::vector<WorkT>{work});
}
