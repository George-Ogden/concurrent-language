#pragma once

#include "fn/continuation.tpp"

#include <gtest/gtest.h>

#include <atomic>
#include <memory>

class ContinuationTest : public ::testing::Test {
  protected:
    void SetUp() override { ThreadManager::register_self(0); }
};

TEST_F(ContinuationTest, UnfinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{2};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    Continuation c{remaining, counter, valid};
    c.update();
    ASSERT_EQ(remaining->load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(**valid, true);
    delete remaining;
    delete valid;
}

TEST_F(ContinuationTest, FinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    Continuation c{remaining, counter, valid};
    c.update();
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
    ASSERT_EQ(**valid, false);
    delete valid;
}

TEST_F(ContinuationTest, InvalidFinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{false};
    Continuation c{remaining, counter, valid};
    c.update();
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
}
