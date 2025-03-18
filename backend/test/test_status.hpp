#pragma once

#include "system/thread_manager.tpp"
#include "work/status.hpp"

#include <gtest/gtest.h>

class StatusTransitionTest : public ::testing::Test {
  protected:
    void SetUp() override {
        ThreadManager::override_concurrency(1);
        ThreadManager::register_self(0);
    }
    void TearDown() override { ThreadManager::reset_concurrency_override(); }
};

TEST_F(StatusTransitionTest, FillJob) {
    Status status;
    ASSERT_FALSE(status.queued());
    ASSERT_FALSE(status.available());
    ASSERT_TRUE(status.unavailable());
    ASSERT_FALSE(status.full());

    ASSERT_TRUE(status.request());

    ASSERT_FALSE(status.queued());
    ASSERT_TRUE(status.available());
    ASSERT_FALSE(status.unavailable());
    ASSERT_FALSE(status.full());

    ASSERT_TRUE(status.enqueue());

    ASSERT_TRUE(status.queued());
    ASSERT_TRUE(status.available());
    ASSERT_FALSE(status.unavailable());
    ASSERT_FALSE(status.full());

    ASSERT_TRUE(status.fill());

    ASSERT_FALSE(status.queued());
    ASSERT_FALSE(status.available());
    ASSERT_FALSE(status.unavailable());
    ASSERT_TRUE(status.full());

    ASSERT_TRUE(status.complete());

    ASSERT_FALSE(status.queued());
    ASSERT_FALSE(status.available());
    ASSERT_TRUE(status.unavailable());
    ASSERT_FALSE(status.full());
}

TEST_F(StatusTransitionTest, CancelJob) {
    Status status;
    ASSERT_FALSE(status.queued());
    ASSERT_FALSE(status.available());
    ASSERT_TRUE(status.unavailable());
    ASSERT_FALSE(status.full());

    ASSERT_TRUE(status.request());

    ASSERT_FALSE(status.queued());
    ASSERT_TRUE(status.available());
    ASSERT_FALSE(status.unavailable());
    ASSERT_FALSE(status.full());

    ASSERT_TRUE(status.enqueue());

    ASSERT_TRUE(status.queued());
    ASSERT_TRUE(status.available());
    ASSERT_FALSE(status.unavailable());
    ASSERT_FALSE(status.full());

    ASSERT_TRUE(status.cancel());

    ASSERT_TRUE(status.queued());
    ASSERT_FALSE(status.available());
    ASSERT_TRUE(status.unavailable());

    ASSERT_FALSE(status.fill());
    ASSERT_TRUE(status.request());

    ASSERT_FALSE(status.queued());
    ASSERT_TRUE(status.available());
    ASSERT_FALSE(status.unavailable());
    ASSERT_FALSE(status.full());

    ASSERT_TRUE(status.fill());
    ASSERT_FALSE(status.cancel());

    ASSERT_FALSE(status.queued());
    ASSERT_FALSE(status.available());
    ASSERT_FALSE(status.unavailable());
    ASSERT_TRUE(status.full());
}

TEST_F(StatusTransitionTest, Dequeue) {
    Status status;
    ASSERT_FALSE(status.queued());
    ASSERT_FALSE(status.available());
    ASSERT_TRUE(status.unavailable());
    ASSERT_FALSE(status.full());

    ASSERT_TRUE(status.request());

    ASSERT_FALSE(status.queued());
    ASSERT_TRUE(status.available());
    ASSERT_FALSE(status.unavailable());
    ASSERT_FALSE(status.full());

    ASSERT_TRUE(status.enqueue());

    ASSERT_TRUE(status.queued());
    ASSERT_TRUE(status.available());
    ASSERT_FALSE(status.unavailable());
    ASSERT_FALSE(status.full());

    ASSERT_FALSE(status.enqueue());
    ASSERT_FALSE(status.dequeue());
    ASSERT_TRUE(status.cancel());

    ASSERT_TRUE(status.queued());
    ASSERT_FALSE(status.available());
    ASSERT_TRUE(status.unavailable());

    ASSERT_TRUE(status.dequeue());

    ASSERT_FALSE(status.queued());
    ASSERT_FALSE(status.available());
    ASSERT_TRUE(status.unavailable());
    ASSERT_FALSE(status.full());
}
