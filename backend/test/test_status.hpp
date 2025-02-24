#pragma once

#include "system/thread_manager.tpp"
#include "work/status.tpp"

#include <gtest/gtest.h>

class StatusTransitionTest : public ::testing::Test {
  protected:
    void SetUp() override {
        ThreadManager::override_concurrency(1);
        ThreadManager::register_self(0);
    }
    void TearDown() override { ThreadManager::reset_concurrency_override(); }
};

TEST_F(StatusTransitionTest, AcquireRelease) {
    Status status;
    ASSERT_FALSE(status.done());
    ASSERT_TRUE(status.acquire());
    ASSERT_TRUE(status.release());
    ASSERT_TRUE(status.acquire());
    ASSERT_FALSE(status.acquire());
    ASSERT_TRUE(status.release());
    ASSERT_FALSE(status.done());
}

TEST_F(StatusTransitionTest, RequiredUnheld) {
    Status status;
    ASSERT_FALSE(status.done());
    ASSERT_FALSE(status.required());
    ASSERT_TRUE(status.require());
    ASSERT_TRUE(status.required());
    ASSERT_FALSE(status.require());
    ASSERT_TRUE(status.required());
    ASSERT_TRUE(status.acquire());
    ASSERT_FALSE(status.release());
    ASSERT_FALSE(status.done());
}

TEST_F(StatusTransitionTest, RequiredHeld) {
    Status status;
    ASSERT_FALSE(status.done());
    ASSERT_TRUE(status.acquire());
    ASSERT_FALSE(status.required());
    ASSERT_TRUE(status.require());
    ASSERT_TRUE(status.required());
    ASSERT_FALSE(status.require());
    ASSERT_TRUE(status.required());
    ASSERT_FALSE(status.release());
    ASSERT_FALSE(status.done());
}

TEST_F(StatusTransitionTest, Finish) {
    Status status;
    ASSERT_FALSE(status.done());
    ASSERT_TRUE(status.acquire());
    ASSERT_FALSE(status.done());
    status.finish();
    ASSERT_TRUE(status.done());
}
